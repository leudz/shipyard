use super::{U32, USIZE};
use shipyard::{EntitiesViewMut, ViewMut, World};

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn world() {
// ANCHOR: world
let mut world = World::new();

let empty_entity = world.add_entity(());
let single_component = world.add_entity(U32(0));
let multiple_components = world.add_entity((U32(0), USIZE(1)));
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut usizes: ViewMut<USIZE>| {
        let empty_entity = entities.add_entity((), ());
        let single_component = entities.add_entity(&mut u32s, U32(0));
        let multiple_components =
            entities.add_entity((&mut u32s, &mut usizes), (U32(0), USIZE(1)));
    },
);
// ANCHOR_END: view
}
