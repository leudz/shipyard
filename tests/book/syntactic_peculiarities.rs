use super::{U32, USIZE};
use shipyard::*;

#[allow(unused)]
#[allow(clippy::toplevel_ref_arg)]
fn ref_mut(ref mut u32s: ViewMut<U32>) {
    let id = EntityId::dead();
    let i: &mut U32 = u32s.get(id).unwrap();
}

#[allow(unused)]
#[allow(clippy::toplevel_ref_arg)]
fn ref_sys(ref u32s: View<U32>, ref usizes: View<USIZE>) {
    let id = EntityId::dead();
    (usizes, u32s).get(id).unwrap();
    (usizes, u32s).get(id).unwrap();
    (usizes, u32s).get(id).unwrap();
    (usizes, u32s).get(id).unwrap();
}
