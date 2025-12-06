use crate::component::Unique;
use crate::views::UniqueOrInitViewMut;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl<'a, T: Unique + Send + Sync + Serialize> Serialize for UniqueOrInitViewMut<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.get().serialize(serializer)
    }
}

pub struct UniqueOrInitViewMutDeserializer<'tmp, 'view, T: Unique> {
    unique: &'tmp mut UniqueOrInitViewMut<'view, T>,
}

impl<'tmp, 'view, T: Unique + Send + Sync> UniqueOrInitViewMutDeserializer<'tmp, 'view, T> {
    fn new(unique: &'tmp mut UniqueOrInitViewMut<'view, T>) -> Self {
        Self { unique }
    }
}

impl<'tmp, 'view, 'de: 'view, T: Unique + Send + Sync> Deserialize<'de>
    for UniqueOrInitViewMutDeserializer<'tmp, 'view, T>
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
        let mut maybe_unique: Option<T> = None;
        Deserialize::deserialize_in_place(deserializer, &mut maybe_unique)?;

        if let (Some(unique), Some(storage)) = (maybe_unique, place.unique.get_mut()) {
            **storage = unique;
        }

        Ok(())
    }
}

impl<'view, 'de: 'view, T: Unique + Send + Sync> Deserialize<'de> for UniqueOrInitViewMut<'view, T>
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
        let mut unique_view_mut_deserializer = UniqueOrInitViewMutDeserializer::new(place);
        Deserialize::deserialize_in_place(deserializer, &mut unique_view_mut_deserializer)
    }
}
