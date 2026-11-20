use bincode::Options as _;
use serde::de::DeserializeSeed as _;
use shipyard::views::serde::de::view_mut::ViewMutDeserializer;
use shipyard::views::serde::ser::view::ViewSerializer;
use shipyard::{Component, EntityId, View, ViewMut, World};

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

/// Asserts that JSON and bincode both round-trip a type-named view through its seed.
#[test]
fn type_named_roundtrip() {
    let mut source = World::new();
    let player = Player {
        name: "Alice".into(),
        score: 100,
    };
    let entity = source.add_entity(player.clone());
    let source_view = source.borrow::<View<Player>>().unwrap();
    let mut serializer = ViewSerializer::new(&source_view);
    serializer.type_names(true);
    let json = serde_json::to_string(&serializer).unwrap();
    let bytes = bincode::DefaultOptions::new()
        .serialize(&serializer)
        .unwrap();

    for binary in [false, true] {
        let mut destination = World::new();
        assert!(destination.spawn(entity));
        let mut view = destination.borrow::<ViewMut<Player>>().unwrap();
        let seed = ViewMutDeserializer::new(&mut view).type_names(true);
        if binary {
            bincode::DefaultOptions::new()
                .deserialize_seed(seed, &bytes)
                .unwrap();
        } else {
            seed.deserialize(&mut serde_json::Deserializer::from_str(&json))
                .unwrap();
        }

        assert_eq!(view.len(), 1);
        assert_eq!(view[entity], player);
    }
}

/// Asserts that positional input reports a missing name or data field and rejects binary EOF.
#[test]
fn type_named_truncated() {
    let bytes = bincode::DefaultOptions::new().serialize(&"Player").unwrap();
    let world = World::new();
    let mut view = world.borrow::<ViewMut<Player>>().unwrap();

    for (length, json) in ["[]", r#"["Player"]"#].into_iter().enumerate() {
        let error = ViewMutDeserializer::new(&mut view)
            .type_names(true)
            .deserialize(&mut serde_json::Deserializer::from_str(json))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("invalid length {length}"))
        );
    }

    let seed = ViewMutDeserializer::new(&mut view).type_names(true);
    let error = bincode::DefaultOptions::new()
        .deserialize_seed(seed, &bytes)
        .unwrap_err();
    assert!(matches!(
        *error,
        bincode::ErrorKind::Io(ref error) if error.kind() == std::io::ErrorKind::UnexpectedEof
    ));
    assert!(view.is_empty());
}

/// Asserts that a type-named map must supply the data field.
#[test]
fn type_named_missing_data() {
    let world = World::new();
    let mut view = world.borrow::<ViewMut<Player>>().unwrap();
    let json = r#"{"type_name":"Player"}"#;
    let error = ViewMutDeserializer::new(&mut view)
        .type_names(true)
        .deserialize(&mut serde_json::Deserializer::from_str(json))
        .unwrap_err();

    assert!(error.to_string().contains("missing field `data`"));
}

/// Asserts that explicit empty data is accepted and that map field order and extra fields are free.
#[test]
fn type_named_empty_data() {
    let world = World::new();
    let mut view = world.borrow::<ViewMut<Player>>().unwrap();

    for json in [
        r#"{"data":[],"extra":{"ignored":true},"type_name":"Other"}"#,
        r#"["Other",[]]"#,
    ] {
        ViewMutDeserializer::new(&mut view)
            .type_names(true)
            .deserialize(&mut serde_json::Deserializer::from_str(json))
            .unwrap();
        assert!(view.is_empty());
    }
}

/// Asserts that both representations keep or replace an existing component according to the flag.
#[test]
fn type_named_override() {
    for json in [
        r#"{"type_name":"Player","data":[[{"index":0,"gen":0},{"name":"Alice","score":100}]]}"#,
        r#"["Player",[[{"index":0,"gen":0},{"name":"Alice","score":100}]]]"#,
    ] {
        let mut world = World::new();
        let entity = world.add_entity(Player {
            name: "Alice".into(),
            score: 50,
        });
        let mut view = world.borrow::<ViewMut<Player>>().unwrap();

        for (override_component, expected) in [(false, 50), (true, 100)] {
            ViewMutDeserializer::new(&mut view)
                .type_names(true)
                .override_component(override_component)
                .deserialize(&mut serde_json::Deserializer::from_str(json))
                .unwrap();
            assert_eq!(view[entity].score, expected);
        }
    }
}
