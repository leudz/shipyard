use shipyard::*;

#[rustfmt::skip]
#[allow(unused)]
#[test]
fn insertion() {
// ANCHOR: insertion
#[derive(Component)]
struct FirstComponent(pub u32);

#[derive(Component)]
struct SecondComponent(pub u32);

let mut world = World::new();

let entity_id_0 = world.add_entity((FirstComponent(322),));
let entity_id_1 = world.add_entity((SecondComponent(17),));
let entity_id_2 = world.add_entity((FirstComponent(5050), SecondComponent(3154)));
let entity_id_3 = world.add_entity((FirstComponent(958),));
// ANCHOR_END: insertion

// ANCHOR: iteration
let (firsts, seconds) = world
	.borrow::<(View<FirstComponent>, View<SecondComponent>)>()
	.unwrap();

for (first, second) in (&firsts, &seconds).iter() {
	// Do some stuff
}
// ANCHOR_END: iteration
drop(firsts);
drop(seconds);

// ANCHOR: removal
world.remove::<(FirstComponent,)>(entity_id_0);
// ANCHOR_END: removal
}
