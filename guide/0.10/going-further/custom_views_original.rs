/// Cargo.toml
///
/// [dependencies]
/// bytemuck = { version = "1.9.1", features = ["derive"] }
/// env_logger = "0.9"
/// glam = "0.20.2"
/// pollster = "0.2.5"
/// shipyard = { git = "https://github.com/leudz/shipyard"}
/// wgpu = "0.12.0"
/// winit = "0.26.1"
use shipyard::{Unique, UniqueView, UniqueViewMut, World};
use std::iter;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let world = pollster::block_on(init(&window));

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !world.run_with_data(input, event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            world.run_with_data(Graphics::resize, *physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            // new_inner_size is &mut so w have to dereference it twice
                            world.run_with_data(Graphics::resize, **new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                world.run(update);
                match world.run(render) {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => world.run(Graphics::reset_size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => {}
        }
    });
}

async fn init(window: &Window) -> World {
    let world = World::new();

    let size = window.inner_size();

    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let surface = unsafe { instance.create_surface(window) };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None, // Trace path
        )
        .await
        .unwrap();

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_preferred_format(&adapter).unwrap(),
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Mailbox,
    };
    surface.configure(&device, &config);

    let camera = BareCamera {
        eye: (0.0, 5.0, -10.0).into(),
        target: (0.0, 0.0, 0.0).into(),
        up: glam::Vec3::Y,
        aspect: config.width as f32 / config.height as f32,
        fovy: 45.0,
        znear: 0.1,
        zfar: 100.0,
    };
    let camera_controller = CameraController::new(0.2);

    let mut camera_uniform = CameraUniform::new();
    camera_uniform.update_view_proj(&camera);

    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Camera Buffer"),
        contents: bytemuck::cast_slice(&[camera_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    world.add_unique(Graphics {
        surface,
        device,
        queue,
        config,
        size,
    });

    world.add_unique(Camera {
        camera,
        camera_controller,
        camera_uniform,
        camera_buffer,
    });

    world
}

fn input(event: &winit::event::WindowEvent, mut camera_bundle: UniqueViewMut<Camera>) -> bool {
    camera_bundle.camera_controller.process_events(event)
}

fn update(graphics: UniqueView<Graphics>, mut camera_bundle: UniqueViewMut<Camera>) {
    let Camera {
        camera_controller,
        camera_uniform,
        camera,
        ..
    } = &mut *camera_bundle;

    camera_controller.update_camera(camera);
    camera_uniform.update_view_proj(&camera);
    graphics.queue.write_buffer(
        &camera_bundle.camera_buffer,
        0,
        bytemuck::cast_slice(&[camera_bundle.camera_uniform]),
    );
}

#[derive(Unique)]
struct Graphics {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

impl Graphics {
    fn resize(
        new_size: winit::dpi::PhysicalSize<u32>,
        mut this: UniqueViewMut<Graphics>,
        mut camera_bundle: UniqueViewMut<Camera>,
    ) {
        if new_size.width > 0 && new_size.height > 0 {
            this.size = new_size;
            this.config.width = new_size.width;
            this.config.height = new_size.height;
            this.surface.configure(&this.device, &this.config);
            camera_bundle.camera.aspect = this.config.width as f32 / this.config.height as f32;
        }
    }
    fn reset_size(mut this: UniqueViewMut<Graphics>, mut camera_bundle: UniqueViewMut<Camera>) {
        if this.size.width > 0 && this.size.height > 0 {
            this.size = this.size;
            this.config.width = this.size.width;
            this.config.height = this.size.height;
            this.surface.configure(&this.device, &this.config);
            camera_bundle.camera.aspect = this.config.width as f32 / this.config.height as f32;
        }
    }
}

fn render(graphics: UniqueView<Graphics>) -> Result<(), wgpu::SurfaceError> {
    // Get a few things from the GPU
    let output = graphics.surface.get_current_texture()?;
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = graphics
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    {
        // RenderPass borrows encoder for all its lifetime
        let mut _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
    }

    // encoder.finish() consumes `encoder`, so the RenderPass needs to disappear before that to release the borrow
    graphics.queue.submit(iter::once(encoder.finish()));
    output.present();

    Ok(())
}

// Camera

#[derive(Unique)]
struct Camera {
    camera: BareCamera,
    camera_controller: CameraController,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: glam::Mat4 = glam::const_mat4!(
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 0.5, 0.0],
    [0.0, 0.0, 0.5, 1.0]
);

struct BareCamera {
    eye: glam::Vec3,
    target: glam::Vec3,
    up: glam::Vec3,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl BareCamera {
    fn build_view_projection_matrix(&self) -> glam::Mat4 {
        let view = glam::Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = glam::Mat4::perspective_rh_gl(self.fovy, self.aspect, self.znear, self.zfar);
        proj * view
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    fn update_view_proj(&mut self, camera: &BareCamera) {
        self.view_proj =
            (OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix()).to_cols_array_2d();
    }
}

// Camera Controller

use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

struct CameraController {
    speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    fn new(speed: f32) -> Self {
        Self {
            speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
        }
    }

    fn process_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn update_camera(&self, camera: &mut BareCamera) {
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.length();

        // Prevents glitching when camera gets too close to the
        // center of the scene.
        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed;
        }

        let right = forward_norm.cross(camera.up);

        // Redo radius calc in case the up/ down is pressed.
        let forward = camera.target - camera.eye;
        let forward_mag = forward.length();

        if self.is_right_pressed {
            // Rescale the distance between the target and eye so
            // that it doesn't change. The eye therefore still
            // lies on the circle made by the target and eye.
            camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
        }
        if self.is_left_pressed {
            camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
    }
}
