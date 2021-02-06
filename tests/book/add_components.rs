use shipyard::{AddComponent, EntitiesView, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, (0u32,));
world.add_component(id, (0u32, 1usize));
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

let id = world
    .borrow::<EntitiesViewMut>()
    .unwrap()
    .add_entity((), ());

let (entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesView, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

entities.add_component(id, &mut u32s, 0);
entities.add_component(id, (&mut u32s, &mut usizes), (0, 1));
u32s.add_component_unchecked(id, 0);
// ANCHOR_END: view
}
