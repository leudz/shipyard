use super::U32;
use shipyard::{AllStoragesViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((U32(0),));

world.delete_entity(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let id = all_storages.add_entity((U32(0),));

all_storages.delete_entity(id);
// ANCHOR_END: view
}
