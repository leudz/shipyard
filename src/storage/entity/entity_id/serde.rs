use std::num::NonZeroU64;

use super::EntityId;
use serde::{ser::SerializeTupleStruct, Deserialize, Deserializer, Serialize, Serializer};

impl EntityId {
    /// Modify the generation of the `EntityId`.
    #[cfg_attr(docsrs, doc(cfg(feature = "serde1")))]
    pub fn set_gen(&mut self, gen: u32) {
        assert!(gen as u64 <= Self::max_gen());
        // SAFE never zero
        self.0 = unsafe {
            NonZeroU64::new_unchecked(
                (self.0.get() & !Self::GEN_MASK) | (gen as u64) << Self::INDEX_LEN,
            )
        };
    }

    /// Make an `EntityId` from an index and generation.
    #[cfg_attr(docsrs, doc(cfg(feature = "serde1")))]
    pub fn from_index_and_gen(index: u64, gen: u32) -> EntityId {
        assert!(index < Self::INDEX_MASK);
        assert!(gen as u64 <= Self::max_gen());

        EntityId(unsafe {
            NonZeroU64::new_unchecked((index + 1) | (gen as u64) << Self::INDEX_LEN)
        })
    }
}

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
            self.0.serialize(serializer)
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
            Ok(EntityId::new_from_parts(index, gen, 0))
        } else {
            Ok(EntityId(Deserialize::deserialize(deserializer)?))
        }
    }
}
