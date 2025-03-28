use shipyard::{
    atomic_refcell::SharedBorrow, tracking::TrackingTimestamp, AllStorages, Borrow, BorrowInfo,
    Component, EntitiesViewMut, IntoIter, Unique, UniqueView, ViewMut, World,
};
use std::iter;

#[derive(Component)]
struct Parent;
#[derive(Component)]
struct Child;

// ANCHOR: view_bundle
#[derive(Borrow, BorrowInfo)]
struct Hierarchy<'v> {
    entities: EntitiesViewMut<'v>,
    parents: ViewMut<'v, Parent>,
    children: ViewMut<'v, Child>,
}
// ANCHOR_END: view_bundle

// ANCHOR: wild_view
struct RandomNumber(u64);
// ANCHOR_END: wild_view

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view() {
#[derive(Component)]
struct Parent;
#[derive(Component)]
struct Child;

// ANCHOR: into_iter
#[derive(Borrow, BorrowInfo, IntoIter)]
#[shipyard(item_name = "Node")]
struct Hierarchy<'v> {
    #[shipyard(item_field_skip)]
    entities: EntitiesViewMut<'v>,
    #[shipyard(item_field_name = "parent")]
    parents: ViewMut<'v, Parent>,
    #[shipyard(item_field_name = "child")]
    children: ViewMut<'v, Child>,
}

let world = World::new();

world.run(|mut hierarchy: Hierarchy| {
    for Node { parent, child } in hierarchy.iter() {
    }
});
// ANCHOR_END: into_iter
}

// we don't want to actually import wgpu for tests
mod wgpu {
    use std::{error::Error, fmt::Display};

    pub(crate) struct Surface;
    impl Surface {
        pub(crate) fn get_current_texture(&self) -> Result<SurfaceTexture, SurfaceError> {
            unreachable!()
        }
    }

    pub(crate) struct Device;
    impl Device {
        pub(crate) fn create_command_encoder(
            &self,
            _: &CommandEncoderDescriptor,
        ) -> CommandEncoder {
            unreachable!()
        }
    }
    pub(crate) struct Queue;
    impl Queue {
        pub(crate) fn submit(&self, iter: impl IntoIterator<Item = ()>) {
            unreachable!()
        }
    }
    pub(crate) struct SurfaceConfiguration;
    #[derive(Debug)]
    pub(crate) struct SurfaceError;
    impl Display for SurfaceError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            unreachable!()
        }
    }
    impl Error for SurfaceError {}

    pub(crate) struct SurfaceTexture {
        pub(crate) texture: Texture,
    }
    impl SurfaceTexture {
        pub(crate) fn present(&self) {
            unreachable!()
        }
    }
    pub(crate) struct Texture;
    impl Texture {
        pub(crate) fn create_view(&self, desc: &TextureViewDescriptor) -> TextureView {
            unreachable!()
        }
    }

    #[derive(Default)]
    pub(crate) struct TextureViewDescriptor;
    pub(crate) struct TextureView;
    pub(crate) struct CommandEncoderDescriptor {
        pub(crate) label: Option<&'static str>,
    }
    pub(crate) struct CommandEncoder;
    impl CommandEncoder {
        pub(crate) fn begin_render_pass(&self, _: &RenderPassDescriptor) -> RenderPass {
            unreachable!()
        }
        pub(crate) fn finish(self) {
            unreachable!()
        }
    }

    pub(crate) struct RenderPassDescriptor<'desc> {
        pub(crate) label: Option<&'static str>,
        pub(crate) color_attachments: &'desc [RenderPassColorAttachment<'desc>],
        pub(crate) depth_stencil_attachment: Option<()>,
    }
    pub(crate) struct RenderPass;
    pub(crate) struct RenderPassColorAttachment<'tex> {
        pub(crate) view: &'tex TextureView,
        pub(crate) resolve_target: Option<&'tex TextureView>,
        pub(crate) ops: Operations<Color>,
    }
    pub(crate) struct Operations<T> {
        pub(crate) load: LoadOp<T>,
        pub(crate) store: bool,
    }
    pub(crate) enum LoadOp<T> {
        Clear(T),
    }
    pub(crate) struct Color {
        pub(crate) r: f32,
        pub(crate) g: f32,
        pub(crate) b: f32,
        pub(crate) a: f32,
    }
}

