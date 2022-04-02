use super::{U32, USIZE};
use shipyard::{AddComponent, EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, (U32(0),));
world.add_component(id, (U32(0), USIZE(1)));
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut usizes: ViewMut<USIZE>| {
        let id = entities.add_entity((), ());

        entities.add_component(id, &mut u32s, U32(0));
        entities.add_component(id, (&mut u32s, &mut usizes), (U32(0), USIZE(1)));
        u32s.add_component_unchecked(id, U32(0));
    },
);
// ANCHOR_END: view
}
