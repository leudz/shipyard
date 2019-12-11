mod all;
mod entity;

pub use all::{AllStorages, AllStoragesViewMut};
pub use entity::{Entities, EntitiesMut, EntitiesView, EntitiesViewMut, Key};

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unknown_storage::UnknownStorage;
use std::any::{Any, TypeId};

/// Abstract away `T` from `AtomicRefCell<T>` to be able to store
/// different types in a `HashMap<TypeId, Storage>`.\
/// `unknown` is the address of the vtable part of the storage's `UnknownStorage` implementation.
pub(crate) struct Storage {
    pub(super) container: AtomicRefCell<Box<dyn Any + Send + Sync>>,
    pub(super) unknown: *const (),
}

impl Storage {
    /// Creates a new `Storage` storing elements of type T.
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        let sparse_set = SparseSet::<T>::default();
        // store the vtable of this trait object
        // for a full explanation see UnknownStorage documentation
        let unknown: [*const (); 2] = unsafe {
            *(&(&sparse_set as &dyn UnknownStorage as *const _) as *const *const _
                as *const [*const _; 2])
        };
        Storage {
            container: AtomicRefCell::new(Box::new(sparse_set)),
            unknown: unknown[1],
        }
    }
    /// Immutably borrows the component container.
    pub(crate) fn sparse_set<T: 'static>(&self) -> Result<Ref<SparseSet<T>>, error::Borrow> {
        Ok(Ref::map(self.container.try_borrow()?, |sparse_set| {
            sparse_set.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows the component container.
    pub(crate) fn sparse_set_mut<T: 'static>(&self) -> Result<RefMut<SparseSet<T>>, error::Borrow> {
        Ok(RefMut::map(
            self.container.try_borrow_mut()?,
            |sparse_set| sparse_set.downcast_mut().unwrap(),
        ))
    }
    /// Immutably borrows entities' storage.
    pub(crate) fn entities(&self) -> Result<Ref<Entities>, error::Borrow> {
        Ok(Ref::map(self.container.try_borrow()?, |entities| {
            entities.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows entities' storage.
    pub(crate) fn entities_mut(&self) -> Result<RefMut<Entities>, error::Borrow> {
        Ok(RefMut::map(self.container.try_borrow_mut()?, |entities| {
            entities.downcast_mut().unwrap()
        }))
    }
    /// Mutably borrows the container and delete `index`.
    pub(crate) fn delete(&mut self, entity: Key) -> Result<&[TypeId], error::Borrow> {
        // reconstruct a `dyn UnknownStorage` from two pointers
        // for a full explanation see UnknownStorage documentation
        let container: RefMut<Box<dyn Any + Send + Sync>> = self.container.try_borrow_mut()?;
        let container = &**container as *const dyn Any as *const ();
        let unknown: &mut dyn UnknownStorage = unsafe {
            &mut **(&[container, self.unknown] as *const _ as *const *mut dyn UnknownStorage)
        };
        Ok(unknown.delete(entity))
    }
    pub(crate) fn unpack(&mut self, entity: Key) -> Result<(), error::Borrow> {
        // reconstruct a `dyn UnknownStorage` from two pointers
        // for a full explanation see UnknownStorage documentation
        let container: RefMut<Box<dyn Any + Send + Sync>> = self.container.try_borrow_mut()?;
        let container = &**container as *const dyn Any as *const ();
        let unknown: &mut dyn UnknownStorage = unsafe {
            &mut **(&[container, self.unknown] as *const _ as *const *mut dyn UnknownStorage)
        };
        unknown.unpack(entity);
        Ok(())
    }
}

#[test]
fn delete() {
    let mut storage = Storage::new::<&'static str>();
    let mut key = Key::zero();
    key.set_index(5);
    storage
        .sparse_set_mut()
        .unwrap()
        .view_mut()
        .insert("test5", key);
    key.set_index(10);
    storage
        .sparse_set_mut()
        .unwrap()
        .view_mut()
        .insert("test10", key);
    key.set_index(1);
    storage
        .sparse_set_mut()
        .unwrap()
        .view_mut()
        .insert("test1", key);
    key.set_index(5);
    storage.delete(key).unwrap();
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(key), None);
    key.set_index(10);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().get(key),
        Some(&"test10")
    );
    key.set_index(1);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().get(key),
        Some(&"test1")
    );
    key.set_index(10);
    storage.delete(key).unwrap();
    key.set_index(1);
    storage.delete(key).unwrap();
    key.set_index(5);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(key), None);
    key.set_index(10);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(key), None);
    key.set_index(1);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(key), None);
}
