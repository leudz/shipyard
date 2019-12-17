use serde::{Serialize, Deserialize, Serializer, Deserializer, ser::{SerializeTuple}};
use super::Key;

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        tup.serialize_element(&self.version())?;
        tup.serialize_element(&self.index())?;
        tup.end()
    }
}

impl<'de> Deserialize<'de> for Key
where {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values: (u64, u64) = Deserialize::deserialize(deserializer)?;
        Ok(Key::new_from_pair(values.0,values.1))
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.version(), self.index())
    }
}