use super::{U32, USIZE};
use shipyard::{EntitiesViewMut, Remove, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((U32(0), USIZE(1)));

world.remove::<(U32,)>(id);
world.remove::<(U32, USIZE)>(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut usizes: ViewMut<USIZE>| {
        let id = entities.add_entity((&mut u32s, &mut usizes), (U32(0), USIZE(1)));

        u32s.remove(id);
        (&mut u32s, &mut usizes).remove(id);
    },
);
// ANCHOR_END: view
}
