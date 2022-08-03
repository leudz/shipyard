mod bunny_state;
mod graphics;

use bunny_state::{add_bunnies, simulate_bunnies, update_bunnies_gpu, BunnyState};
use graphics::{init_graphics, render, reset_window, resize_window, FrameTime};
use shipyard::{UniqueViewMut, World};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        // Winit prevents sizing with CSS, so we have to set
        // the size manually when on web.
        use winit::dpi::PhysicalSize;

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| {
                let width = win.inner_width().unwrap().as_f64().unwrap();
                let height = win.inner_height().unwrap().as_f64().unwrap();
                window.set_inner_size(PhysicalSize::new(width, height));

                win.document()
            })
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    let mut state = egui_winit::State::new(&event_loop);
    let context = egui::Context::default();

    let world = World::new();

    world.add_unique(FrameTime(0.0));
    world.add_unique(BunnyState {
        bunnies_per_click: 1000,
    });

    init_graphics(&world, &window, context).await;
    world.run(add_bunnies);

    #[cfg(not(target_arch = "wasm32"))]
    let mut last_frame_inst = Instant::now();
    #[cfg(target_arch = "wasm32")]
    let mut last_frame_inst = web_sys::window().unwrap().performance().unwrap().now();
    let (mut frame_count, mut accum_time) = (0, 0.0);

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
            if world.run(|graphics: UniqueViewMut<graphics::Graphics>| {
                state.on_event(&graphics.context, &event)
            }) {
                return;
            }

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
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Space),
                            ..
                        },
                    ..
                }
                | WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => world.run(add_bunnies),
                WindowEvent::Resized(physical_size) => {
                    world.run_with_data(resize_window, (*physical_size, None));
                }
                WindowEvent::ScaleFactorChanged {
                    new_inner_size,
                    scale_factor,
                } => {
                    // new_inner_size is &&mut so we have to dereference it twice
                    world.run_with_data(
                        resize_window,
                        (**new_inner_size, Some(*scale_factor as f32)),
                    );
                }
                _ => {}
            }
        }
        Event::RedrawRequested(window_id) if window_id == window.id() => {
            frame_count += 1;

            let input = state.take_egui_input(&window);
            world.run(|mut graphics: UniqueViewMut<graphics::Graphics>| {
                graphics.input = input;
            });

            #[cfg(not(target_arch = "wasm32"))]
            {
                accum_time += last_frame_inst.elapsed().as_secs_f64();
                last_frame_inst = Instant::now();
            }
            #[cfg(target_arch = "wasm32")]
            {
                let now = web_sys::window().unwrap().performance().unwrap().now();
                accum_time += (now - last_frame_inst) / 1000.0;
                last_frame_inst = now;
            }

            if frame_count == 10 {
                world.run(|mut frame_time: UniqueViewMut<FrameTime>| {
                    frame_time.0 = accum_time as f32 * 1000.0 / frame_count as f32;
                });

                accum_time = 0.0;
                frame_count = 0;
            }

            world.run(simulate_bunnies);
            world.run(update_bunnies_gpu);

            match world.run(render) {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost) => {
                    world.run(reset_window);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(err) => eprintln!("{err:?}"),
            }
        }
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        _ => {}
    });
}
