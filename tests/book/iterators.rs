use super::{U32, USIZE};
use shipyard::{IntoIter, IntoWithId, View, ViewMut, World};

#[test]
#[rustfmt::skip]
fn iter() {
// ANCHOR: iter
let world = World::new();

let (mut u32s, usizes) = world.borrow::<(ViewMut<U32>, View<USIZE>)>().unwrap();

for i in u32s.iter() {
    dbg!(i);
}

for (mut i, j) in (&mut u32s, &usizes).iter() {
    i.0 += j.0 as u32;
}
// ANCHOR_END: iter
}

#[test]
#[rustfmt::skip]
fn with_id() {
// ANCHOR: with_id
let world = World::new();

let u32s = world.borrow::<View<U32>>().unwrap();

for (id, i) in u32s.iter().with_id() {
    println!("{:?} belongs to entity {:?}", i, id);
}
// ANCHOR_END: with_id
}

#[test]
#[rustfmt::skip]
fn not() {
// ANCHOR: not
let world = World::new();

let (u32s, usizes) = world.borrow::<(View<U32>, View<USIZE>)>().unwrap();

for (i, _) in (&u32s, !&usizes).iter() {
    dbg!(i);
}
// ANCHOR_END: not
}
