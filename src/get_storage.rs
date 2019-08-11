use crate::atomic_refcell::Ref;
use crate::component_storage::AllStorages;
use crate::error;
use crate::sparse_array::{Read, Write};
use std::any::TypeId;
use std::convert::TryInto;

// `GetStorage` allows to retrive component storages
// A storage can be found based on the `TypeId` of the input
// Mutability is determined by `&` for read access and `&mut` for write access
pub trait GetStorage<'a> {
    type Storage;
    fn get_in(storages: Ref<'a, AllStorages>) -> Result<Self::Storage, error::GetStorage>;
}

pub trait AbstractMut<'a> {
    type Out;
    fn borrow(storages: Ref<'a, AllStorages>) -> Result<Self::Out, error::GetStorage>;
}

impl<'a, T: 'static> AbstractMut<'a> for &T {
    type Out = Read<'a, T>;
    fn borrow(storages: Ref<'a, AllStorages>) -> Result<Self::Out, error::GetStorage> {
        Ref::try_map(storages, |storages| {
            match storages.0.get(&TypeId::of::<T>()) {
                Some(storage) => Ok(storage),
                None => Err(error::GetStorage::MissingComponent),
            }
        })?
        .try_into()
        .map_err(error::GetStorage::StorageBorrow)
    }
}

impl<'a, T: 'static> AbstractMut<'a> for &mut T {
    type Out = Write<'a, T>;
    fn borrow(storages: Ref<'a, AllStorages>) -> Result<Self::Out, error::GetStorage> {
        Ref::try_map(storages, |storages| {
            match storages.0.get(&TypeId::of::<T>()) {
                Some(storage) => Ok(storage),
                None => Err(error::GetStorage::MissingComponent),
            }
        })?
        .try_into()
        .map_err(error::GetStorage::StorageBorrow)
    }
}

impl<'a, T: AbstractMut<'a>> GetStorage<'a> for T {
    type Storage = T::Out;
    fn get_in(storages: Ref<'a, AllStorages>) -> Result<Self::Storage, error::GetStorage> {
        Ok(T::borrow(storages)?)
    }
}

macro_rules! impl_get_storage {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AbstractMut<'a>),+> GetStorage<'a> for ($($type,)+) {
            type Storage = ($($type::Out,)+);
            fn get_in(storages: Ref<'a, AllStorages>) -> Result<Self::Storage, error::GetStorage> {
                Ok(($(
                    $type::borrow(Ref::clone(&storages))?,
                )+))
            }
        }
    }
}

macro_rules! get_storage {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_get_storage![$(($left_type, $left_index))*];
        get_storage![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_get_storage![$(($type, $index))*];
    }
}

get_storage![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
