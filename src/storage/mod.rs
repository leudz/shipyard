mod all;
mod entity;
mod storage_id;
mod unique;

pub use all::{AllStorages, DeleteAny};
pub use entity::{Entities, EntitiesIter, EntityId};
pub use storage_id::StorageId;
pub use unique::Unique;

pub(crate) use crate::type_id::TypeIdHasher;
// #[cfg(feature = "serde1")]
// pub(crate) use all::AllStoragesSerializer;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
// #[cfg(feature = "serde1")]
// use crate::serde_setup::GlobalDeConfig;
use crate::sparse_set::SparseSet;
use crate::type_id::TypeId;
use crate::unknown_storage::UnknownStorage;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::type_name;
// #[cfg(feature = "serde1")]
// use hashbrown::HashMap;

/// Abstract away `T` from `AtomicRefCell<T>` to be able to store
/// different types in a `HashMap<TypeId, Storage>`.  
/// and box the `AtomicRefCell` so it doesn't move when the `HashMap` reallocates
pub(crate) struct Storage(pub(crate) Box<AtomicRefCell<dyn UnknownStorage>>);

#[cfg(not(feature = "non_send"))]
unsafe impl Send for Storage {}

unsafe impl Sync for Storage {}

impl Storage {
    /// Creates a new `Storage` storing elements of type T.
    #[inline]
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new(sparse_set)))
    }
    #[cfg(feature = "non_send")]
    #[inline]
    pub(crate) fn new_non_send<T: 'static + Sync>(world_thread_id: std::thread::ThreadId) -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new_non_send(
            sparse_set,
            world_thread_id,
        )))
    }
    #[cfg(feature = "non_sync")]
    #[inline]
    pub(crate) fn new_non_sync<T: 'static + Send>() -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new_non_sync(sparse_set)))
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[inline]
    pub(crate) fn new_non_send_sync<T: 'static>(world_thread_id: std::thread::ThreadId) -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new_non_send_sync(
            sparse_set,
            world_thread_id,
        )))
    }
    #[inline]
    pub(crate) fn new_unique<T: 'static + Send + Sync>(component: T) -> Self {
        Storage(Box::new(AtomicRefCell::new(Unique(component))))
    }
    #[cfg(feature = "non_send")]
    #[inline]
    pub(crate) fn new_unique_non_send<T: 'static + Sync>(
        component: T,
        world_thread_id: std::thread::ThreadId,
    ) -> Self {
        Storage(Box::new(AtomicRefCell::new_non_send(
            Unique(component),
            world_thread_id,
        )))
    }
    #[cfg(feature = "non_sync")]
    #[inline]
    pub(crate) fn new_unique_non_sync<T: 'static + Send>(component: T) -> Self {
        Storage(Box::new(AtomicRefCell::new_non_sync(Unique(component))))
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[inline]
    pub(crate) fn new_unique_non_send_sync<T: 'static>(
        component: T,
        world_thread_id: std::thread::ThreadId,
    ) -> Self {
        Storage(Box::new(AtomicRefCell::new_non_send_sync(
            Unique(component),
            world_thread_id,
        )))
    }
    /// Immutably borrows the component container.
    #[inline]
    pub(crate) fn sparse_set<T: 'static>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::GetStorage> {
        let storage = self
            .0
            .try_borrow()
            .map_err(|borrow| error::GetStorage::StorageBorrow((type_name::<T>(), borrow)))?;

        Ok(Ref::map(storage, |unknown| {
            unknown.sparse_set::<T>().unwrap()
        }))
    }
    /// Mutably borrows the component container.
    #[inline]
    pub(crate) fn sparse_set_mut<T: 'static>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::GetStorage> {
        let storage = self
            .0
            .try_borrow_mut()
            .map_err(|borrow| error::GetStorage::StorageBorrow((type_name::<T>(), borrow)))?;

        Ok(RefMut::map(storage, |unknown| {
            unknown.sparse_set_mut::<T>().unwrap()
        }))
    }
    /// Immutably borrows entities' storage.
    #[inline]
    pub(crate) fn entities(&self) -> Result<Ref<'_, Entities>, error::Borrow> {
        Ok(Ref::map(self.0.try_borrow()?, |unknown| {
            unknown.entities().unwrap()
        }))
    }
    /// Mutably borrows entities' storage.
    #[inline]
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, Entities>, error::Borrow> {
        Ok(RefMut::map(self.0.try_borrow_mut()?, |unknown| {
            unknown.entities_mut().unwrap()
        }))
    }
    #[inline]
    pub(crate) fn unique<T: 'static>(&self) -> Result<Ref<'_, T>, error::GetStorage> {
        let storage = self
            .0
            .try_borrow()
            .map_err(|borrow| error::GetStorage::StorageBorrow((type_name::<T>(), borrow)))?;

        Ok(Ref::map(storage, |unknown| unknown.unique::<T>().unwrap()))
    }
    #[inline]
    pub(crate) fn unique_mut<T: 'static>(&self) -> Result<RefMut<'_, T>, error::GetStorage> {
        let storage = self
            .0
            .try_borrow_mut()
            .map_err(|borrow| error::GetStorage::StorageBorrow((type_name::<T>(), borrow)))?;

        Ok(RefMut::map(storage, |unknown| {
            unknown.unique_mut::<T>().unwrap()
        }))
    }
    /// Mutably borrows the container and delete `index`.
    pub(crate) fn delete(
        &mut self,
        entity: EntityId,
        storage_to_unpack: &mut Vec<TypeId>,
    ) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.delete(entity, storage_to_unpack);
        Ok(())
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.unpack(entity);
        Ok(())
    }
    pub(crate) fn clear(&mut self) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.clear();
        Ok(())
    }
    pub(crate) fn share(&mut self, owned: EntityId, shared: EntityId) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.share(owned, shared);
        Ok(())
    }
}

