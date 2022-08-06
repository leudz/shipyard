use super::{Pos, Vel};
use shipyard::{EntitiesViewMut, Remove, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.remove::<Vel>(id);
world.remove::<(Pos, Vel)>(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let id = entities.add_entity((&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));

        vm_pos.remove(id);
        (&mut vm_pos, &mut vm_vel).remove(id);
    },
);
// ANCHOR_END: view
}
