use super::{U32, USIZE};
use shipyard::{Get, ViewMut, World};

#[test]
#[rustfmt::skip]
fn get() {
// ANCHOR: get
let mut world = World::new();

let id = world.add_entity((U32(0), USIZE(1)));

let (mut u32s, mut usizes) = world.borrow::<(ViewMut<U32>, ViewMut<USIZE>)>().unwrap();

(&mut usizes).get(id).unwrap().0 += 1;

let (mut i, j) = (&mut u32s, &usizes).get(id).unwrap();
i.0 += j.0 as u32;

u32s[id].0 += 1;
// ANCHOR_END: get
}
