use shipyard::{AllStoragesViewMut, Delete, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.delete_component::<(u32,)>(id);
world.delete_component::<(u32, usize)>(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn world_all() {
// ANCHOR: world_all
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.strip(id);
// ANCHOR_END: world_all
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let id = entities.add_entity((&mut u32s, &mut usizes), (0, 1));

u32s.delete(id);
(&mut u32s, &mut usizes).delete(id);
// ANCHOR_END: view
}

#[test]
#[rustfmt::skip]
fn view_all() {
// ANCHOR: view_all
let world = World::new();

let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let id = all_storages.add_entity((0u32, 1usize));

all_storages.strip(id);
// ANCHOR_END: view_all
}
