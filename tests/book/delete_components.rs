use super::{U32, USIZE};
use shipyard::{AllStoragesViewMut, Delete, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((U32(0), USIZE(1)));

world.delete_component::<U32>(id);
world.delete_component::<(U32, USIZE)>(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn world_all() {
// ANCHOR: world_all
let mut world = World::new();

let id = world.add_entity((U32(0), USIZE(1)));

world.strip(id);
// ANCHOR_END: world_all
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut usizes: ViewMut<USIZE>| {
        let id = entities.add_entity((&mut u32s, &mut usizes), (U32(0), USIZE(1)));

        u32s.delete(id);
        (&mut u32s, &mut usizes).delete(id);
    },
);
// ANCHOR_END: view
}

#[test]
#[rustfmt::skip]
fn view_all() {
// ANCHOR: view_all
let world = World::new();

world.run(|mut all_storages: AllStoragesViewMut| {
    let id = all_storages.add_entity((U32(0), USIZE(1)));

    all_storages.strip(id);
});
// ANCHOR_END: view_all
}
