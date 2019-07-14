use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::sparse_array::SparseArray;
use std::any::Any;

/// Wrapper over `AtomicRefCell<Box<SparseArray<T>>>` to be able to store different `T`s
/// in a `HashMap<TypeId, ComponentStorage>`
pub(crate) struct ComponentStorage(AtomicRefCell<Box<dyn Any + Send + Sync>>);

impl ComponentStorage {
    /// Creates a new `ComponentStorage` storing elements of type T
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        ComponentStorage(AtomicRefCell::new(Box::new(SparseArray::<T>::default())))
    }
    /// Immutably borrows the container
    pub(crate) fn get<'a, T: 'static>(&'a self) -> Result<Ref<'a, SparseArray<T>>, error::Borrow> {
        Ok(Ref::map(self.0.try_borrow()?, |array| {
            array.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows the container
    pub(crate) fn get_mut<'a, T: 'static>(
        &'a self,
    ) -> Result<RefMut<'a, SparseArray<T>>, error::Borrow> {
        Ok(RefMut::map(self.0.try_borrow_mut()?, |array| {
            array.downcast_mut().unwrap()
        }))
    }
}
