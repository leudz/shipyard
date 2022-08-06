use super::{Pos, Vel};
use shipyard::{Get, ViewMut, World};

#[test]
#[rustfmt::skip]
fn get() {
// ANCHOR: get
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.run(|mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
    (&mut vm_vel).get(id).unwrap().0 += 1.0;

    let (mut i, j) = (&mut vm_pos, &vm_vel).get(id).unwrap();
    i.0 += j.0;

    vm_pos[id].0 += 1.0;
});
// ANCHOR_END: get
}
