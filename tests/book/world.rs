use shipyard::*;

#[test]
#[allow(unused)]
#[rustfmt::skip]
fn new() {
// ANCHOR: world_new
let world = World::default();
// or
let world = World::new();
// ANCHOR_END: world_new
}

#[test]
#[allow(unused)]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(|entities: EntitiesView| {});
// ANCHOR_END: view
}
