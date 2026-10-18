use crate::entity_id::EntityId;
use crate::views::EntitiesViewMut;
use core::fmt;
use serde::de::{DeserializeSeed, Visitor};
use serde::{Deserialize, Deserializer};

/// Builder to customize [`EntitiesViewMut`]'s deserialization format.
///
/// Make sure to match the configuration used when serializing.
pub struct EntitiesViewMutDeserializer<'tmp, 'view> {
    entities: &'tmp mut EntitiesViewMut<'view>,
}

impl<'tmp, 'view> EntitiesViewMutDeserializer<'tmp, 'view> {
    #[allow(missing_docs)]
    pub fn new(
        entities: &'tmp mut EntitiesViewMut<'view>,
    ) -> EntitiesViewMutDeserializer<'tmp, 'view> {
        EntitiesViewMutDeserializer { entities }
    }
}

impl<'tmp, 'view, 'de: 'view> DeserializeSeed<'de> for EntitiesViewMutDeserializer<'tmp, 'view> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SeqVisitor<'tmp, 'view> {
            place: EntitiesViewMutDeserializer<'tmp, 'view>,
        }

        impl<'tmp, 'view, 'de: 'view> Visitor<'de> for SeqVisitor<'tmp, 'view> {
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

        deserializer.deserialize_seq(SeqVisitor { place: self })
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
        let entities_view_mut_deserializer = EntitiesViewMutDeserializer::new(place);
        DeserializeSeed::deserialize(entities_view_mut_deserializer, deserializer)
    }
}
