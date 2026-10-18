#[test]
#[rustfmt::skip]
fn recipe_ser() {
// ANCHOR: ser
use shipyard::{Component, EntitiesViewMut, ViewMut, WorldBorrow};

#[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Name(String);

#[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum FavoriteLanguage {
    Rust,
}

#[derive(WorldBorrow, serde::Serialize)]
struct LanguagesViewMut<'v> {
    #[serde(skip)]
    entities: EntitiesViewMut<'v>,
    #[serde(borrow)]
    vm_name: ViewMut<'v, Name>,
    #[serde(borrow)]
    vm_favorite_language: ViewMut<'v, FavoriteLanguage>,
}
// ANCHOR_END: ser
}

#[test]
#[rustfmt::skip]
fn recipe_de_start() {
    use serde::Deserializer;
    use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, WorldBorrow};

    #[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Name(String);

    #[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    enum FavoriteLanguage {
        Rust,
    }

    #[derive(WorldBorrow, serde::Serialize)]
    struct LanguagesViewMut<'v> {
        #[serde(skip)]
        entities: EntitiesViewMut<'v>,
        #[serde(borrow)]
        vm_name: ViewMut<'v, Name>,
        #[serde(borrow)]
        vm_favorite_language: ViewMut<'v, FavoriteLanguage>,
    }

// ANCHOR: de_start
impl<'tmp, 'view, 'de: 'view> serde::Deserialize<'de> for LanguagesViewMut<'view> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("LanguagesViewMut cannot be directly deserialized. Use deserialize_in_place instead.")
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LanguagesViewMutVisitor<'tmp, 'view> {
            place: &'tmp mut LanguagesViewMut<'view>,
        }
        impl<'tmp, 'view, 'de: 'view> serde::de::Visitor<'de> for LanguagesViewMutVisitor<'tmp, 'view> {
            type Value = ();

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a struct with vm_name and vm_favorite_language fields")
            }

            fn visit_map<A>(mut self, mut map: A) -> Result<(), A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                todo!()
            }
        }

        deserializer.deserialize_struct(
            "LanguagesViewMut",
            &["vm_name", "vm_favorite_language"],
            LanguagesViewMutVisitor { place },
        )
    }
}
// ANCHOR_END: de_start
}

#[test]
#[rustfmt::skip]
fn recipe_de() {
    use serde::Deserializer;
    use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, WorldBorrow};

    #[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Name(String);

    #[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    enum FavoriteLanguage {
        Rust,
    }

    #[derive(WorldBorrow, serde::Serialize)]
    struct LanguagesViewMut<'v> {
        #[serde(skip)]
        entities: EntitiesViewMut<'v>,
        #[serde(borrow)]
        vm_name: ViewMut<'v, Name>,
        #[serde(borrow)]
        vm_favorite_language: ViewMut<'v, FavoriteLanguage>,
    }


    struct LanguagesViewMutVisitor<'tmp, 'view> {
        place: &'tmp mut LanguagesViewMut<'view>,
    }
    impl<'tmp, 'view, 'de: 'view> serde::de::Visitor<'de> for LanguagesViewMutVisitor<'tmp, 'view> {
        type Value = ();

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a struct with vm_name and vm_favorite_language fields")
        }

        fn visit_map<A>(mut self, mut map: A) -> Result<(), A::Error>
        where
        A: serde::de::MapAccess<'de>,
        {
// ANCHOR: fields
struct NameDeserializer<'tmp, 'view> {
    place: &'tmp mut LanguagesViewMut<'view>,
}
impl<'tmp, 'view, 'de: 'view> serde::de::DeserializeSeed<'de> for NameDeserializer<'tmp, 'view> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        serde::Deserialize::deserialize_in_place(deserializer, &mut self.place.vm_name)
    }
}

struct FavoriteLanguageDeserializer<'tmp, 'view> {
    place: &'tmp mut LanguagesViewMut<'view>,
}
impl<'tmp, 'view, 'de: 'view> serde::de::DeserializeSeed<'de>
    for FavoriteLanguageDeserializer<'tmp, 'view>
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        serde::Deserialize::deserialize_in_place(
            deserializer,
            &mut self.place.vm_favorite_language,
        )
    }
}
// ANCHOR_END: fields

// ANCHOR: struct
while let Some(key) = map.next_key::<String>()? {
    match key.as_str() {
        "vm_name" => {
            map.next_value_seed(NameDeserializer { place: self.place })?;
        }
        "vm_favorite_language" => {
            map.next_value_seed(FavoriteLanguageDeserializer { place: self.place })?;
        }
        _ => {
            // Skip unknown fields
            map.next_value::<serde::de::IgnoredAny>()?;
        }
    }
}
// ANCHOR_END: struct

