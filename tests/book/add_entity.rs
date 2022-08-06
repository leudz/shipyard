use super::{Pos, Vel};
use shipyard::{EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn world() {
// ANCHOR: world
let mut world = World::new();

let empty_entity = world.add_entity(());
let single_component = world.add_entity(Pos::new());
let multiple_components = world.add_entity((Pos::new(), Vel::new()));
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let empty_entity = entities.add_entity((), ());
        let single_component = entities.add_entity(&mut vm_pos, Pos::new());
        let multiple_components =
            entities.add_entity((&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));
    },
);
// ANCHOR_END: view
}
