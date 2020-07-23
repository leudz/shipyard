use super::EntityId;
use serde::{ser::SerializeTupleStruct, Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for EntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let mut tup = serializer.serialize_tuple_struct("EntityId", 2)?;
            tup.serialize_field(&self.index())?;
            tup.serialize_field(&(self.gen() as u16))?;
            tup.end()
        } else {
            serializer.serialize_u64(self.0.get())
        }
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let (index, gen): (u64, u16) = Deserialize::deserialize(deserializer)?;
            Ok(EntityId::new_from_pair_unchecked(index, gen))
        } else {
            Ok(EntityId(Deserialize::deserialize(deserializer)?))
        }
    }
}
