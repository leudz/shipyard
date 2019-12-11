mod hasher;
mod view;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::entity::{Entities, Key};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unknown_storage::UnknownStorage;
use hasher::TypeIdHasher;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
pub(crate) use view::AllStoragesViewMut;

/// Abstract away `T` from `SparseSet<T>` to be able to store
/// different types in a `HashMap<TypeId, Storage>`.
/// `unknown` is the address of the vtable part of the `SparseSet`'s `UnknownStorage` implementation.
pub(crate) struct Storage {
    data: AtomicRefCell<Box<dyn Any + Send + Sync>>,
    unknown: *const (),
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
            data: AtomicRefCell::new(Box::new(sparse_set)),
            unknown: unknown[1],
        }
    }
    /// Immutably borrows the container.
    pub(crate) fn sparse_set<T: 'static>(&self) -> Result<Ref<SparseSet<T>>, error::Borrow> {
        Ok(Ref::map(self.data.try_borrow()?, |sparse_set| {
            sparse_set.downcast_ref().unwrap()
        }))
    }
    pub(crate) fn entities(&self) -> Result<Ref<Entities>, error::Borrow> {
        Ok(Ref::map(self.data.try_borrow()?, |entities| {
            entities.downcast_ref().unwrap()
        }))
    }
    pub(crate) fn entities_mut(&self) -> Result<RefMut<Entities>, error::Borrow> {
        Ok(RefMut::map(self.data.try_borrow_mut()?, |entities| {
            entities.downcast_mut().unwrap()
        }))
    }
    /// Mutably borrows the container.
    pub(crate) fn sparse_set_mut<T: 'static>(&self) -> Result<RefMut<SparseSet<T>>, error::Borrow> {
        Ok(RefMut::map(self.data.try_borrow_mut()?, |sparse_set| {
            sparse_set.downcast_mut().unwrap()
        }))
    }
    /// Mutably borrows the container and delete `index`.
    pub(crate) fn delete(&mut self, entity: Key) -> Result<&[TypeId], error::Borrow> {
        // reconstruct a `dyn UnknownStorage` from two pointers
        // for a full explanation see UnknownStorage documentation
        let sparse_set: RefMut<Box<dyn Any + Send + Sync>> = self.data.try_borrow_mut()?;
        let sparse_set = &**sparse_set as *const dyn Any as *const ();
        let unknown: &mut dyn UnknownStorage = unsafe {
            &mut **(&[sparse_set, self.unknown] as *const _ as *const *mut dyn UnknownStorage)
        };
        Ok(unknown.delete(entity))
    }
    pub(crate) fn unpack(&mut self, entity: Key) -> Result<(), error::Borrow> {
        // reconstruct a `dyn UnknownStorage` from two pointers
        // for a full explanation see UnknownStorage documentation
        let sparse_set: RefMut<Box<dyn Any + Send + Sync>> = self.data.try_borrow_mut()?;
        let sparse_set = &**sparse_set as *const dyn Any as *const ();
        let unknown: &mut dyn UnknownStorage = unsafe {
            &mut **(&[sparse_set, self.unknown] as *const _ as *const *mut dyn UnknownStorage)
        };
        unknown.unpack(entity);
        Ok(())
    }
}

/// Contains all components present in the World.
// Wrapper to hide `TypeIdHasher` and the whole `HashMap` from public interface
pub struct AllStorages(pub(crate) HashMap<TypeId, Storage, BuildHasherDefault<TypeIdHasher>>);

impl Default for AllStorages {
    fn default() -> Self {
        let mut storages = HashMap::default();

        let entities = Entities::default();
        let unknown: [*const (); 2] = unsafe {
            *(&(&entities as &dyn UnknownStorage as *const _) as *const *const _
                as *const [*const (); 2])
        };

        storages.insert(
            TypeId::of::<Entities>(),
            Storage {
                data: AtomicRefCell::new(Box::new(entities)),
                unknown: unknown[1],
            },
        );

        AllStorages(storages)
    }
}

impl AllStorages {
    /// Register a new component type and create a storage for it.
    /// Does nothing if a storage already exists.
    pub(crate) fn register<T: 'static + Send + Sync>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(Storage::new::<T>);
    }
    /// Register a new unique component and create a storage for it.
    /// Does nothing if a storage already exists.
    pub(crate) fn register_unique<T: 'static + Send + Sync>(&mut self, componnent: T) {
        let type_id = TypeId::of::<T>();
        let storage = Storage::new::<T>();
        storage.sparse_set_mut().unwrap().insert_unique(componnent);
        if let Some(storage) = self.0.insert(type_id, storage) {
            *self.0.get_mut(&type_id).unwrap() = storage;
        }
    }
    pub(crate) fn view_mut(&mut self) -> AllStoragesViewMut {
        AllStoragesViewMut(&mut self.0)
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
