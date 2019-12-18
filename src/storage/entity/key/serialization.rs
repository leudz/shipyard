use super::Key;
use serde::{ser::SerializeTuple, Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        tup.serialize_element(&(self.index() as u64))?;
        tup.serialize_element(&(self.version() as u16))?;
        tup.end()
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values: (u64, u16) = Deserialize::deserialize(deserializer)?;
        Ok(Key::new_from_pair(values.0, values.1))
    }
}
