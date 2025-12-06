use crate::add_component::AddComponent;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::tracking::Tracking;
use crate::views::ViewMut;
use core::fmt;
use serde::de::{DeserializeOwned, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub struct ViewMutDeserializer<'tmp, 'view, T: Component, Track> {
    view: &'tmp mut ViewMut<'view, T, Track>,
    override_component: bool,
}

impl<'tmp, 'view, T: Component, Track> ViewMutDeserializer<'tmp, 'view, T, Track> {
    pub fn new(
        view: &'tmp mut ViewMut<'view, T, Track>,
    ) -> ViewMutDeserializer<'tmp, 'view, T, Track> {
        ViewMutDeserializer {
            view,
            override_component: true,
        }
    }
}

impl<'a, T: Component + Serialize, Track: Tracking> Serialize for ViewMut<'a, T, Track> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_view().serialize(serializer)
    }
}

impl<'tmp, 'view, 'de: 'view, T: Component, Track: Tracking> Deserialize<'de>
    for ViewMutDeserializer<'tmp, 'view, T, Track>
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
        struct SeqVisitor<'tmp2, 'tmp, 'view, T: Component, Track: Tracking> {
            place: &'tmp2 mut ViewMutDeserializer<'tmp, 'view, T, Track>,
        }

        impl<'tmp2, 'tmp, 'view, 'de: 'view, T: Component, Track: Tracking> Visitor<'de>
            for SeqVisitor<'tmp2, 'tmp, 'view, T, Track>
        where
            T: for<'d> Deserialize<'d>,
        {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence of entity_id-component pairs")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                if self.place.override_component {
                    while let Some((eid, component)) = seq.next_element::<(EntityId, T)>()? {
                        self.place.view.add_component_unchecked(eid, component);
                    }
                } else {
                    while let Some((eid, component)) = seq.next_element::<(EntityId, T)>()? {
                        if self.place.view.contains(eid) {
                            continue;
                        }

                        self.place.view.add_component_unchecked(eid, component);
                    }
                }

                Ok(())
            }
        }

        deserializer.deserialize_seq(SeqVisitor { place })
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
        let mut view_mut_deserializer = ViewMutDeserializer::new(place);
        Deserialize::deserialize_in_place(deserializer, &mut view_mut_deserializer)
    }
}
