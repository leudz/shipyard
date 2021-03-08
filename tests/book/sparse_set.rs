use shipyard::*;

#[rustfmt::skip]
#[allow(unused)]
#[test]
fn insertion() {
// ANCHOR: insertion
let mut world = World::new();

let entity0 = world.add_entity((0u32,));
let entity1 = world.add_entity((10.0f32,));
let entity2 = world.add_entity((20u32,));
// ANCHOR_END: insertion

// ANCHOR: removal
world.remove::<(u32,)>(entity0);
// ANCHOR_END: removal
}
