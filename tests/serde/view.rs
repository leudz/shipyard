use shipyard::{Component, EntityId, View, World};

#[derive(Component, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Player {
    name: String,
    score: u32,
}

#[test]
fn view_serialize() {
    let mut world = World::new();

    let player_alice = Player {
        name: "Alice".to_string(),
        score: 100,
    };
    let player_bob = Player {
        name: "Bob".to_string(),
        score: 250,
    };

    let eid1 = world.add_entity(player_alice.clone());
    let eid2 = world.add_entity(player_bob.clone());

    world.run(|view: View<Player>| {
        let serialized = serde_json::to_string(&view).unwrap();

        assert_eq!(
            serialized,
            "[\
                [\
                    {\"index\":0,\"gen\":0},\
                    {\"name\":\"Alice\",\"score\":100}\
                ],\
                [\
                    {\"index\":1,\"gen\":0},\
                    {\"name\":\"Bob\",\"score\":250}\
                ]\
            ]"
        );

        let deserialized: Vec<(EntityId, Player)> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.len(), 2);

        assert_eq!(deserialized[0], (eid1, player_alice));
        assert_eq!(deserialized[1], (eid2, player_bob));
    });
}
