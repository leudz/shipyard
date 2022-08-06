use super::{Pos, Vel};
use shipyard::{AddComponent, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, Vel::new());
world.add_component(id, (Pos::new(), Vel::new()));
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let id = entities.add_entity((), ());

        entities.add_component(id, &mut vm_pos, Pos::new());
        entities.add_component(id, (&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));
        vm_vel.add_component_unchecked(id, Vel::new());
    },
);
// ANCHOR_END: view
}
