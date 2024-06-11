use shipyard::*;

struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn entity_id_serde() {
    let world = World::default();

    // create and check a couple entities
    let (entity_id0, _) = world.run(
        |(mut entities, mut u32s): (EntitiesViewMut, ViewMut<U32>)| {
            let entity_id0 = entities.add_entity(&mut u32s, U32(0));
            check_roundtrip(entity_id0, "{\"index\":0,\"gen\":0}");

            let entity_id1 = entities.add_entity(&mut u32s, U32(1));
            check_roundtrip(entity_id1, "{\"index\":1,\"gen\":0}");

            (entity_id0, entity_id1)
        },
    );

    // delete the first entity
    world.run(|mut all_storages: AllStoragesViewMut| {
        assert!(all_storages.delete_entity(entity_id0));
    });

    // add 2 more
    world.run(
        |(mut entities, mut u32s): (EntitiesViewMut, ViewMut<U32>)| {
            let entity_id2 = entities.add_entity(&mut u32s, U32(2));
            // generation was bumped
            check_roundtrip(entity_id2, "{\"index\":0,\"gen\":1}");

            let entity_id3 = entities.add_entity(&mut u32s, U32(1));
            check_roundtrip(entity_id3, "{\"index\":2,\"gen\":0}");
        },
    );
}

fn check_roundtrip(entity_id: EntityId, expected: &str) {
    assert_eq!(expected, serde_json::to_string(&entity_id).unwrap());
    let new_entity_id: EntityId = serde_json::from_str(expected).unwrap();
    assert_eq!(entity_id, new_entity_id);
}
