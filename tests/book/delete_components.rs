use shipyard::*;

#[test]
fn single() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world.run(|mut u32s: ViewMut<u32>| {
        u32s.delete(entity_id);
    });
}

#[test]
fn multiple() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world.run(|mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
        Delete::<(u32, usize)>::delete((&mut u32s, &mut usizes), entity_id);
    });
}

#[test]
fn strip() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world.run(|mut all_storages: AllStoragesViewMut| {
        all_storages.strip(entity_id);
    });
}
