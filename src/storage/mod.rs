mod all;
mod entity;
mod storage_id;
mod unique;

pub use all::{AllStorages, CustomDeleteAny, DeleteAny, Retain};
pub use entity::{Entities, EntitiesIter, EntityId};
pub use storage_id::StorageId;
pub use unique::Unique;

// #[cfg(feature = "serde1")]
// pub(crate) use all::AllStoragesSerializer;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::unknown_storage::UnknownStorage;
use alloc::boxed::Box;
#[cfg(feature = "non_send")]
use std::thread::ThreadId;
// #[cfg(feature = "serde1")]
// use crate::serde_setup::GlobalDeConfig;
// #[cfg(feature = "serde1")]
// use hashbrown::HashMap;

/// Abstract away `T` from `AtomicRefCell<T>` to be able to store
/// different types in a `HashMap<TypeId, Storage>`.  
/// and box the `AtomicRefCell` so it doesn't move when the `HashMap` reallocates
pub(crate) struct Storage(*mut AtomicRefCell<dyn UnknownStorage>);

#[cfg(not(feature = "non_send"))]
unsafe impl Send for Storage {}

unsafe impl Sync for Storage {}

impl Drop for Storage {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.0);
        }
    }
}

impl Storage {
    #[inline]
    fn new<T: UnknownStorage + Send + Sync + 'static>(value: T) -> Self {
        Storage(Box::into_raw(Box::new(AtomicRefCell::new(value))))
    }
    #[cfg(feature = "non_send")]
    #[inline]
    fn new_non_send<T: UnknownStorage + Sync + 'static>(value: T, thread_id: ThreadId) -> Self {
        Storage(Box::into_raw(Box::new(AtomicRefCell::new_non_send(
            value, thread_id,
        ))))
    }
    #[cfg(feature = "non_sync")]
    #[inline]
    fn new_non_sync<T: UnknownStorage + Send + 'static>(value: T) -> Self {
        Storage(Box::into_raw(Box::new(AtomicRefCell::new_non_sync(value))))
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[inline]
    fn new_non_send_sync<T: UnknownStorage + 'static>(value: T, thread_id: ThreadId) -> Self {
        Storage(Box::into_raw(Box::new(AtomicRefCell::new_non_send_sync(
            value, thread_id,
        ))))
    }
    #[inline]
    fn get<T: 'static>(&self) -> Result<Ref<'_, &T>, error::Borrow> {
        Ok(Ref::map(unsafe { &*self.0 }.try_borrow()?, |storage| {
            storage.any().downcast_ref::<T>().unwrap()
        }))
    }
    #[inline]
    fn get_mut<T: 'static>(&self) -> Result<RefMut<'_, &mut T>, error::Borrow> {
        Ok(RefMut::map(
            unsafe { &*self.0 }.try_borrow_mut()?,
            |storage| storage.any_mut().downcast_mut().unwrap(),
        ))
    }
    #[inline]
    fn get_mut_exclusive<T: 'static>(&mut self) -> &mut T {
        // SAFE this is not `AllStorages`
        unsafe { (&mut *self.0).get_mut() }
            .any_mut()
            .downcast_mut()
            .unwrap()
    }
    #[inline]
    fn share(&mut self, owned: EntityId, shared: EntityId) {
        // SAFE this is not `AllStorages`
        unsafe { (&mut *self.0).get_mut() }.share(owned, shared);
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
    use crate::sparse_set::SparseSet;

    let storage = Storage(Box::into_raw(Box::new(AtomicRefCell::new(SparseSet::<
        &'static str,
    >::new()))));
    let mut entity_id = EntityId::zero();
    entity_id.set_index(5);
    storage
        .get_mut::<SparseSet<&'static str>>()
        .unwrap()
        .insert(entity_id, "test5");
    entity_id.set_index(10);
    storage
        .get_mut::<SparseSet<&'static str>>()
        .unwrap()
        .insert(entity_id, "test10");
    entity_id.set_index(1);
    storage
        .get_mut::<SparseSet<&'static str>>()
        .unwrap()
        .insert(entity_id, "test1");
    entity_id.set_index(5);
    unsafe { &*storage.0 }
        .try_borrow_mut()
        .unwrap()
        .delete(entity_id);
    assert_eq!(
        storage
            .get_mut::<SparseSet::<&'static str>>()
            .unwrap()
            .private_get(entity_id),
        None
    );
    entity_id.set_index(10);
    assert_eq!(
        storage
            .get_mut::<SparseSet::<&'static str>>()
            .unwrap()
            .private_get(entity_id),
        Some(&"test10")
    );
    entity_id.set_index(1);
    assert_eq!(
        storage
            .get_mut::<SparseSet::<&'static str>>()
            .unwrap()
            .private_get(entity_id),
        Some(&"test1")
    );
    entity_id.set_index(10);
    unsafe { &*storage.0 }
        .try_borrow_mut()
        .unwrap()
        .delete(entity_id);
    entity_id.set_index(1);
    unsafe { &*storage.0 }
        .try_borrow_mut()
        .unwrap()
        .delete(entity_id);
    entity_id.set_index(5);
    assert_eq!(
        storage
            .get_mut::<SparseSet::<&'static str>>()
            .unwrap()
            .private_get(entity_id),
        None
    );
    entity_id.set_index(10);
    assert_eq!(
        storage
            .get_mut::<SparseSet::<&'static str>>()
            .unwrap()
            .private_get(entity_id),
        None
    );
    entity_id.set_index(1);
    assert_eq!(
        storage
            .get_mut::<SparseSet::<&'static str>>()
            .unwrap()
            .private_get(entity_id),
        None
    );
}
