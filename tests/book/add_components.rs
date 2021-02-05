use shipyard::{EntitiesView, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world_one() {
// ANCHOR: world_one
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, (0u32,));
// ANCHOR_END: world_one
}

#[test]
#[rustfmt::skip]
fn world_multiple() {
// ANCHOR: world_multiple
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, (0u32, 1usize));
// ANCHOR_END: world_multiple
}

#[test]
#[rustfmt::skip]
fn view_one() {
// ANCHOR: view_one
let world = World::new();

let id = world
    .borrow::<EntitiesViewMut>()
    .unwrap()
    .add_entity((), ());

let (entities, mut u32s) = world.borrow::<(EntitiesView, ViewMut<u32>)>().unwrap();

entities.add_component(id, &mut u32s, 0);
// ANCHOR_END: view_one
}

#[test]
#[rustfmt::skip]
fn view_multiple() {
// ANCHOR: view_multiple
let world = World::new();

let id = world
    .borrow::<EntitiesViewMut>()
    .unwrap()
    .add_entity((), ());

let (entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesView, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

entities.add_component(id, (&mut u32s, &mut usizes), (0, 1));
// ANCHOR_END: view_multiple
}
