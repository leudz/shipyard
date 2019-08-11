mod delete;
mod hasher;
mod view;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::get_storage::GetStorage;
use crate::sparse_array::SparseArray;
use delete::Delete;
use hasher::TypeIdHasher;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
pub(crate) use view::AllStoragesViewMut;

/// Abstract away `T` from `SparseArray<T>` to be able to store
/// different types in a `HashMap<TypeId, ComponentStorage>`.
/// `delete` is the address of the vtable part of the `SparseArray`'s `Delete` implementation.
pub(crate) struct ComponentStorage {
    data: AtomicRefCell<Box<dyn Any + Send + Sync>>,
    delete: usize,
}

impl ComponentStorage {
    /// Creates a new `ComponentStorage` storing elements of type T.
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        let array = SparseArray::<T>::default();
        // store the vtable of this trait object
        // for a full explanation see Delete documentation
        let delete: [usize; 2] = unsafe {
            *(&(&array as &dyn Delete as *const _) as *const *const _ as *const [usize; 2])
        };
        ComponentStorage {
            data: AtomicRefCell::new(Box::new(array)),
            delete: delete[1],
        }
    }
    /// Immutably borrows the container.
    pub(crate) fn array<'a, T: 'static>(
        &'a self,
    ) -> Result<Ref<'a, SparseArray<T>>, error::Borrow> {
        Ok(Ref::map(self.data.try_borrow()?, |array| {
            array.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows the container.
    pub(crate) fn array_mut<'a, T: 'static>(
        &'a self,
    ) -> Result<RefMut<'a, SparseArray<T>>, error::Borrow> {
        Ok(RefMut::map(self.data.try_borrow_mut()?, |array| {
            array.downcast_mut().unwrap()
        }))
    }
    /// Mutably borrows the container and delete `index`.
    pub(crate) fn delete(&mut self, index: usize) -> Result<(), error::Borrow> {
        // reconstruct a `dyn Delete` from two pointers
        // for a full explanation see Delete documentation
        let array: RefMut<Box<dyn Any + Send + Sync>> = self.data.try_borrow_mut()?;
        let array: usize = &**array as *const dyn Any as *const () as usize;
        let delete: &mut dyn Delete =
            unsafe { &mut **(&[array, self.delete] as *const _ as *const *mut dyn Delete) };
        delete.delete(index);
        Ok(())
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
            .or_insert_with(ComponentStorage::new::<T>);
    }
    pub(crate) fn view_mut(&mut self) -> AllStoragesViewMut {
        AllStoragesViewMut(&mut self.0)
    }
}

impl<'a> Ref<'a, AllStorages> {
    /// Retrives storages based on type `T`.
    ///
    /// `&T` returns a read access to the storage.
    ///
    /// `&mut T` returns a write access to the storage.
    ///
    /// To retrive multiple storages at once, use a tuple.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let all_storages = world.all_storages();
    /// let (usizes, u32s) = all_storages.get_storage::<(&mut usize, &u32)>();
    /// ```
    pub fn get_storage<T: GetStorage<'a>>(&self) -> T::Storage {
        self.try_get_storage::<T>().unwrap()
    }
    /// Retrives storages based on type `T`.
    ///
    /// `&T` returns a read access to the storage.
    ///
    /// `&mut T` returns a write access to the storage.
    ///
    /// To retrive multiple storages at once, use a tuple.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let all_storages = world.all_storages();
    /// let (usizes, u32s) = all_storages.try_get_storage::<(&mut usize, &u32)>().unwrap();
    /// ```
    pub fn try_get_storage<T: GetStorage<'a>>(&self) -> Result<T::Storage, error::GetStorage> {
        T::get_in(Ref::clone(self))
    }
}

impl<'a> RefMut<'a, AllStorages> {
    /// Retrives storages based on type `T` consuming the `RefMut<AllStorages>` in the process
    /// to only borrow it immutably.
    ///
    /// `&T` returns a read access to the storage.
    ///
    /// `&mut T` returns a write access to the storage.
    ///
    /// To retrive multiple storages at once, use a tuple.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let all_storages = world.all_storages_mut();
    /// let (usizes, u32s) = all_storages.get_storage::<(&mut usize, &u32)>();
    /// ```
    pub fn get_storage<T: GetStorage<'a>>(self) -> T::Storage {
        self.try_get_storage::<T>().unwrap()
    }
    /// Retrives storages based on type `T` consuming the `RefMut<AllStorages>` in the process
    /// to only borrow it immutably.
    ///
    /// `&T` returns a read access to the storage.
    ///
    /// `&mut T` returns a write access to the storage.
    ///
    /// To retrive multiple storages at once, use a tuple.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let all_storages = world.all_storages_mut();
    /// let (usizes, u32s) = all_storages.try_get_storage::<(&mut usize, &u32)>().unwrap();
    /// ```
    pub fn try_get_storage<T: GetStorage<'a>>(self) -> Result<T::Storage, error::GetStorage> {
        RefMut::downgrade(self).try_get_storage::<T>()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn delete() {
        let mut storage = ComponentStorage::new::<&'static str>();
        storage.array_mut().unwrap().insert("test5", 5);
        storage.array_mut().unwrap().insert("test10", 10);
        storage.array_mut().unwrap().insert("test1", 1);
        storage.delete(5).unwrap();
        assert_eq!(storage.array::<&str>().unwrap().get(5), None);
        assert_eq!(storage.array::<&str>().unwrap().get(10), Some(&"test10"));
        assert_eq!(storage.array::<&str>().unwrap().get(1), Some(&"test1"));
        storage.delete(10).unwrap();
        storage.delete(1).unwrap();
        assert_eq!(storage.array::<&str>().unwrap().get(5), None);
        assert_eq!(storage.array::<&str>().unwrap().get(10), None);
        assert_eq!(storage.array::<&str>().unwrap().get(1), None);
    }
}
