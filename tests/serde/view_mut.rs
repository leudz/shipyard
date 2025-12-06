use shipyard::{Component, EntityId, ViewMut, World};

#[derive(Component, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Player {
    name: String,
    score: u32,
}

#[test]
fn view_mut_deserialize_in_place() {
    let world = World::new();

    let player_alice = Player {
        name: "Alice".to_string(),
        score: 100,
    };
    let player_bob = Player {
        name: "Bob".to_string(),
        score: 250,
    };

    world.run(|mut vm_player: ViewMut<Player>| {
        let serialized = "[\
            [\
                {\"index\":1,\"gen\":0},\
                {\"name\":\"Bob\",\"score\":250}\
            ],\
            [\
                {\"index\":0,\"gen\":0},\
                {\"name\":\"Alice\",\"score\":100}\
            ]\
        ]";

        // Use serde_json's Deserializer with the deserialize_in_place method
        let mut deserializer = serde_json::Deserializer::from_str(serialized);
        serde::Deserialize::deserialize_in_place(&mut deserializer, &mut vm_player).unwrap();
    });

    world.run(|vm_player: ViewMut<Player>| {
        assert_eq!(vm_player.len(), 2);

        assert_eq!(
            vm_player[EntityId::new_from_index_and_gen(0, 0)],
            player_alice
        );
        assert_eq!(
            vm_player[EntityId::new_from_index_and_gen(1, 0)],
            player_bob
        );
    });
}

#[test]
#[should_panic(
    expected = "ViewMut cannot be directly deserialized. Use deserialize_in_place instead."
)]
fn view_mut_direct_deserialize_panic() {
    serde_json::from_str::<ViewMut<Player>>("").unwrap();
}
