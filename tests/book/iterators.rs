use super::{Pos, Vel};
use shipyard::{IntoIter, View, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let world = World::new();

for (i, j) in &mut world.iter::<(&mut Pos, &Vel)>() {
    i.0 += j.0;
}
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn iter() {
// ANCHOR: iter
let world = World::new();

world.run(|mut vm_pos: ViewMut<Pos>, v_vel: View<Vel>| {
    for i in vm_pos.iter() {
        dbg!(i);
    }
    
    for (i, j) in (&mut vm_pos, &v_vel).iter() {
        i.0 += j.0;
    }
});
// ANCHOR_END: iter
}

#[test]
#[rustfmt::skip]
fn with_id() {
// ANCHOR: with_id
let world = World::new();

world.run(|v_pos: View<Pos>| {
    for (id, i) in v_pos.iter().with_id() {
        println!("{:?} belongs to entity {:?}", i, id);
    }
});
// ANCHOR_END: with_id
}

#[test]
#[rustfmt::skip]
fn not() {
// ANCHOR: not
let world = World::new();

world.run(|v_pos: View<Pos>, v_vel: View<Vel>| {
    for (i, _) in (&v_pos, !&v_vel).iter() {
        dbg!(i);
    }
});
// ANCHOR_END: not
}
