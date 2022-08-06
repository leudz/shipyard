use super::{Pos, Vel};
use shipyard::{AllStoragesViewMut, Delete, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.delete_component::<Vel>(id);
world.delete_component::<(Pos, Vel)>(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn world_all() {
// ANCHOR: world_all
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.strip(id);
// ANCHOR_END: world_all
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let id = entities.add_entity((&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));

        vm_pos.delete(id);
        (&mut vm_pos, &mut vm_vel).delete(id);
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
    let id = all_storages.add_entity((Pos::new(), Vel::new()));

    all_storages.strip(id);
});
// ANCHOR_END: view_all
}
