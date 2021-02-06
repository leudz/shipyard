use shipyard::{Get, ViewMut, World};

#[test]
#[rustfmt::skip]
fn get() {
// ANCHOR: get
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

let (mut u32s, mut usizes) = world.borrow::<(ViewMut<u32>, ViewMut<usize>)>().unwrap();

*(&mut usizes).get(id).unwrap() += 1;

let (mut i, j) = (&mut u32s, &usizes).get(id).unwrap();
*i += *j as u32;

u32s[id] += 1;
// ANCHOR_END: get
}
