use crate::add_component::AddComponent;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::tracking::Tracking;
use crate::views::ViewMut;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::de::{DeserializeOwned, DeserializeSeed, Error, IgnoredAny, Visitor};
use serde::{Deserialize, Deserializer};

/// Builder to customize [`ViewMut`]'s deserialization format.
///
/// Make sure to match the configuration used when serializing.
pub struct ViewMutDeserializer<'tmp, 'view, T: Component, Track> {
    #[allow(missing_docs)]
    pub view: &'tmp mut ViewMut<'view, T, Track>,
    /// Whether to replace existing components. Defaults to `true`.
    pub override_component: bool,
    /// Expects the component's type name in addition to the ids and components.
    pub type_names: bool,
}

impl<'tmp, 'view, T: Component, Track> ViewMutDeserializer<'tmp, 'view, T, Track> {
    #[allow(missing_docs)]
    pub fn new(
        view: &'tmp mut ViewMut<'view, T, Track>,
    ) -> ViewMutDeserializer<'tmp, 'view, T, Track> {
        ViewMutDeserializer {
            view,
            override_component: true,
            type_names: false,
        }
    }

    /// Controls whether deserialized components replace existing components.
    pub fn override_component(mut self, override_component: bool) -> Self {
        self.override_component = override_component;

        self
    }

    /// Expects the component's type name in addition to the ids and components.
    pub fn type_names(mut self, type_names: bool) -> Self {
        self.type_names = type_names;

        self
    }
}

impl<T: Component, Track: Tracking> ViewMutDeserializer<'_, '_, T, Track> {
    fn insert_components(&mut self, data: Vec<(EntityId, T)>) {
        for (entity, component) in data {
            if !self.override_component && self.view.contains(entity) {
                continue;
            }

            self.view.add_component_unchecked(entity, component);
        }
    }
}

impl<'tmp, 'view, 'de: 'view, T: Component, Track: Tracking> DeserializeSeed<'de>
    for ViewMutDeserializer<'tmp, 'view, T, Track>
where
    T: DeserializeOwned,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.type_names {
            struct StructVisitor<'tmp, 'view, T: Component, Track: Tracking> {
                place: ViewMutDeserializer<'tmp, 'view, T, Track>,
            }

            impl<'tmp, 'view, 'de: 'view, T: Component, Track: Tracking> Visitor<'de>
                for StructVisitor<'tmp, 'view, T, Track>
            where
                T: DeserializeOwned,
            {
                type Value = ();

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a struct with type_name and data fields")
                }

                fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::SeqAccess<'de>,
                {
                    if seq.next_element::<String>()?.is_none() {
                        return Err(A::Error::invalid_length(0, &self));
                    }
                    let Some(data) = seq.next_element::<Vec<(EntityId, T)>>()? else {
                        return Err(A::Error::invalid_length(1, &self));
                    };

                    self.place.insert_components(data);
                    Ok(())
                }

                fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    let mut data = None;

                    // Owned keys also support escaped field names.
                    while let Some(key) = map.next_key::<String>()? {
                        if key == "data" {
                            data = Some(map.next_value()?);
                        } else {
                            // The type name is informational. Skip it and any unknown fields.
                            map.next_value::<IgnoredAny>()?;
                        }
                    }

                    let Some(data) = data else {
                        return Err(A::Error::missing_field("data"));
                    };

                    self.place.insert_components(data);
                    Ok(())
                }
            }

            deserializer.deserialize_struct(
                "View",
                &["type_name", "data"],
                StructVisitor { place: self },
            )
        } else {
            struct SeqVisitor<'tmp, 'view, T: Component, Track: Tracking> {
                place: ViewMutDeserializer<'tmp, 'view, T, Track>,
            }

            impl<'tmp, 'view, 'de: 'view, T: Component, Track: Tracking> Visitor<'de>
                for SeqVisitor<'tmp, 'view, T, Track>
            where
                T: DeserializeOwned,
            {
                type Value = ();

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a sequence of entity_id-component pairs")
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::SeqAccess<'de>,
                {
                    let override_component = self.place.override_component;

                    while let Some((eid, component)) = seq.next_element::<(EntityId, T)>()? {
                        if !override_component && self.place.view.contains(eid) {
                            continue;
                        }

                        self.place.view.add_component_unchecked(eid, component);
                    }

                    Ok(())
                }
            }

            deserializer.deserialize_seq(SeqVisitor { place: self })
        }
    }
}

impl<'view, 'de: 'view, T: Component, Track: Tracking> Deserialize<'de> for ViewMut<'view, T, Track>
where
    T: DeserializeOwned,
{
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
        let view_mut_deserializer = ViewMutDeserializer::new(place);
        DeserializeSeed::deserialize(view_mut_deserializer, deserializer)
    }
}
