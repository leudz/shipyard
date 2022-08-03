use crate::bunny_state::{BunnyPosition, BunnyState};
use egui::{ClippedPrimitive, Context};
use egui_wgpu::renderer::ScreenDescriptor;
use image::GenericImageView;
use shipyard::{Unique, UniqueView, UniqueViewMut, View, World};
use wgpu::{include_wgsl, util::DeviceExt};
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub(crate) struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    pub(crate) const fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[derive(Unique)]
pub(crate) struct Graphics {
    pub(crate) surface: wgpu::Surface,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) size: winit::dpi::PhysicalSize<u32>,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) vertex_count: u32,
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) scale_factor: f32,
    pub(crate) screen_size_buffer: wgpu::Buffer,
    pub(crate) screen_size_bind_group: wgpu::BindGroup,
    pub(crate) bunny_diffuse_bind_group: wgpu::BindGroup,
    pub(crate) context: egui::Context,
    pub(crate) input: egui::RawInput,
    pub(crate) egui_render_pass: egui_wgpu::renderer::RenderPass,
}

pub(crate) async fn init_graphics(world: &World, window: &Window, context: Context) {
    let size = window.inner_size();

    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let surface = unsafe { instance.create_surface(window) };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
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
                #[cfg(target_arch = "wasm32")]
                limits: wgpu::Limits::downlevel_webgl2_defaults(),
                #[cfg(not(target_arch = "wasm32"))]
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .unwrap();

    let supported_format = surface.get_supported_formats(&adapter)[0];
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: supported_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
    };
    surface.configure(&device, &config);

    let screen_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Screen Size Buffer"),
        contents: bytemuck::cast_slice(&[size.width as f32, size.height as f32, 0.0, 0.0]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let screen_size_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("screen_size_bind_group_layout"),
        });

    let screen_size_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &screen_size_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: screen_size_buffer.as_entire_binding(),
        }],
        label: Some("screen_size_bind_group"),
    });

    let diffuse_bytes = include_bytes!("bunny.png");
    let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
    let diffuse_rgba = diffuse_image.to_rgba8();

    let dimensions = diffuse_image.dimensions();

    let texture_size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };
    let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("diffuse_texture"),
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &diffuse_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &diffuse_rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
            rows_per_image: std::num::NonZeroU32::new(dimensions.1),
        },
        texture_size,
    );

    let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: None,
        ..Default::default()
    });

    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

    let bunny_diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &texture_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
            },
        ],
        label: Some("diffuse_bind_group"),
    });

    let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&screen_size_bind_group_layout, &texture_bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[Vertex::desc(), BunnyPosition::desc()],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Cw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    });

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&[
            [[0.0f32, 0.0], [0.0, 1.0]],
            [[0.0, 32.0], [0.0, 0.0]],
            [[25.0, 0.0], [1.0, 1.0]],
            [[0.0, 32.0], [0.0, 0.0]],
            [[25.0, 32.0], [1.0, 0.0]],
            [[25.0, 0.0], [1.0, 1.0]],
        ]),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Instance Buffer"),
        contents: &[],
        usage: wgpu::BufferUsages::VERTEX,
    });

    context.set_pixels_per_point(window.scale_factor() as f32);

    let egui_render_pass = egui_wgpu::renderer::RenderPass::new(&device, supported_format, 1);

    world.add_unique(Graphics {
        surface,
        device,
        queue,
        config,
        size,
        render_pipeline,
        vertex_buffer,
        vertex_count: 0,
        instance_buffer,
        scale_factor: window.scale_factor() as f32,
        screen_size_buffer,
        screen_size_bind_group,
        bunny_diffuse_bind_group,
        context,
        input: egui::RawInput::default(),
        egui_render_pass,
    });
}

