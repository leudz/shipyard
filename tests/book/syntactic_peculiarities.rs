use super::{Pos, Vel};
use shipyard::*;

#[allow(unused)]
#[allow(clippy::toplevel_ref_arg)]
fn ref_mut(ref mut vm_pos: ViewMut<Pos>) {
    let id = EntityId::dead();
    let pos: Mut<Pos> = vm_pos.get(id).unwrap();
}

#[allow(unused)]
#[allow(clippy::toplevel_ref_arg)]
fn ref_sys(ref v_pos: View<Pos>, ref v_vel: View<Vel>) {
    let id = EntityId::dead();
    (v_pos, v_vel).get(id).unwrap();
    (v_pos, v_vel).get(id).unwrap();
    (v_pos, v_vel).get(id).unwrap();
    (v_pos, v_vel).get(id).unwrap();
}
