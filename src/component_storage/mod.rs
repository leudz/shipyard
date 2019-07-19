mod hasher;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::sparse_array::SparseArray;
use hasher::TypeIdHasher;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// Wrapper over `AtomicRefCell<Box<SparseArray<T>>>` to be able to store different `T`s
/// in a `HashMap<TypeId, ComponentStorage>`
pub struct ComponentStorage(AtomicRefCell<Box<dyn Any + Send + Sync>>);

impl ComponentStorage {
    /// Creates a new `ComponentStorage` storing elements of type T
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        ComponentStorage(AtomicRefCell::new(Box::new(SparseArray::<T>::default())))
    }
    /// Immutably borrows the container
    pub(crate) fn array<'a, T: 'static>(
        &'a self,
    ) -> Result<Ref<'a, SparseArray<T>>, error::Borrow> {
        Ok(Ref::map(self.0.try_borrow()?, |array| {
            array.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows the container
    pub(crate) fn array_mut<'a, T: 'static>(
        &'a self,
    ) -> Result<RefMut<'a, SparseArray<T>>, error::Borrow> {
        Ok(RefMut::map(self.0.try_borrow_mut()?, |array| {
            array.downcast_mut().unwrap()
        }))
    }
}

/// Contains all components present in the World
// Wrapper to hide `TypeIdHasher` and the whole `HashMap` from public interface
#[derive(Default)]
pub struct AllStorages(
    pub(crate) HashMap<TypeId, ComponentStorage, BuildHasherDefault<TypeIdHasher>>,
);

impl AllStorages {
    /// Add a storage for a new type
    /// Does nothing if a storage already exists
    pub fn add_storage<T: 'static + Send + Sync>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| ComponentStorage::new::<T>());
    }
}
