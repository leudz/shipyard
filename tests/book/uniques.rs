use shipyard::*;

#[derive(Unique)]
struct Camera;

impl Camera {
    fn new() -> Self {
        Camera
    }
}

#[rustfmt::skip]
#[allow(unused)]
fn uniques() {
// ANCHOR: uniques
let world = World::new();

world.add_unique(Camera::new());

world
    .run(|camera: UniqueView<Camera>| {
        // -- snip --
    });
// ANCHOR_END: uniques
}
