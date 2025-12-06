use crate::component::Unique;
use crate::views::UniqueViewMut;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl<'a, T: Unique + Serialize> Serialize for UniqueViewMut<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.unique.value.serialize(serializer)
    }
}

pub struct UniqueViewMutDeserializer<'tmp, 'view, T: Unique> {
    unique: &'tmp mut UniqueViewMut<'view, T>,
}

impl<'tmp, 'view, T: Unique> UniqueViewMutDeserializer<'tmp, 'view, T> {
    fn new(unique: &'tmp mut UniqueViewMut<'view, T>) -> Self {
        Self { unique }
    }
}

impl<'tmp, 'view, 'de: 'view, T: Unique> Deserialize<'de>
    for UniqueViewMutDeserializer<'tmp, 'view, T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("UniqueViewMut cannot be directly deserialized. Use deserialize_in_place instead.")
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize_in_place(deserializer, &mut place.unique.unique.value)
    }
}

impl<'view, 'de: 'view, T: Unique> Deserialize<'de> for UniqueViewMut<'view, T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("UniqueViewMut cannot be directly deserialized. Use deserialize_in_place instead.")
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut unique_view_mut_deserializer = UniqueViewMutDeserializer::new(place);
        Deserialize::deserialize_in_place(deserializer, &mut unique_view_mut_deserializer)
    }
}
