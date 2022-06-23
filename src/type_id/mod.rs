mod hasher;

pub(crate) use hasher::TypeIdHasher;

use core::hash::{Hash, Hasher};

/// Custom `TypeId` to be able to deserialize it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]

pub struct TypeId(pub(crate) u64);

impl TypeId {
    pub(crate) fn of<T: ?Sized + 'static>() -> Self {
        core::any::TypeId::of::<T>().into()
    }
}

impl From<core::any::TypeId> for TypeId {
    fn from(type_id: core::any::TypeId) -> Self {
        let mut hasher = TypeIdHasher::default();

        type_id.hash(&mut hasher);

        TypeId(hasher.finish())
    }
}

impl From<&core::any::TypeId> for TypeId {
    fn from(type_id: &core::any::TypeId) -> Self {
        let mut hasher = TypeIdHasher::default();

        type_id.hash(&mut hasher);

        TypeId(hasher.finish())
    }
}

impl PartialEq<core::any::TypeId> for TypeId {
    fn eq(&self, other: &core::any::TypeId) -> bool {
        let type_id: TypeId = other.into();

        *self == type_id
    }
}

impl PartialEq<TypeId> for core::any::TypeId {
    fn eq(&self, other: &TypeId) -> bool {
        *other == *self
    }
}
