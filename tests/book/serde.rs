#[rustfmt::skip]
#[test]
fn entity_id() {
// ANCHOR: entity_id
use shipyard::{EntityId, World};

let mut world = World::new();

let eid1 = world.add_entity(());

let serialized = serde_json::to_string(&eid1).unwrap();
assert_eq!(serialized, r#"{"index":0,"gen":0}"#);

let new_eid: EntityId = serde_json::from_str(&serialized).unwrap();
assert_eq!(new_eid, eid1);
// ANCHOR_END: entity_id
}

#[rustfmt::skip]
#[test]
fn single_view() {
// ANCHOR: single_view
use shipyard::{Component, EntityId, View, ViewMut, World};

#[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Name(String);

let mut world = World::new();

let eid1 = world.add_entity(Name("Alice".to_string()));
let eid2 = world.add_entity(Name("Bob".to_string()));

// There is also a World::serialize
let serialized = world.run(|v_name: View<Name>| serde_json::to_string(&v_name).unwrap());

drop(world);

let mut world = World::new();

let mut deserializer = serde_json::de::Deserializer::from_str(&serialized);
world
    .deserialize::<_, ViewMut<Name>>(&mut deserializer)
    .unwrap();

assert_eq!(world.get::<&Name>(eid2).unwrap().0, "Bob");
assert_eq!(world.get::<&Name>(eid1).unwrap().0, "Alice");

// Note that we never added eid1 or eid2 to this second World
// they weren't added during deserialization either
// the World is currently in an unstable state

assert_eq!(world.is_entity_alive(eid1), false);

// To fix it, we can use `World::spawn` for example
// we could've also created empty entities
// or (de)serialized EntitiesViewMut

world.spawn(eid1);
world.spawn(eid2);

assert_eq!(world.is_entity_alive(eid1), true);
assert_eq!(world.is_entity_alive(eid2), true);
// ANCHOR_END: single_view
}

#[rustfmt::skip]
#[test]
fn multiple_views() {
// ANCHOR: multiple_views
use shipyard::{
    error, Component, EntitiesViewMut, EntityId, View, ViewMut, World, WorldBorrow,
};

#[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Name(String);

#[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum FavoriteLanguage {
    Rust,
}

#[derive(WorldBorrow, serde::Serialize, serde::Deserialize)]
struct LanguagesViewMut<'v> {
    #[serde(borrow)]
    entities: EntitiesViewMut<'v>,
    #[serde(borrow)]
    vm_name: ViewMut<'v, Name>,
    #[serde(borrow)]
    vm_favorite_language: ViewMut<'v, FavoriteLanguage>,
}

let mut world = World::new();

let eid1 = world.add_entity((Name("Alice".to_string()), FavoriteLanguage::Rust));
let eid2 = world.add_entity(Name("Bob".to_string()));

let serialized =
    world.run(|vm_languages: LanguagesViewMut| serde_json::to_string(&vm_languages).unwrap());

drop(world);

let mut world = World::new();

let mut deserializer = serde_json::de::Deserializer::from_str(&serialized);
world
    .deserialize::<_, LanguagesViewMut>(&mut deserializer)
    .unwrap();

assert_eq!(world.get::<&Name>(eid1).unwrap().0, "Alice");
assert_eq!(
    *world.get::<&FavoriteLanguage>(eid1).unwrap(),
    &FavoriteLanguage::Rust
);
assert_eq!(world.get::<&Name>(eid2).unwrap().0, "Bob");
assert!(matches!(
    world.get::<&FavoriteLanguage>(eid2),
    Err(error::GetComponent::MissingComponent(_))
));

// This time we serialized EntitiesViewMut
// so no unstable state

assert_eq!(world.is_entity_alive(eid1), true);
assert_eq!(world.is_entity_alive(eid2), true);
// ANCHOR_END: multiple_views
}
