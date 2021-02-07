use shipyard::{IntoIter, IntoWithId, View, ViewMut, World};

#[test]
#[rustfmt::skip]
fn iter() {
// ANCHOR: iter
let world = World::new();

let (mut u32s, usizes) = world.borrow::<(ViewMut<u32>, View<usize>)>().unwrap();

for i in u32s.iter() {
    dbg!(i);
}

for (mut i, j) in (&mut u32s, &usizes).iter() {
    *i += *j as u32;
}
// ANCHOR_END: iter
}

#[test]
#[rustfmt::skip]
fn with_id() {
// ANCHOR: with_id
let world = World::new();

let u32s = world.borrow::<View<u32>>().unwrap();

for (id, i) in u32s.iter().with_id() {
    println!("{} belongs to entity {:?}", i, id);
}
// ANCHOR_END: with_id
}

#[test]
#[rustfmt::skip]
fn not() {
// ANCHOR: not
let world = World::new();

let (u32s, usizes) = world.borrow::<(View<u32>, View<usize>)>().unwrap();

for (i, _) in (&u32s, !&usizes).iter() {
    dbg!(i);
}
// ANCHOR_END: not
}
