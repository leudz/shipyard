use shipyard::*;

#[test]
fn entity_id_serde() {
    let world = World::default();

    // create and check a couple entities
    let (entity_id0, _) = world
        .run(
            |(mut entities, mut u32s): (EntitiesViewMut, ViewMut<u32>)| {
                let entity_id0 = entities.add_entity(&mut u32s, 0);
                check_roundtrip(entity_id0, "[0,0]");

                let entity_id1 = entities.add_entity(&mut u32s, 1);
                check_roundtrip(entity_id1, "[1,0]");

                (entity_id0, entity_id1)
            },
        )
        .unwrap();

    // delete the first entity
    world
        .run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete_entity(entity_id0));
        })
        .unwrap();

    // add 2 more
    world
        .run(
            |(mut entities, mut u32s): (EntitiesViewMut, ViewMut<u32>)| {
                let entity_id2 = entities.add_entity(&mut u32s, 2);
                // generation was bumped
                check_roundtrip(entity_id2, "[0,1]");

                let entity_id3 = entities.add_entity(&mut u32s, 1);
                check_roundtrip(entity_id3, "[2,0]");
            },
        )
        .unwrap();
}

fn check_roundtrip(entity_id: EntityId, expected: &str) {
    assert_eq!(expected, serde_json::to_string(&entity_id).unwrap());
    let new_entity_id: EntityId = serde_json::from_str(expected).unwrap();
    assert_eq!(entity_id, new_entity_id);
}
