use shipyard::*;

#[derive(Component)]
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

world.add_unique(Camera::new()).unwrap();

world
    .run(|camera: UniqueView<Camera>| {
        // -- snip --
    })
    .unwrap();
// ANCHOR_END: uniques
}
