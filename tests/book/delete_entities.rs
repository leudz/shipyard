use shipyard::*;

#[test]
fn test() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world
        .run(|mut all_storages: AllStoragesViewMut| {
            all_storages.delete_entity(entity_id);
        })
        .unwrap();
}
