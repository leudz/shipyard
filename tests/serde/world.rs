use shipyard::{Component, EntityId, View, ViewMut, World, WorldBorrow};

#[derive(Component, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Player {
    name: String,
    score: u32,
}

#[derive(Component, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Location {
    x: i32,
    y: i32,
}

#[derive(WorldBorrow, serde::Serialize, serde::Deserialize)]
struct PlayerLocationView<'v> {
    #[serde(borrow)]
    vm_player: ViewMut<'v, Player>,
    #[serde(borrow)]
    vm_location: ViewMut<'v, Location>,
}

#[test]
fn world_serialize() {
    let mut world = World::new();

    let player_alice = Player {
        name: "Alice".to_string(),
        score: 100,
    };
    let player_bob = Player {
        name: "Bob".to_string(),
        score: 250,
    };
    let location_alice = Location { x: 10, y: 20 };
    let location_bob = Location { x: -5, y: 15 };

    world.add_entity((player_alice, location_alice));
    world.add_entity((player_bob, location_bob));

    let mut serialized_players_locations = Vec::new();
    world
        .serialize::<_, PlayerLocationView>(&mut serde_json::ser::Serializer::new(
            &mut serialized_players_locations,
        ))
        .unwrap_or_else(|_| panic!());

    let serialized_players_locations_str = String::from_utf8(serialized_players_locations).unwrap();
    assert_eq!(
        serialized_players_locations_str,
        "{\
            \"vm_player\":[\
                [\
                    {\"index\":0,\"gen\":0},\
                    {\"name\":\"Alice\",\"score\":100}\
                ],\
                [\
                    {\"index\":1,\"gen\":0},\
                    {\"name\":\"Bob\",\"score\":250}\
                ]\
            ],\
            \"vm_location\":[\
                [\
                    {\"index\":0,\"gen\":0},\
                    {\"x\":10,\"y\":20}\
                ],\
                [\
                    {\"index\":1,\"gen\":0},\
                    {\"x\":-5,\"y\":15}\
                ]\
            ]\
        }",
    );
}

#[test]
fn world_deserialize() {
    let world = World::new();

    let player_alice = Player {
        name: "Alice".to_string(),
        score: 100,
    };
    let player_bob = Player {
        name: "Bob".to_string(),
        score: 250,
    };
    let location_alice = Location { x: 10, y: 20 };
    let location_bob = Location { x: -5, y: 15 };

    let serialized_players_locations = "{\
        \"vm_player\":[\
            [\
                {\"index\":0,\"gen\":0},\
                {\"name\":\"Alice\",\"score\":100}\
            ],\
            [\
                {\"index\":1,\"gen\":0},\
                {\"name\":\"Bob\",\"score\":250}\
            ]\
        ],\
        \"vm_location\":[\
            [\
                {\"index\":0,\"gen\":0},\
                {\"x\":10,\"y\":20}\
            ],\
            [\
                {\"index\":1,\"gen\":0},\
                {\"x\":-5,\"y\":15}\
            ]\
        ]\
    }";

    world
        .deserialize::<_, PlayerLocationView>(&mut serde_json::de::Deserializer::from_str(
            serialized_players_locations,
        ))
        .unwrap_or_else(|_| panic!());

    world.run(|v_player: View<Player>, v_location: View<Location>| {
        let bob = EntityId::new_from_index_and_gen(1, 0);
        let alice = EntityId::new_from_index_and_gen(0, 0);

        assert_eq!(v_player[bob], player_bob);
        assert_eq!(v_location[bob], location_bob);

        assert_eq!(v_player[alice], player_alice);
        assert_eq!(v_location[alice], location_alice);
    });
}
