mod hasher;

pub(crate) use hasher::TypeIdHasher;

use core::hash::{Hash, Hasher};

/// Custom `TypeId` to be able to deserialize it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]

pub struct TypeId(pub(crate) u128);

impl TypeId {
    pub(crate) fn of<T: ?Sized + 'static>() -> Self {
        core::any::TypeId::of::<T>().into()
    }
    #[cfg(test)]
    pub(crate) fn of_val<T: ?Sized + 'static>(_: &T) -> TypeId {
        core::any::TypeId::of::<T>().into()
    }
}

impl From<core::any::TypeId> for TypeId {
    fn from(type_id: core::any::TypeId) -> Self {
        match core::mem::size_of::<core::any::TypeId>() {
            8 => {
                let mut hasher = TypeIdHasher::default();

                type_id.hash(&mut hasher);

                TypeId(hasher.finish() as u128)
            }
            16 => unsafe {
                // This is technically unsound, core::any::TypeId has rust layout
                // but there is no other way to get the full value anymore

                let type_id_ptr: *const core::any::TypeId = &type_id;
                let type_id_ptr = type_id_ptr as *const TypeId;
                *type_id_ptr
            },
            _ => panic!("Compiler version not supported"),
        }
    }
}

impl From<&core::any::TypeId> for TypeId {
    fn from(type_id: &core::any::TypeId) -> Self {
        type_id.into()
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
