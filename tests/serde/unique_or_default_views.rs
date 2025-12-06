use shipyard::{Unique, UniqueOrDefaultView, UniqueOrDefaultViewMut, World};

#[derive(Unique, Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct GameState {
    level: u32,
    score: u32,
}

#[test]
fn unique_or_default_view_serialize() {
    let world = World::new();

    let game_state = GameState {
        level: 5,
        score: 1500,
    };

    world.add_unique(game_state.clone());

    world.run(|unique: UniqueOrDefaultView<GameState>| {
        let serialized = serde_json::to_string(&unique).unwrap();

        assert_eq!(serialized, r#"{"level":5,"score":1500}"#);

        let deserialized: GameState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, game_state);
    });
}

#[test]
fn unique_or_default_view_mut_serialize() {
    let world = World::new();

    let game_state = GameState {
        level: 5,
        score: 1500,
    };

    world.add_unique(game_state.clone());

    world.run(|unique: UniqueOrDefaultViewMut<GameState>| {
        let serialized = serde_json::to_string(&unique).unwrap();

        assert_eq!(serialized, r#"{"level":5,"score":1500}"#);

        let deserialized: GameState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, game_state);
    });
}

#[test]
fn unique_or_default_view_mut_deserialize_in_place() {
    let world = World::new();

    let game_state = GameState {
        level: 1,
        score: 100,
    };

    world.add_unique(game_state);

    let updated_state = GameState {
        level: 10,
        score: 5000,
    };

    world.run(|mut unique: UniqueOrDefaultViewMut<GameState>| {
        let serialized_updated = r#"{"level":10,"score":5000}"#;

        let mut deserializer = serde_json::Deserializer::from_str(serialized_updated);
        serde::Deserialize::deserialize_in_place(&mut deserializer, &mut unique).unwrap();
    });

    world.run(|unique: UniqueOrDefaultView<GameState>| {
        assert_eq!(*unique, updated_state);
    });
}

#[test]
#[should_panic(
    expected = "UniqueViewMut cannot be directly deserialized. Use deserialize_in_place instead."
)]
fn unique_or_default_view_mut_direct_deserialize_panic() {
    serde_json::from_str::<UniqueOrDefaultViewMut<GameState>>("").unwrap();
}