// #[cfg(feature = "serde1")]
// pub(crate) struct StorageDeserializer<'a> {
//     pub(crate) storage: &'a mut Storage,
//     pub(crate) entities_map: &'a HashMap<EntityId, EntityId>,
//     pub(crate) de_config: GlobalDeConfig,
// }

// #[cfg(feature = "serde1")]
// impl<'de> serde::de::DeserializeSeed<'de> for StorageDeserializer<'_> {
//     type Value = ();

//     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         let deserializer: &mut dyn crate::erased_serde::Deserializer<'de> =
//             &mut crate::erased_serde::Deserializer::erase(deserializer);

//         let storage = self
//             .storage
//             .0
//             .try_borrow_mut()
//             .map_err(|err| serde::de::Error::custom(err))?;
//         let de = storage
//             .deserialize()
//             .ok_or_else(|| serde::de::Error::custom("Type isn't serializable."))?;
//         drop(storage);

//         *self.storage = (de)(self.de_config, self.entities_map, deserializer)
//             .map_err(serde::de::Error::custom)?;

//         Ok(())
//     }
// }

#[test]
fn delete() {
    let mut storage = Storage::new::<&'static str>();
    let mut entity_id = EntityId::zero();
    let mut storage_to_unpack = Vec::new();
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
    storage.delete(entity_id, &mut storage_to_unpack).unwrap();
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().private_get(entity_id),
        None
    );
    entity_id.set_index(10);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().private_get(entity_id),
        Some(&"test10")
    );
    entity_id.set_index(1);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().private_get(entity_id),
        Some(&"test1")
    );
    entity_id.set_index(10);
    storage.delete(entity_id, &mut storage_to_unpack).unwrap();
    entity_id.set_index(1);
    storage.delete(entity_id, &mut storage_to_unpack).unwrap();
    entity_id.set_index(5);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().private_get(entity_id),
        None
    );
    entity_id.set_index(10);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().private_get(entity_id),
        None
    );
    entity_id.set_index(1);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().private_get(entity_id),
        None
    );
}
