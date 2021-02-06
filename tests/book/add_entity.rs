use shipyard::{EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn world() {
// ANCHOR: world
let mut world = World::new();

let empty_entity = world.add_entity(());
let single_component = world.add_entity((0u32,));
let multiple_components = world.add_entity((0u32, 1usize));
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view() {
// ANCHOR: view
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let empty_entity = entities.add_entity((), ());
let single_component = entities.add_entity(&mut u32s, 0);
let multiple_components = entities.add_entity((&mut u32s, &mut usizes), (0, 1));
// ANCHOR_END: view
}
