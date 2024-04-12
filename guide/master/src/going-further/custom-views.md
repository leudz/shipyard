# Custom Views

Custom views are types that you can borrow (like `View` or `UniqueView`) but are not provided by `shipyard`.

Many types can become custom views, they'll fall into one of two categories: View Bundle or Wild View.
View bundles only contain other views while wild views can contain other types.

Example of a View Bundle:
```rust, noplaypen
struct Hierarchy<'v> {
    entities: EntitiesViewMut<'v>,
    parents: ViewMut<'v, Parent>,
    children: ViewMut<'v, Child>,
}
```

Example of a Wild View:
```rust, noplaypen
struct RandomNumber(u64);
```

### Concrete example

When creating a frame with any low level api there is always some boilerplate. We'll look at how custom views can help for `wgpu`.

The original code creates the frame in a system by borrowing `Graphics` which contains everything needed.
The rendering part just clears the screen with a color.

The entire starting code for this chapter is available in [this file](./custom_views_original.rs). You can copy all of it in a fresh `main.rs` and edit the fresh `Cargo.toml`.

<details>
<summary>Original</summary>

```rust, noplaypen
#[derive(Unique)]
struct Graphics {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u64>,
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
```
</details>

We want to abstract the beginning and end of the system to get this version working.
The error handling is going to move, we could keep it closer to the original by having a `ResultRenderGraphicsViewMut` for example. 

```rust, noplaypen
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
```

We'll start by creating a struct to hold our init state.

```rust, noplaypen
struct RenderGraphicsViewMut {
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
}
```

Now let's make this struct able to be borrowed and generate the initial state we need.

```rust, noplaypen
impl<'v> shipyard::Borrow<'v> for RenderGraphicsViewMut {
    type View = RenderGraphicsViewMut;

    fn borrow(
        world: &'v shipyard::World,
        last_run: Option<u64>,
        current: u64,
    ) -> Result<Self::View, shipyard::error::GetStorage> {
        // Even if we don't use tracking for Graphics, it's good to build an habit of using last_run and current when creating custom views
        let graphics = <UniqueView<Graphics> as shipyard::IntoBorrow>::Borrow::borrow(
            world, last_run, current,
        )?;
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
        })
    }
}
```

We now have a custom view! We can't change our system just yet, we're missing `output`.

Let's add `output` and `graphics` to our custom view.

```rust, noplaypen
struct RenderGraphicsViewMut<'v> {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    // New fields
    output: Option<wgpu::SurfaceTexture>,
    graphics: UniqueView<'v, Graphics>,
}
```

Since our view now has a lifetime we need a bit of boilerplate ([explanation](../going-deeper/workload-creation.md)).

```rust, noplaypen
struct RenderGraphicsBorrower {}

impl shipyard::IntoBorrow for RenderGraphicsViewMut<'_> {
    type Borrow = RenderGraphicsBorrower;
}
```

With that our of the way we can revisit our `Borrow` implementation and add one for `Drop`.

```rust, noplaypen
impl<'v> shipyard::Borrow<'v> for RenderGraphicsBorrower {
    type View = RenderGraphicsViewMut<'v>;

    fn borrow(
        world: &'v shipyard::World,
        last_run: Option<u64>,
        current: u64,
    ) -> Result<Self::View, shipyard::error::GetStorage> {
        let graphics = <UniqueView<Graphics> as shipyard::IntoBorrow>::Borrow::borrow(
            world, last_run, current,
        )?;
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
```

Our custom view is now fully functional and we successfully moved code that would be duplicated out of the render system.
You can remove the error handling in `main.rs` to see the result.

As a final touch we can implement `BorrowInfo` and `AllStoragesBorrow`. Respectively to make our view work with workloads and `AllStorages`.

```rust, noplaypen
// SAFE: All storages info is recorded.
unsafe impl shipyard::BorrowInfo for RenderGraphicsViewMut<'_> {
    fn borrow_info(info: &mut Vec<shipyard::info::TypeInfo>) {
        <UniqueView<Graphics>>::borrow_info(info);
    }
}

impl<'v> shipyard::AllStoragesBorrow<'v> for RenderGraphicsBorrower {
    fn all_borrow(
        all_storages: &'v shipyard::AllStorages,
        last_run: Option<u64>,
        current: u64,
    ) -> Result<Self::View, shipyard::error::GetStorage> {
        let graphics = <UniqueView<Graphics> as shipyard::IntoBorrow>::Borrow::all_borrow(
            all_storages,
            last_run,
            current,
        )?;
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
```
