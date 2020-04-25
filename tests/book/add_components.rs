use shipyard::*;

#[test]
fn single() {
    let world = World::new();

    let entity_id = world.borrow::<EntitiesViewMut>().add_entity((), ());

    world.run(|entities: EntitiesView, mut u32s: ViewMut<u32>| {
        entities.add_component(&mut u32s, 0, entity_id);
    });
}

#[test]
fn multiple() {
    let world = World::new();

    let entity_id = world.borrow::<EntitiesViewMut>().add_entity((), ());

    world.run(
        |entities: EntitiesView, mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
            entities.add_component((&mut u32s, &mut usizes), (0, 10), entity_id);
        },
    );
}
