mod hasher;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::get_storage::GetStorage;
use crate::sparse_array::SparseArray;
use hasher::TypeIdHasher;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// Wrapper over `AtomicRefCell<Box<SparseArray<T>>>` to be able to store different `T`s
/// in a `HashMap<TypeId, ComponentStorage>`.
pub(crate) struct ComponentStorage(AtomicRefCell<Box<dyn Any + Send + Sync>>);

impl ComponentStorage {
    /// Creates a new `ComponentStorage` storing elements of type T.
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        ComponentStorage(AtomicRefCell::new(Box::new(SparseArray::<T>::default())))
    }
    /// Immutably borrows the container.
    pub(crate) fn array<'a, T: 'static>(
        &'a self,
    ) -> Result<Ref<'a, SparseArray<T>>, error::Borrow> {
        Ok(Ref::map(self.0.try_borrow()?, |array| {
            array.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows the container.
    pub(crate) fn array_mut<'a, T: 'static>(
        &'a self,
    ) -> Result<RefMut<'a, SparseArray<T>>, error::Borrow> {
        Ok(RefMut::map(self.0.try_borrow_mut()?, |array| {
            array.downcast_mut().unwrap()
        }))
    }
}

/// Contains all components present in the World.
// Wrapper to hide `TypeIdHasher` and the whole `HashMap` from public interface
#[derive(Default)]
pub struct AllStorages(
    pub(crate) HashMap<TypeId, ComponentStorage, BuildHasherDefault<TypeIdHasher>>,
);

impl AllStorages {
    /// Register a new component type and create a storage for it.
    /// Does nothing if a storage already exists.
    pub(crate) fn register<T: 'static + Send + Sync>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| ComponentStorage::new::<T>());
    }
}

impl<'a> Ref<'a, AllStorages> {
    /// Same as `try_get_storage` but will `unwrap` if an error occurs.
    pub fn get_storage<T: GetStorage<'a>>(&self) -> T::Storage {
        self.try_get_storage::<T>().unwrap()
    }
    /// Retrives storages based on type `T`.
    /// `&T` returns a read access to the storage.
    /// `&mut T` returns a write access to the storage.
    /// To retrive multiple storages at once, use a tuple.
    pub fn try_get_storage<T: GetStorage<'a>>(&self) -> Result<T::Storage, error::GetStorage> {
        T::get_in(Ref::clone(self))
    }
}

impl<'a> RefMut<'a, AllStorages> {
    /// Same as `try_get_storage` but will `unwrap` if an error occurs.
    pub fn get_storage<T: GetStorage<'a>>(self) -> T::Storage {
        self.try_get_storage::<T>().unwrap()
    }
    /// Retrives storages based on type `T`.
    /// `&T` returns a read access to the storage.
    /// `&mut T` returns a write access to the storage.
    /// To retrive multiple storages at once, use a tuple.
    pub fn try_get_storage<T: GetStorage<'a>>(self) -> Result<T::Storage, error::GetStorage> {
        RefMut::downgrade(self).try_get_storage::<T>()
    }
}
