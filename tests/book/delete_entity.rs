use super::Pos;
use shipyard::{AllStoragesViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity(Pos::new());

world.delete_entity(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(|mut all_storages: AllStoragesViewMut| {
    let id = all_storages.add_entity(Pos::new());

    all_storages.delete_entity(id);
});
// ANCHOR_END: view
}
