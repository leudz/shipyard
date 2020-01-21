mod all;
mod entity;

pub use all::AllStorages;
pub use entity::{Entities, EntitiesMut, EntityId};

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unknown_storage::UnknownStorage;
use std::any::{Any, TypeId};

pub enum StorageId {
    TypeId(TypeId),
    Custom(u64),
}

impl From<TypeId> for StorageId {
    fn from(type_id: TypeId) -> Self {
        StorageId::TypeId(type_id)
    }
}

impl From<u64> for StorageId {
    fn from(int: u64) -> Self {
        StorageId::Custom(int)
    }
}

/// Abstract away `T` from `AtomicRefCell<T>` to be able to store
/// different types in a `HashMap<TypeId, Storage>`.\
/// and box the `AtomicRefCell` so it doesn't move when the `HashMap` reallocates
/// `unknown` is the address of the vtable part of the storage's `UnknownStorage` implementation.
pub(crate) struct Storage {
    pub(super) container: Box<AtomicRefCell<dyn Any>>,
    pub(super) unknown: *const (),
}

#[cfg(not(feature = "non_send"))]
unsafe impl Send for Storage {}

unsafe impl Sync for Storage {}

impl Storage {
    /// Creates a new `Storage` storing elements of type T.
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        let sparse_set = SparseSet::<T>::default();
        // store the vtable of this trait object
        // for a full explanation see UnknownStorage documentation
        let unknown: [*const (); 2] = unsafe {
            let unknown_storage: &dyn UnknownStorage = &sparse_set;
            let unknown_storage: *const _ = unknown_storage;
            let unknown_storage: *const *const _ = &unknown_storage;
            *(unknown_storage as *const [*const (); 2])
        };
        Storage {
            container: Box::new(AtomicRefCell::new(sparse_set, None, true)),
            unknown: unknown[1],
        }
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn new_non_send<T: 'static + Sync>() -> Self {
        let sparse_set = SparseSet::<T>::default();
        // store the vtable of this trait object
        // for a full explanation see UnknownStorage documentation
        let unknown: [*const (); 2] = unsafe {
            let unknown_storage: &dyn UnknownStorage = &sparse_set;
            let unknown_storage: *const _ = unknown_storage;
            let unknown_storage: *const *const _ = &unknown_storage;
            *(unknown_storage as *const [*const (); 2])
        };
        Storage {
            container: Box::new(AtomicRefCell::new(
                sparse_set,
                Some(std::thread::current().id()),
                true,
            )),
            unknown: unknown[1],
        }
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn new_non_sync<T: 'static + Send>() -> Self {
        let sparse_set = SparseSet::<T>::default();
        // store the vtable of this trait object
        // for a full explanation see UnknownStorage documentation
        let unknown: [*const (); 2] = unsafe {
            let unknown_storage: &dyn UnknownStorage = &sparse_set;
            let unknown_storage: *const _ = unknown_storage;
            let unknown_storage: *const *const _ = &unknown_storage;
            *(unknown_storage as *const [*const (); 2])
        };
        Storage {
            container: Box::new(AtomicRefCell::new(sparse_set, None, false)),
            unknown: unknown[1],
        }
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn new_non_send_sync<T: 'static>() -> Self {
        let sparse_set = SparseSet::<T>::default();
        // store the vtable of this trait object
        // for a full explanation see UnknownStorage documentation
        let unknown: [*const (); 2] = unsafe {
            let unknown_storage: &dyn UnknownStorage = &sparse_set;
            let unknown_storage: *const _ = unknown_storage;
            let unknown_storage: *const *const _ = &unknown_storage;
            *(unknown_storage as *const [*const (); 2])
        };
        Storage {
            container: Box::new(AtomicRefCell::new(
                sparse_set,
                Some(std::thread::current().id()),
                false,
            )),
            unknown: unknown[1],
        }
    }
    /// Immutably borrows the component container.
    pub(crate) fn sparse_set<T: 'static>(&self) -> Result<Ref<'_, SparseSet<T>>, error::Borrow> {
        Ok(Ref::map(self.container.try_borrow()?, |sparse_set| {
            sparse_set.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows the component container.
    pub(crate) fn sparse_set_mut<T: 'static>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::Borrow> {
        Ok(RefMut::map(
            self.container.try_borrow_mut()?,
            |sparse_set| sparse_set.downcast_mut().unwrap(),
        ))
    }
    /// Immutably borrows entities' storage.
    pub(crate) fn entities(&self) -> Result<Ref<'_, Entities>, error::Borrow> {
        Ok(Ref::map(self.container.try_borrow()?, |entities| {
            entities.downcast_ref().unwrap()
        }))
    }
    /// Mutably borrows entities' storage.
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, Entities>, error::Borrow> {
        Ok(RefMut::map(self.container.try_borrow_mut()?, |entities| {
            entities.downcast_mut().unwrap()
        }))
    }
    /// Mutably borrows the container and delete `index`.
    pub(crate) fn delete(&mut self, entity: EntityId) -> Result<&[TypeId], error::Borrow> {
        // reconstruct a `dyn UnknownStorage` from two pointers
        // for a full explanation see UnknownStorage documentation
        let container: RefMut<'_, dyn Any> = self.container.try_borrow_mut()?;
        let container: &dyn Any = &*container;
        let container: *const dyn Any = container;
        let container: *const () = container as _;
        let unknown: &mut dyn UnknownStorage = unsafe {
            let unknown_storage: *const _ = &[container, self.unknown];
            let unknown_storage: *const *mut dyn UnknownStorage = unknown_storage as _;
            &mut **unknown_storage
        };
        Ok(unknown.delete(entity))
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) -> Result<(), error::Borrow> {
        // reconstruct a `dyn UnknownStorage` from two pointers
        // for a full explanation see UnknownStorage documentation
        let container: RefMut<'_, dyn Any> = self.container.try_borrow_mut()?;
        let container: &dyn Any = &*container;
        let container: *const dyn Any = container;
        let container: *const () = container as _;
        let unknown: &mut dyn UnknownStorage = unsafe {
            let unknown_storage: *const _ = &[container, self.unknown];
            let unknown_storage: *const *mut dyn UnknownStorage = unknown_storage as _;
            &mut **unknown_storage
        };
        unknown.unpack(entity);
        Ok(())
    }
}

#[test]
fn delete() {
    let mut storage = Storage::new::<&'static str>();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(5);
    storage.sparse_set_mut().unwrap().insert("test5", entity_id);
    entity_id.set_index(10);
    storage
        .sparse_set_mut()
        .unwrap()
        .insert("test10", entity_id);
    entity_id.set_index(1);
    storage.sparse_set_mut().unwrap().insert("test1", entity_id);
    entity_id.set_index(5);
    storage.delete(entity_id).unwrap();
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().get(entity_id),
        Some(&"test10")
    );
    entity_id.set_index(1);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().get(entity_id),
        Some(&"test1")
    );
    entity_id.set_index(10);
    storage.delete(entity_id).unwrap();
    entity_id.set_index(1);
    storage.delete(entity_id).unwrap();
    entity_id.set_index(5);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
    entity_id.set_index(1);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
}
