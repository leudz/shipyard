use alloc::borrow::Cow;

pub(crate) static ANCHOR: () = ();

/// Defines how the `World` should be serialized.
#[derive(Clone, Copy)]
pub struct GlobalSerConfig {
    pub same_binary: bool,
    pub with_entities: bool,
    pub with_shared: WithShared,
}

impl Default for GlobalSerConfig {
    fn default() -> Self {
        GlobalSerConfig {
            same_binary: true,
            with_entities: true,
            with_shared: WithShared::PerStorage,
        }
    }
}

/// Defines how the `World` should be deserialized.
#[derive(Clone, Copy)]
pub struct GlobalDeConfig {
    pub existing_entities: ExistingEntities,
    pub with_shared: WithShared,
}

impl Default for GlobalDeConfig {
    fn default() -> Self {
        GlobalDeConfig {
            existing_entities: ExistingEntities::AsNew,
            with_shared: WithShared::PerStorage,
        }
    }
}

/// Describes what the deserialize process should do when it encounters an already existing entity.
/// - AsNew will deserialize the entity's components with a new `EntityId`
/// - Merge will only deserialize the components the entity didn't have
/// - Replace will delete all components present and add the ones from the deserializetion
/// - Skip will not deserialize this entity
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExistingEntities {
    AsNew,
    Merge,
    Replace,
    Skip,
}

/// Describes how shared components should be (de)seriliazed.
/// - All will (de)serialize shared component for all storages
/// - None will not (de)serialize shared components for any storage
/// - PerStorage will (de)serailize shared components following each storage's config
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WithShared {
    All,
    None,
    PerStorage,
}

pub struct Identifier(pub(crate) Cow<'static, str>);

impl Identifier {
    pub fn new<I: Into<Cow<'static, str>>>(identifier: I) -> Self {
        Identifier(identifier.into())
    }
}

/// Defines how a storage should be serialized.
pub struct SerConfig {
    pub identifier: Option<Identifier>,
    pub with_shared: bool,
}

impl Default for SerConfig {
    fn default() -> Self {
        SerConfig {
            identifier: None,
            with_shared: false,
        }
    }
}

pub(crate) struct SerInfos {
    pub(crate) same_binary: bool,
    pub(crate) with_entities: bool,
}

impl serde::Serialize for SerInfos {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("SerInfos", 2)?;

        state.serialize_field("same_binary", &self.same_binary)?;
        state.serialize_field("with_entities", &self.with_entities)?;

        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for SerInfos {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &'static [&'static str] = &["same_binary", "with_entities"];

        enum Field {
            SameBinary,
            WithEntities,
        }

        struct FieldVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldVisitor {
            type Value = Field;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("field identifier")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0u64 => Ok(Field::SameBinary),
                    1u64 => Ok(Field::WithEntities),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"field index 0 <= i < 2",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "same_binary" => Ok(Field::SameBinary),
                    "with_entities" => Ok(Field::WithEntities),
                    _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                }
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    b"same_binary" => Ok(Field::SameBinary),
                    b"with_entities" => Ok(Field::WithEntities),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Bytes(value),
                        &"field are `same_binary` and `with_entities`",
                    )),
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for Field {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SerInfos;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("struct SerInfos")
            }

            #[inline]
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let same_binary = seq.next_element::<bool>()?.ok_or_else(|| {
                    serde::de::Error::invalid_length(0, &"struct SerInfos with 2 elements")
                })?;
                let with_entities = seq.next_element::<bool>()?.ok_or_else(|| {
                    serde::de::Error::invalid_length(1, &"struct SerInfos with 2 elements")
                })?;

                Ok(SerInfos {
                    same_binary,
                    with_entities,
                })
            }

            #[inline]
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut same_binary: Option<bool> = None;
                let mut with_entities: Option<bool> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::SameBinary => {
                            if same_binary.is_some() {
                                return Err(serde::de::Error::duplicate_field("same_binary"));
                            }
                            same_binary = Some(map.next_value::<bool>()?);
                        }
                        Field::WithEntities => {
                            if with_entities.is_some() {
                                return Err(serde::de::Error::duplicate_field("with_entities"));
                            }
                            with_entities = Some(map.next_value::<bool>()?);
                        }
                    }
                }

                let same_binary =
                    same_binary.ok_or_else(|| serde::de::Error::missing_field("same_binary"))?;
                let with_entities = with_entities
                    .ok_or_else(|| serde::de::Error::missing_field("with_entities"))?;

                Ok(SerInfos {
                    same_binary,
                    with_entities,
                })
            }
        }

        deserializer.deserialize_struct("SerInfos", FIELDS, Visitor)
    }
}
