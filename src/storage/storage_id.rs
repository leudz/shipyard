use crate::type_id::TypeId;
use core::cmp::Ordering;

/// Id of a storage, can be a `TypeId` or `u64`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum StorageId {
    /// Identify a Rust type known at compile time
    TypeId(TypeId),
    /// Identify a type only known at runtime
    Custom(u64),
}

impl StorageId {
    /// Returns `T`'s `StorageId`.
    pub fn of<T: 'static>() -> Self {
        TypeId::of::<T>().into()
    }
}

impl From<TypeId> for StorageId {
    fn from(type_id: TypeId) -> Self {
        StorageId::TypeId(type_id)
    }
}

impl From<core::any::TypeId> for StorageId {
    fn from(type_id: core::any::TypeId) -> Self {
        StorageId::TypeId(type_id.into())
    }
}

impl From<u64> for StorageId {
    fn from(int: u64) -> Self {
        StorageId::Custom(int)
    }
}

impl PartialEq<TypeId> for StorageId {
    fn eq(&self, type_id: &TypeId) -> bool {
        if let StorageId::TypeId(self_type_id) = self {
            self_type_id == type_id
        } else {
            false
        }
    }
}

impl PartialOrd<TypeId> for StorageId {
    fn partial_cmp(&self, type_id: &TypeId) -> Option<Ordering> {
        if let StorageId::TypeId(self_type_id) = self {
            self_type_id.partial_cmp(type_id)
        } else {
            Some(Ordering::Less)
        }
    }
}

impl Default for StorageId {
    fn default() -> Self {
        StorageId::Custom(u64::MAX)
    }
}

impl core::fmt::Debug for StorageId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("StorageId");

        match self {
            StorageId::TypeId(type_id) => {
                debug_struct.field("TypeId", type_id);
            }
            StorageId::Custom(custom) => {
                debug_struct.field("Custom", custom);
            }
        }

        debug_struct.finish()
    }
}
