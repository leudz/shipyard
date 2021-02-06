use shipyard::{EntitiesViewMut, Remove, ViewMut, World};

#[test]
#[rustfmt::skip]
fn world() {
// ANCHOR: world
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.remove::<(u32,)>(id);
world.remove::<(u32, usize)>(id);
// ANCHOR_END: world
}

#[test]
#[rustfmt::skip]
fn view() {
// ANCHOR: view
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let id = entities.add_entity((&mut u32s, &mut usizes), (0, 1));

u32s.remove(id);
(&mut u32s, &mut usizes).remove(id);
// ANCHOR_END: view
}
