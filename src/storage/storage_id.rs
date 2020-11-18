use core::cmp::Ordering;

use crate::type_id::TypeId;

/// Id of a storage, can be a `TypeId` or `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageId {
    TypeId(TypeId),
    Custom(u64),
}

impl StorageId {
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
        StorageId::Custom(core::u64::MAX)
    }
}
