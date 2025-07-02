mod hasher;

pub(crate) use hasher::TypeIdHasher;

use core::{
    hash::{Hash, Hasher},
    mem::{align_of, size_of},
};

/// Custom `TypeId` to be able to deserialize it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeId(pub(crate) u128);

impl TypeId {
    /// Returns the `TypeId` of the type this generic function has been instantiated with.
    pub fn of<T: ?Sized + 'static>() -> Self {
        core::any::TypeId::of::<T>().into()
    }
    #[cfg(test)]
    pub(crate) fn of_val<T: ?Sized + 'static>(_: &T) -> TypeId {
        core::any::TypeId::of::<T>().into()
    }
}

impl From<core::any::TypeId> for TypeId {
    fn from(type_id: core::any::TypeId) -> Self {
        match size_of::<core::any::TypeId>() {
            8 => {
                let mut hasher = TypeIdHasher::default();

                type_id.hash(&mut hasher);

                TypeId(hasher.finish() as u128)
            }
            16 => unsafe {
                // This is technically unsound, core::any::TypeId has rust layout
                // but there is no other way to get the full value anymore

                match align_of::<core::any::TypeId>() {
                    8 => {
                        let type_id_ptr: *const core::any::TypeId = &type_id;
                        let type_id_ptr = type_id_ptr as *const (u64, u64);
                        let type_id = *type_id_ptr;
                        let type_id = ((type_id.0 as u128) << 64) | type_id.1 as u128;
                        TypeId(type_id)
                    }
                    16 => {
                        let type_id_ptr: *const core::any::TypeId = &type_id;
                        let type_id_ptr = type_id_ptr as *const TypeId;
                        *type_id_ptr
                    }
                    _ => panic!("Compiler version not supported, please report it to https://github.com/leudz/shipyard/issues or Zulip"),
                }
            },
            _ => panic!("Compiler version not supported, please report it to https://github.com/leudz/shipyard/issues or Zulip"),
        }
    }
}

impl From<&core::any::TypeId> for TypeId {
    fn from(type_id: &core::any::TypeId) -> Self {
        (*type_id).into()
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