// we don't want to actually import winit for tests
mod winit {
    pub(crate) mod dpi {
        pub(crate) struct PhysicalSize<T>(T);
    }
}

// ANCHOR: original
#[derive(Unique)]
struct Graphics {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
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
// ANCHOR_END: original

#[rustfmt::skip]
mod render {
    use super::wgpu;
    use super::RenderGraphicsViewMut;

// ANCHOR: render
fn render(mut graphics: RenderGraphicsViewMut) {
    let mut _render_pass = graphics
        .encoder
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &graphics.view,
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
// ANCHOR_END: render
}

// ANCHOR: custom_view
struct RenderGraphicsViewMut {
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
}
// ANCHOR_END: custom_view

// ANCHOR: borrow
impl shipyard::borrow::Borrow for RenderGraphicsViewMut {
    type View<'v> = RenderGraphicsViewMut;

    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, shipyard::error::GetStorage> {
        // Even if we don't use tracking for Graphics, it's good to build an habit of using last_run and current when creating custom views
        let graphics =
            UniqueView::<Graphics>::borrow(&all_storages, all_borrow, last_run, current)?;
        // This error will now be reported as an error during the view creation process and not the system but is still bubbled up
        let output = graphics
            .surface
            .get_current_texture()
            .map_err(shipyard::error::GetStorage::from_custom)?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = graphics
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        Ok(RenderGraphicsViewMut { encoder, view })
    }
}
// ANCHOR_END: borrow

#[rustfmt::skip]
mod view {
    use super::{wgpu, UniqueView, Graphics, AllStorages, SharedBorrow, TrackingTimestamp};
    use std::iter;

// ANCHOR: custom_view_full
struct RenderGraphicsViewMut<'v> {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    // New fields
    output: Option<wgpu::SurfaceTexture>,
    graphics: UniqueView<'v, Graphics>,
}
// ANCHOR_END: custom_view_full

// ANCHOR: borrow_revisit
impl shipyard::borrow::Borrow for RenderGraphicsViewMut<'_> {
    type View<'v> = RenderGraphicsViewMut<'v>;

    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, shipyard::error::GetStorage> {
        // Even if we don't use tracking for Graphics, it's good to build an habit of using last_run and current when creating custom views
        let graphics =
            UniqueView::<Graphics>::borrow(&all_storages, all_borrow, last_run, current)?;
        // This error will now be reported as an error during the view creation process and not the system but is still bubbled up
        let output = graphics
            .surface
            .get_current_texture()
            .map_err(shipyard::error::GetStorage::from_custom)?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = graphics
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        Ok(RenderGraphicsViewMut {
            encoder,
            view,
            output: Some(output),
            graphics,
        })
    }
}

impl Drop for RenderGraphicsViewMut<'_> {
    fn drop(&mut self) {
        // I chose to swap here to not have to use an `Option<wgpu::CommandEncoder>` in a publicly accessible field
        let encoder = std::mem::replace(
            &mut self.encoder,
            self.graphics
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                }),
        );

        self.graphics.queue.submit(iter::once(encoder.finish()));
        // output on the other hand is only used here so an `Option` is good enough
        self.output.take().unwrap().present();
    }
}
// ANCHOR_END: borrow_revisit

// ANCHOR: borrow_info
// SAFE: All storages info is recorded.
unsafe impl shipyard::borrow::BorrowInfo for RenderGraphicsViewMut<'_> {
    fn borrow_info(info: &mut Vec<shipyard::scheduler::info::TypeInfo>) {
        <UniqueView<Graphics>>::borrow_info(info);
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), shipyard::error::GetStorage>>,
    ) {
        // We only have a single UniqueView so no tracking
    }
}
// ANCHOR_END: borrow_info
}
