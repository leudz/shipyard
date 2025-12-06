use shipyard::{EntitiesViewMut, EntityId, World};

#[test]
fn entities_view_mut_deserialize_in_place() {
    let world = World::new();

    let eid1 = EntityId::new_from_index_and_gen(0, 0);
    let eid2 = EntityId::new_from_index_and_gen(1, 0);

    world.run(|mut entities: EntitiesViewMut| {
        let serialized = "[\
            {\"index\":1,\"gen\":0},\
            {\"index\":0,\"gen\":0}\
        ]";

        let mut deserializer = serde_json::Deserializer::from_str(serialized);
        serde::Deserialize::deserialize_in_place(&mut deserializer, &mut entities).unwrap();
    });

    world.run(|entities: EntitiesViewMut| {
        assert_eq!(entities.iter().count(), 2);

        assert!(entities.is_alive(eid1));
        assert!(entities.is_alive(eid2));
    });
}

#[test]
#[should_panic(
    expected = "EntitiesViewMut cannot be directly deserialized. Use deserialize_in_place instead."
)]
fn entities_view_mut_direct_deserialize_panic() {
    serde_json::from_str::<EntitiesViewMut>("").unwrap();
}
