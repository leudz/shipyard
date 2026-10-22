use crate::bunny_state::{BunnyPosition, BunnyState};
use egui_wgpu::ScreenDescriptor;
use image::GenericImageView;
use shipyard::borrow::NonSendSync;
use shipyard::{Unique, UniqueView, UniqueViewMut, View};
use std::sync::Arc;
use wgpu::{include_wgsl, util::DeviceExt};
use winit::dpi::PhysicalSize;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

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

/// Everything we need to draw.
///
/// In the browser the wgpu types aren't `Send` nor `Sync`,
/// it's stored with `add_unique_non_send_sync` and borrowed with [`NonSendSync`].
#[derive(Unique)]
pub(crate) struct Graphics {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// Last non zero window size, zero until the surface is configured.
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) vertex_count: u32,
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) screen_size_buffer: wgpu::Buffer,
    pub(crate) screen_size_bind_group: wgpu::BindGroup,
    pub(crate) bunny_diffuse_bind_group: wgpu::BindGroup,
    pub(crate) context: egui::Context,
    pub(crate) egui_render_pass: egui_wgpu::Renderer,
}

impl Graphics {
    /// Returns `true` once the surface has a size, before that we can't draw.
    pub(crate) fn is_configured(&self) -> bool {
        // `resize_window` only stores non zero sizes, and configures the surface when it does.
        self.size.width > 0 && self.size.height > 0
    }
}

/// Creates the GPU resources for `window`.
///
/// The surface isn't configured yet, [`resize_window`] does it once the window has a size.
pub(crate) async fn init_graphics(
    window: Arc<Window>,
    display: OwnedDisplayHandle,
    context: egui::Context,
) -> Graphics {
    #[cfg(not(target_arch = "wasm32"))]
    let backends = wgpu::Backends::all();
    #[cfg(target_arch = "wasm32")]
    let backends = wgpu::Backends::GL;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display))
    });
    let surface = instance
        .create_surface(window)
        .expect("Couldn't create the surface");
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .expect("Couldn't find a GPU adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            #[cfg(target_arch = "wasm32")]
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            #[cfg(not(target_arch = "wasm32"))]
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .expect("Couldn't create the GPU device");

    let supported_format = surface.get_capabilities(&adapter).formats[0];
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: supported_format,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: 0,
        height: 0,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
    };

    let screen_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Screen Size Buffer"),
        contents: bytemuck::cast_slice(&[0.0f32; 4]),
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
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &diffuse_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &diffuse_rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * dimensions.0),
            rows_per_image: Some(dimensions.1),
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
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
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
        bind_group_layouts: &[
            Some(&screen_size_bind_group_layout),
            Some(&texture_bind_group_layout),
        ],
        immediate_size: 0,
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(Vertex::desc()), Some(BunnyPosition::desc())],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
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
        multiview_mask: None,
        cache: None,
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
    // `update_bunnies_gpu` replaces it with a bigger one when bunnies are spawned.
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Instance Buffer"),
        contents: &[],
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    let egui_render_pass = egui_wgpu::Renderer::new(
        &device,
        supported_format,
        egui_wgpu::RendererOptions::default(),
    );

    Graphics {
        surface,
        device,
        queue,
        config,
        size: PhysicalSize::new(0, 0),
        render_pipeline,
        vertex_buffer,
        vertex_count: 0,
        instance_buffer,
        screen_size_buffer,
        screen_size_bind_group,
        bunny_diffuse_bind_group,
        context,
        egui_render_pass,
    }
}

/// Configures the surface for `new_size`.
///
/// Zero sizes are ignored, wgpu can't configure a surface with them.
/// Minimized windows and canvases not laid out yet report one.
pub(crate) fn resize_window(
    new_size: PhysicalSize<u32>,
    mut graphics: NonSendSync<UniqueViewMut<Graphics>>,
) {
    if new_size.width == 0 || new_size.height == 0 {
        return;
    }

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

/// Draws the bunnies and the egui overlay.
///
/// Returns egui's platform output, to give back to `egui_winit`.\
/// Returns `None` when the frame was skipped.
pub(crate) fn render(
    egui_input: egui::RawInput,
    mut graphics: NonSendSync<UniqueViewMut<Graphics>>,
    mut bunny_state: UniqueViewMut<BunnyState>,
    position: View<BunnyPosition>,
    frame_time: UniqueView<FrameTime>,
) -> Option<egui::PlatformOutput> {
    let graphics: &mut Graphics = &mut graphics;

    let output = match graphics.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(output)
        | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            graphics
                .surface
                .configure(&graphics.device, &graphics.config);
            return None;
        }
        wgpu::CurrentSurfaceTexture::Timeout
        | wgpu::CurrentSurfaceTexture::Occluded
        | wgpu::CurrentSurfaceTexture::Validation => return None,
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // egui frame
    let mut egui_output = graphics.context.run_pass(egui_input, |context| {
        show_bunny_info(
            context,
            position.len(),
            frame_time.0,
            &mut bunny_state.bunnies_per_click,
        )
    });
    let pixels_per_point = egui_output.pixels_per_point;
    let paint_jobs = graphics
        .context
        .tessellate(egui_output.shapes, pixels_per_point);
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [graphics.config.width, graphics.config.height],
        pixels_per_point,
    };

    // egui uploads, they're submitted with the frame below.
    // We drain the deltas, egui asserts none are left unapplied when they're dropped.
    for (id, image_deltas) in egui_output.textures_delta.set.drain() {
        for image_delta in &image_deltas {
            graphics.egui_render_pass.update_texture(
                &graphics.device,
                &graphics.queue,
                id,
                image_delta,
            );
        }
    }

    let mut encoder = graphics
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    let egui_command_buffers = graphics.egui_render_pass.update_buffers(
        &graphics.device,
        &graphics.queue,
        &mut encoder,
        &paint_jobs,
        &screen_descriptor,
    );

    // Bunnies then egui on top
    {
        let mut render_pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            // egui needs a `'static` render pass.
            .forget_lifetime();

        render_pass.set_pipeline(&graphics.render_pipeline);
        render_pass.set_bind_group(0, &graphics.screen_size_bind_group, &[]);
        render_pass.set_bind_group(1, &graphics.bunny_diffuse_bind_group, &[]);
        render_pass.set_vertex_buffer(0, graphics.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, graphics.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..graphics.vertex_count);

        graphics
            .egui_render_pass
            .render(&mut render_pass, &paint_jobs, &screen_descriptor);
    }

    graphics
        .queue
        .submit(egui_command_buffers.into_iter().chain([encoder.finish()]));
    graphics.queue.present(output);

    // The frame we just submitted might still use them, so we only free them now.
    for id in egui_output.textures_delta.free.drain() {
        graphics.egui_render_pass.free_texture(&id);
    }

    Some(egui_output.platform_output)
}

#[derive(Unique)]
pub(crate) struct FrameTime(pub(crate) f32);

fn show_bunny_info(
    context: &egui::Context,
    bunny_count: usize,
    frame_time: f32,
    bunnies_per_click: &mut u32,
) {
    egui::Area::new(egui::Id::unique("Bynny Info"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .movable(false)
        .show(context, |ui| {
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
}
