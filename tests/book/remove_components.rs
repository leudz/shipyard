use shipyard::{EntitiesViewMut, Remove, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world_one() {
// ANCHOR: world_one
let mut world = World::new();

let id = world.add_entity((0u32,));

world.remove::<(u32,)>(id);
// ANCHOR_END: world_one
}

#[test]
#[rustfmt::skip]
fn world_multiple() {
// ANCHOR: world_multiple
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.remove::<(u32, usize)>(id);
// ANCHOR_END: world_multiple
}

#[test]
#[rustfmt::skip]
fn view_one() {
// ANCHOR: view_one
let world = World::new();

let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>().unwrap();

let id = entities.add_entity(&mut u32s, 0);

u32s.remove(id);
// ANCHOR_END: view_one
}

#[test]
#[rustfmt::skip]
fn view_multiple() {
// ANCHOR: view_multiple
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let id = entities.add_entity((&mut u32s, &mut usizes), (0, 1));

(&mut u32s, &mut usizes).remove(id);
// ANCHOR_END: view_multiple
}
