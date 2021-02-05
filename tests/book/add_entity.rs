use shipyard::{EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn world_empty() {
// ANCHOR: world_empty
let mut world = World::new();

let id = world.add_entity(());
// ANCHOR_END: world_empty
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn world_one() {
// ANCHOR: world_one
let mut world = World::new();

let id = world.add_entity((0u32,));
// ANCHOR_END: world_one
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn world_multiple() {
// ANCHOR: world_multiple
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));
// ANCHOR_END: world_multiple
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view_empty() {
// ANCHOR: view_empty
let world = World::new();

let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

let id = entities.add_entity((), ());
// ANCHOR_END: view_empty
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view_one() {
// ANCHOR: view_one
let world = World::new();

let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>().unwrap();

let id = entities.add_entity(&mut u32s, 0);
// ANCHOR_END: view_one
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view_multiple() {
// ANCHOR: view_multiple
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let id = entities.add_entity((&mut u32s, &mut usizes), (0, 1));
// ANCHOR_END: view_multiple
}