// ANCHOR: entities
for eid in self
    .place
    .vm_name
    .iter()
    .ids()
    .chain(self.place.vm_favorite_language.iter().ids())
{
    self.place.entities.spawn(eid);
}

Ok(())
// ANCHOR_END: entities
        }
    }
}

// ANCHOR: test
#[test]
fn recipe_test() {
    use serde::Deserializer;
    use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, World, WorldBorrow};

    let mut world = World::new();

    let eid1 = world.add_entity(Name("Alice".to_string()));
    let eid2 = world.add_entity((Name("Bob".to_string()), FavoriteLanguage::Rust));

    let serialized = world.run(|vm_favorite_language: LanguagesViewMut| {
        serde_json::to_string(&vm_favorite_language).unwrap()
    });

    drop(world);

    let mut new_world = World::new();

    let mut deserializer = serde_json::Deserializer::from_str(&serialized);
    new_world.deserialize::<_, LanguagesViewMut>(&mut deserializer);

    new_world.run(|mut vm_favorite_language: LanguagesViewMut| {
        assert!(vm_favorite_language.entities.is_alive(eid1));
        assert!(vm_favorite_language.entities.is_alive(eid2));

        assert_eq!(vm_favorite_language.vm_name[eid1].0, "Alice");
        assert_eq!(vm_favorite_language.vm_name[eid2].0, "Bob");
        assert_eq!(
            vm_favorite_language.vm_favorite_language[eid2],
            FavoriteLanguage::Rust
        );
    });

    // -----
    // -----
    // -----

    #[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Name(String);

    #[derive(Component, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    enum FavoriteLanguage {
        Rust,
    }

    #[derive(WorldBorrow, serde::Serialize)]
    struct LanguagesViewMut<'v> {
        #[serde(skip)]
        entities: EntitiesViewMut<'v>,
        #[serde(borrow)]
        vm_name: ViewMut<'v, Name>,
        #[serde(borrow)]
        vm_favorite_language: ViewMut<'v, FavoriteLanguage>,
    }

    impl<'tmp, 'view, 'de: 'view> serde::Deserialize<'de> for LanguagesViewMut<'view> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            panic!(
                "LanguagesViewMut cannot be directly deserialized. Use deserialize_in_place instead."
            )
        }

        fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
        where
            D: Deserializer<'de>,
        {
            struct LanguagesViewMutVisitor<'tmp, 'view> {
                place: &'tmp mut LanguagesViewMut<'view>,
            }
            impl<'tmp, 'view, 'de: 'view> serde::de::Visitor<'de> for LanguagesViewMutVisitor<'tmp, 'view> {
                type Value = ();

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a struct with vm_name and vm_favorite_language fields")
                }

                fn visit_map<A>(mut self, mut map: A) -> Result<(), A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    struct NameDeserializer<'tmp, 'view> {
                        place: &'tmp mut LanguagesViewMut<'view>,
                    }
                    impl<'tmp, 'view, 'de: 'view> serde::de::DeserializeSeed<'de> for NameDeserializer<'tmp, 'view> {
                        type Value = ();

                        fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
                        where
                            D: Deserializer<'de>,
                        {
                            serde::Deserialize::deserialize_in_place(
                                deserializer,
                                &mut self.place.vm_name,
                            )
                        }
                    }

                    struct FavoriteLanguageDeserializer<'tmp, 'view> {
                        place: &'tmp mut LanguagesViewMut<'view>,
                    }
                    impl<'tmp, 'view, 'de: 'view> serde::de::DeserializeSeed<'de>
                        for FavoriteLanguageDeserializer<'tmp, 'view>
                    {
                        type Value = ();

                        fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
                        where
                            D: Deserializer<'de>,
                        {
                            serde::Deserialize::deserialize_in_place(
                                deserializer,
                                &mut self.place.vm_favorite_language,
                            )
                        }
                    }

                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "vm_name" => {
                                map.next_value_seed(NameDeserializer { place: self.place })?;
                            }
                            "vm_favorite_language" => {
                                map.next_value_seed(FavoriteLanguageDeserializer {
                                    place: self.place,
                                })?;
                            }
                            _ => {
                                // Skip unknown fields
                                map.next_value::<serde::de::IgnoredAny>()?;
                            }
                        }
                    }

                    for eid in self
                        .place
                        .vm_name
                        .iter()
                        .ids()
                        .chain(self.place.vm_favorite_language.iter().ids())
                    {
                        self.place.entities.spawn(eid);
                    }

                    Ok(())
                }
            }

            deserializer.deserialize_struct(
                "LanguagesViewMut",
                &["vm_name", "vm_favorite_language"],
                LanguagesViewMutVisitor { place },
            )
        }
    }
}
// ANCHOR_END: test
