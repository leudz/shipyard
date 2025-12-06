use crate::entity_id::EntityId;
use crate::views::EntitiesViewMut;
use core::fmt;
use serde::de::Visitor;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub struct EntitiesViewMutDeserializer<'tmp, 'view> {
    entities: &'tmp mut EntitiesViewMut<'view>,
}

impl<'tmp, 'view> EntitiesViewMutDeserializer<'tmp, 'view> {
    pub fn new(
        entities: &'tmp mut EntitiesViewMut<'view>,
    ) -> EntitiesViewMutDeserializer<'tmp, 'view> {
        EntitiesViewMutDeserializer { entities }
    }
}

impl<'a> Serialize for EntitiesViewMut<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;

        self.iter()
            .try_for_each(|eid| seq.serialize_element(&eid))?;

        seq.end()
    }
}

impl<'tmp, 'view, 'de: 'view> Deserialize<'de> for EntitiesViewMutDeserializer<'tmp, 'view> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("ViewMut cannot be directly deserialized. Use deserialize_in_place instead.")
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SeqVisitor<'tmp2, 'tmp, 'view> {
            place: &'tmp2 mut EntitiesViewMutDeserializer<'tmp, 'view>,
        }

        impl<'tmp2, 'tmp, 'view, 'de: 'view> Visitor<'de> for SeqVisitor<'tmp2, 'tmp, 'view> {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence of entity_id-component pairs")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                while let Some(eid) = seq.next_element::<EntityId>()? {
                    self.place.entities.spawn(eid);
                }

                Ok(())
            }
        }

        deserializer.deserialize_seq(SeqVisitor { place })
    }
}

impl<'view, 'de: 'view> Deserialize<'de> for EntitiesViewMut<'view> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("EntitiesViewMut cannot be directly deserialized. Use deserialize_in_place instead.")
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut entities_view_mut_deserializer = EntitiesViewMutDeserializer::new(place);
        Deserialize::deserialize_in_place(deserializer, &mut entities_view_mut_deserializer)
    }
}
