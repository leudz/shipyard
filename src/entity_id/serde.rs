use super::EntityId;
use core::fmt;
use core::num::NonZeroU64;
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};

const FIELDS: &[&str] = &["index", "gen"];

impl Serialize for EntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let mut ser_struct = serializer.serialize_struct("EntityId", 2)?;
            ser_struct.serialize_field(FIELDS[0], &(self.index()))?;
            ser_struct.serialize_field(FIELDS[1], &(self.gen()))?;
            ser_struct.end()
        } else {
            let clone = self.clone();
            ((clone.0).get() - 1).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Index,
            Generation,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("`index` or `gen`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "index" => Ok(Field::Index),
                            "gen" => Ok(Field::Generation),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct EntityIdVisitor;

        impl<'de> Visitor<'de> for EntityIdVisitor {
            type Value = EntityId;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("struct EntityId")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<EntityId, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let index = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;

                let generation = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;

                Ok(EntityId::new_from_parts(index, generation))
            }

            fn visit_map<V>(self, mut map: V) -> Result<EntityId, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut index = None;
                let mut generation = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Index => {
                            if index.is_some() {
                                return Err(de::Error::duplicate_field("index"));
                            }
                            index = Some(map.next_value()?);
                        }
                        Field::Generation => {
                            if generation.is_some() {
                                return Err(de::Error::duplicate_field("gen"));
                            }
                            generation = Some(map.next_value()?);
                        }
                    }
                }

                let index = index.ok_or_else(|| de::Error::missing_field("index"))?;
                let generation = generation.ok_or_else(|| de::Error::missing_field("gen"))?;

                Ok(EntityId::new_from_parts(index, generation))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_struct("EntityId", FIELDS, EntityIdVisitor)
        } else {
            let non_zero: u64 = Deserialize::deserialize(deserializer)?;

            Ok(EntityId(unsafe { NonZeroU64::new_unchecked(non_zero + 1) }))
        }
    }
}

#[test]
fn serde_json() {
    let string = serde_json::to_string(&EntityId::new_from_index_and_gen(10, 2)).unwrap();
    assert_eq!(r#"{"index":10,"gen":2}"#, string);

    let entity = serde_json::de::from_str::<EntityId>(&string).unwrap();
    assert_eq!(entity, EntityId::new_from_index_and_gen(10, 2));
}

#[test]
fn bincode() {
    let bytes = bincode::serialize(&EntityId::new_from_parts(10, 2)).unwrap();
    assert_eq!(&[10, 0, 0, 0, 0, 0, 2, 0][..], &bytes);

    let entity = bincode::deserialize::<EntityId>(&bytes).unwrap();
    assert_eq!(entity, EntityId::new_from_parts(10, 2));
}
