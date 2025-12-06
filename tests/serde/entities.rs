use shipyard::{EntitiesView, EntityId, World};

#[test]
fn entities_view_serialize() {
    let mut world = World::new();

    let eid1 = world.add_entity(());
    let eid2 = world.add_entity(());

    world.run(|entities: EntitiesView| {
        let serialized = serde_json::to_string(&entities).unwrap();

        assert_eq!(serialized, r#"[{"index":0,"gen":0},{"index":1,"gen":0}]"#);

        let deserialized: Vec<EntityId> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.len(), 2);

        assert_eq!(deserialized[0], eid1);
        assert_eq!(deserialized[1], eid2);
    });
}
