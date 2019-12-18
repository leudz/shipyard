use super::Key;
use serde::{ser::SerializeTupleStruct, Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let mut tup = serializer.serialize_tuple_struct("Key", 2)?;
            tup.serialize_field(&(self.index() as u64))?;
            tup.serialize_field(&(self.version() as u16))?;
            tup.end()
        } else {
            serializer.serialize_u64(self.0.get())
        }
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let (index, version): (u64, u16) = Deserialize::deserialize(deserializer)?;
            Ok(Key::new_from_pair(index, version))
        } else {
            Ok(Key(Deserialize::deserialize(deserializer)?))
        }
    }
}
