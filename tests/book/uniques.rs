use shipyard::*;

#[rustfmt::skip]
#[allow(unused)]
fn unique_declare_derive() {
// ANCHOR: unique_declare_derive
// Using a derive macro
#[derive(Unique)]
struct Camera;
// ANCHOR_END: unique_declare_derive
}

#[rustfmt::skip]
#[allow(unused)]
fn unique_declare_manual() {
// ANCHOR: unique_declare_manual
// By manually implementing the trait
struct Camera;
impl Unique for Camera {}
// ANCHOR_END: unique_declare_manual
}

#[rustfmt::skip]
#[allow(unused)]
fn uniques() {
#[derive(Unique)]
struct Camera;

impl Camera {
    fn new() -> Self {
        Camera
    }
}
// ANCHOR: uniques
let world = World::new();

world.add_unique(Camera::new());

world
    .run(|camera: UniqueView<Camera>| {
        // -- snip --
    });
// ANCHOR_END: uniques
}
