mod bunny_state;
mod graphics;

use bunny_state::{
    add_bunnies, simulate_bunnies, update_bunnies_gpu, BunnyPosition, BunnyState, Vel,
};
use graphics::{init_graphics, render, resize_window, FrameTime, Graphics};
use shipyard::borrow::NonSendSync;
use shipyard::{UniqueView, UniqueViewMut, World};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }

    // Browser graphics are initialized asynchronously, they come back to the event loop as a user event.
    let event_loop = EventLoop::<Graphics>::with_user_event()
        .build()
        .expect("Couldn't create the event loop");
    let app = App::new(&event_loop);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop
            .run_app(&mut app)
            .expect("The event loop stopped with an error");
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;

        event_loop.spawn_app(app);
    }
}

struct App {
    world: World,
    egui_context: egui::Context,
    /// Created on the first resume.
    window: Option<AppWindow>,
    /// Set once the `Graphics` unique is in the world.
    graphics_ready: bool,
    initial_bunnies_spawned: bool,
    frame_timer: FrameTimer,
    /// Brings the initialized graphics back to the event loop.
    proxy: EventLoopProxy<Graphics>,
}

struct AppWindow {
    window: Arc<Window>,
    egui_state: egui_winit::State,
}

impl App {
    fn new(event_loop: &EventLoop<Graphics>) -> App {
        let world = World::builder()
            .with_group::<(BunnyPosition, Vel)>()
            .build();

        world.add_unique(FrameTime(0.0));
        world.add_unique(BunnyState {
            bunnies_per_click: 1000,
        });
        world.add_workload(|| simulate_bunnies);

        App {
            world,
            egui_context: egui::Context::default(),
            window: None,
            graphics_ready: false,
            initial_bunnies_spawned: false,
            frame_timer: FrameTimer::new(),
            proxy: event_loop.create_proxy(),
        }
    }

    /// Stores the initialized graphics and sizes the surface.
    fn add_graphics(&mut self, graphics: Graphics) {
        self.world.add_unique_non_send_sync(graphics);
        self.graphics_ready = true;

        // Resizes that happened during initialization had no surface to apply to.
        // We catch up with the latest size, which also configures the surface for the first time.
        if let Some(app_window) = &self.window {
            self.world
                .run_with_data(resize_window, app_window.window.inner_size());
        }
    }

    /// Returns `true` once graphics are initialized and the surface has a size we can draw to.
    fn can_draw(&self) -> bool {
        self.graphics_ready
            && self
                .world
                .run(|graphics: NonSendSync<UniqueView<Graphics>>| graphics.is_configured())
    }

    fn redraw(&mut self) {
        if !self.can_draw() {
            return;
        }

        let Some(app_window) = &mut self.window else {
            return;
        };

        // Bunnies are placed relative to the window size, so we wait until we have one.
        if !self.initial_bunnies_spawned {
            self.world.run(add_bunnies);
            self.initial_bunnies_spawned = true;
        }

        if let Some(frame_time) = self.frame_timer.tick() {
            self.world
                .run(|mut frame_time_view: UniqueViewMut<FrameTime>| {
                    frame_time_view.0 = frame_time;
                });
        }

        self.world
            .run_default_workload()
            .expect("Couldn't run the simulation workload");
        self.world.run(update_bunnies_gpu);

        let egui_input = app_window.egui_state.take_egui_input(&app_window.window);
        if let Some(platform_output) = self.world.run_with_data(render, egui_input) {
            app_window
                .egui_state
                .handle_platform_output(&app_window.window, platform_output);
        }
    }
}

impl ApplicationHandler<Graphics> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Some platforms resume more than once, we only set things up the first time.
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .expect("Couldn't create the window"),
        );

        #[cfg(target_arch = "wasm32")]
        mount_canvas(&window);

        let egui_state = egui_winit::State::new(
            self.egui_context.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        self.window = Some(AppWindow {
            window: window.clone(),
            egui_state,
        });

        let graphics = init_graphics(
            window,
            event_loop.owned_display_handle(),
            self.egui_context.clone(),
        );

        // Browsers can't block on the GPU futures, there initialization finishes later.
        // Both paths hand the graphics back with a user event.
        // Sending only fails once the event loop is gone, nothing is left to draw then.
        let proxy = self.proxy.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let _ = proxy.send_event(pollster::block_on(graphics));

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let _ = proxy.send_event(graphics.await);
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, graphics: Graphics) {
        self.add_graphics(graphics);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app_window) = &mut self.window else {
            return;
        };

        if window_id != app_window.window.id() {
            return;
        }

        if app_window
            .egui_state
            .on_window_event(&app_window.window, &event)
            .consumed
        {
            return;
        }

        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Space),
                        ..
                    },
                ..
            }
            | WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.can_draw() {
                    self.world.run(add_bunnies);
                }
            }
            WindowEvent::Resized(physical_size) => {
                // Before graphics are ready, `add_graphics` picks up the latest size instead.
                if self.graphics_ready {
                    self.world.run_with_data(resize_window, physical_size);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app_window) = &self.window {
            app_window.window.request_redraw();
        }
    }
}

/// Puts the canvas under `#bunny_demo` and sizes it to the browser window.
#[cfg(target_arch = "wasm32")]
fn mount_canvas(window: &Window) {
    use winit::dpi::LogicalSize;
    use winit::platform::web::WindowExtWebSys;

    let browser_window = web_sys::window().expect("Couldn't get the browser window");

    let canvas = window.canvas().expect("Couldn't get the canvas");
    browser_window
        .document()
        .and_then(|document| document.get_element_by_id("bunny_demo"))
        .and_then(|container| container.append_child(&canvas).ok())
        .expect("Couldn't append canvas to document body.");

    // The canvas has to be in the page before winit can size it.
    let width = browser_window.inner_width().unwrap().as_f64().unwrap();
    let height = browser_window.inner_height().unwrap().as_f64().unwrap();
    let _ = window.request_inner_size(LogicalSize::new(width, height));
}

/// Averages frame times over 10 frames.
struct FrameTimer {
    #[cfg(not(target_arch = "wasm32"))]
    last_frame: Instant,
    /// In milliseconds, from `performance.now()`.
    #[cfg(target_arch = "wasm32")]
    last_frame: f64,
    frame_count: u32,
    /// In seconds.
    accum_time: f64,
}

impl FrameTimer {
    fn new() -> FrameTimer {
        FrameTimer {
            #[cfg(not(target_arch = "wasm32"))]
            last_frame: Instant::now(),
            #[cfg(target_arch = "wasm32")]
            last_frame: performance_now(),
            frame_count: 0,
            accum_time: 0.0,
        }
    }

    /// Records a frame and returns the average frame time in milliseconds every 10 frames.
    fn tick(&mut self) -> Option<f32> {
        self.frame_count += 1;

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.accum_time += self.last_frame.elapsed().as_secs_f64();
            self.last_frame = Instant::now();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let now = performance_now();
            self.accum_time += (now - self.last_frame) / 1000.0;
            self.last_frame = now;
        }

        if self.frame_count < 10 {
            return None;
        }

        let frame_time = self.accum_time as f32 * 1000.0 / self.frame_count as f32;
        self.accum_time = 0.0;
        self.frame_count = 0;

        Some(frame_time)
    }
}

#[cfg(target_arch = "wasm32")]
fn performance_now() -> f64 {
    web_sys::window().unwrap().performance().unwrap().now()
}
