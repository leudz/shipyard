use super::{F32, U32};
use shipyard::*;

#[rustfmt::skip]
#[allow(unused)]
#[test]
fn insertion() {
// ANCHOR: insertion
let mut world = World::new();

let entity0 = world.add_entity((U32(0),));
let entity1 = world.add_entity((F32(10.0),));
let entity2 = world.add_entity((U32(20),));
// ANCHOR_END: insertion

// ANCHOR: removal
world.remove::<(U32,)>(entity0);
// ANCHOR_END: removal
}