pub(crate) fn resize_window(
    (new_size, scale_factor): (winit::dpi::PhysicalSize<u32>, Option<f32>),
    mut graphics: UniqueViewMut<Graphics>,
) {
    if new_size.width > 0 && new_size.height > 0 {
        graphics.size = new_size;
        graphics.config.width = new_size.width;
        graphics.config.height = new_size.height;
        graphics
            .surface
            .configure(&graphics.device, &graphics.config);
        graphics.queue.write_buffer(
            &graphics.screen_size_buffer,
            0,
            bytemuck::cast_slice(&[new_size.width as f32, new_size.height as f32, 0.0, 0.0]),
        );
    }

    if let Some(scale_factor) = scale_factor {
        graphics.scale_factor = scale_factor;
    }
}

pub(crate) fn reset_window(graphics: UniqueViewMut<Graphics>) {
    let size = graphics.size;
    resize_window((size, None), graphics);
}

pub(crate) fn render(
    mut graphics: UniqueViewMut<Graphics>,
    mut bunny_state: UniqueViewMut<BunnyState>,
    position: View<BunnyPosition>,
    frame_time: UniqueView<FrameTime>,
) -> Result<(), wgpu::SurfaceError> {
    let output = graphics.surface.get_current_texture()?;

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = graphics
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    let (paint_jobs, screen_descriptor) = update_egui_render_pass(
        &mut graphics,
        position.len(),
        frame_time.0,
        &mut bunny_state.bunnies_per_click,
    );

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
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
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&graphics.render_pipeline);
        render_pass.set_bind_group(0, &graphics.screen_size_bind_group, &[]);
        render_pass.set_bind_group(1, &graphics.bunny_diffuse_bind_group, &[]);
        render_pass.set_vertex_buffer(0, graphics.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, graphics.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..graphics.vertex_count);

        graphics.egui_render_pass.execute_with_renderpass(
            &mut render_pass,
            &paint_jobs,
            &screen_descriptor,
        );
    }

    graphics.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

#[derive(Unique)]
pub(crate) struct FrameTime(pub(crate) f32);

fn update_egui_render_pass(
    graphics: &mut Graphics,
    bunny_count: usize,
    frame_time: f32,
    bunnies_per_click: &mut u32,
) -> (Vec<ClippedPrimitive>, ScreenDescriptor) {
    let input = graphics.input.clone();
    graphics.context.begin_frame(input);

    egui::Area::new("Bynny Info")
        .fixed_pos(egui::pos2(0.0, 0.0))
        .movable(false)
        .show(&graphics.context, |ui| {
            ui.label(
                egui::RichText::new(format!("Bunny count: {bunny_count}"))
                    .color(egui::Color32::BLACK)
                    .font(egui::FontId::proportional(30.0)),
            );
            ui.label(
                egui::RichText::new(format!("Frame time: {frame_time}ms"))
                    .color(egui::Color32::BLACK)
                    .font(egui::FontId::proportional(30.0)),
            );
            ui.label(
                egui::RichText::new(format!("FPS: {}", 1000.0 / frame_time))
                    .color(egui::Color32::BLACK)
                    .font(egui::FontId::proportional(30.0)),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Bunnies per click: ")
                        .color(egui::Color32::BLACK)
                        .font(egui::FontId::proportional(30.0)),
                );
                ui.add(egui::widgets::DragValue::new(bunnies_per_click).speed(200));
            });
        });

    let egui_output = graphics.context.end_frame();
    let paint_jobs = graphics.context.tessellate(egui_output.shapes);

    let screen_descriptor = ScreenDescriptor {
        pixels_per_point: graphics.scale_factor,
        size_in_pixels: [graphics.size.width, graphics.size.height],
    };
    for (id, image_delta) in &egui_output.textures_delta.set {
        graphics.egui_render_pass.update_texture(
            &graphics.device,
            &graphics.queue,
            *id,
            image_delta,
        );
    }
    for id in &egui_output.textures_delta.free {
        graphics.egui_render_pass.free_texture(id);
    }
    graphics.egui_render_pass.update_buffers(
        &graphics.device,
        &graphics.queue,
        &paint_jobs,
        &screen_descriptor,
    );

    (paint_jobs, screen_descriptor)
}
