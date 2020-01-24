mod hasher;

use super::{Entities, EntityId, Storage};
use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unknown_storage::UnknownStorage;
use core::cell::UnsafeCell;
pub(crate) use hasher::TypeIdHasher;
use parking_lot::{lock_api::RawRwLock as _, RawRwLock};
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// Contains all components present in the World.
// The lock is held very briefly:
// - shared: when trying to find a storage
// - unique: when adding a storage
// once the storage is found or created the lock is released
// this is safe since World is still borrowed and there is no way to delete a storage
// so any access to storages are valid as long as the World exists
// we use a HashMap, it can reallocate, but even in this case the storages won't move since they are boxed
pub struct AllStorages {
    lock: RawRwLock,
    storages: UnsafeCell<HashMap<TypeId, Storage, BuildHasherDefault<TypeIdHasher>>>,
}

impl Default for AllStorages {
    fn default() -> Self {
        let mut storages = HashMap::default();

        let entities = Entities::default();
        let unknown: [*const (); 2] = unsafe {
            let unknown: &dyn UnknownStorage = &entities;
            let unknown: *const _ = unknown;
            let unknown: *const *const _ = &unknown;
            *(unknown as *const [*const (); 2])
        };

        storages.insert(
            TypeId::of::<Entities>(),
            Storage {
                container: Box::new(AtomicRefCell::new(entities, None, true)),
                unknown: unknown[1],
            },
        );

        AllStorages {
            storages: UnsafeCell::new(storages),
            lock: RawRwLock::INIT,
        }
    }
}

impl AllStorages {
    pub(crate) fn entities(&self) -> Result<Ref<'_, Entities>, error::Borrow> {
        let type_id = TypeId::of::<Entities>();
        self.lock.lock_shared();
        // SAFE we locked
        let storages = unsafe { &*self.storages.get() };
        // AllStorages is always created with Entities so there's no way to not find it
        let storage = &storages[&type_id];
        match storage.entities() {
            Ok(entities) => {
                self.lock.unlock_shared();
                Ok(entities)
            }
            Err(err) => {
                self.lock.unlock_shared();
                Err(err)
            }
        }
    }
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, Entities>, error::Borrow> {
        let type_id = TypeId::of::<Entities>();
        self.lock.lock_shared();
        // SAFE we locked
        let storages = unsafe { &*self.storages.get() };
        // AllStorages is always created with Entities so there's no way to not find it
        let storage = &storages[&type_id];
        match storage.entities_mut() {
            Ok(entities) => {
                self.lock.unlock_shared();
                Ok(entities)
            }
            Err(err) => {
                self.lock.unlock_shared();
                Err(err)
            }
        }
    }
    pub(crate) fn get<T: 'static + Send + Sync>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::Borrow> {
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new::<T>)
            .sparse_set::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    pub(crate) fn get_mut<T: 'static + Send + Sync>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::Borrow> {
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new::<T>)
            .sparse_set_mut::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn get_non_send<T: 'static + Sync>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::Borrow> {
        // Sync components can be accessed by any thread with a shared access
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_send::<T>)
            .sparse_set::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn get_non_send_mut<T: 'static + Sync>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::Borrow> {
        // Sync components can only be accessed by the thread they were created in with a unique access
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_send::<T>)
            .sparse_set_mut::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn get_non_sync<T: 'static + Send>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::Borrow> {
        // Send components can be accessed by one thread at a time
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_sync::<T>)
            .sparse_set::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn get_non_sync_mut<T: 'static + Send>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::Borrow> {
        // Send components can be accessed by one thread at a time
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_sync::<T>)
            .sparse_set_mut::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn get_non_send_sync<T: 'static>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::Borrow> {
        // !Send + !Sync components can only be accessed by the thread they were created in
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_send_sync::<T>)
            .sparse_set::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn get_non_send_sync_mut<T: 'static>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::Borrow> {
        // !Send + !Sync components can only be accessed by the thread they were created in
        let type_id = TypeId::of::<T>();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                self.lock.unlock_shared();
                return sparse_set;
            }
        }
        self.lock.unlock_shared();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_send_sync::<T>)
            .sparse_set_mut::<T>();
        self.lock.unlock_exclusive();
        sparse_set
    }
    /// Register a new unique component and create a storage for it.
    /// Does nothing if a storage already exists.
    pub(crate) fn register_unique<T: 'static + Send + Sync>(&self, component: T) {
        self.get_mut::<T>().unwrap().insert_unique(component)
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn register_unique_non_send<T: 'static + Sync>(&self, component: T) {
        self.get_non_send_mut::<T>()
            .unwrap()
            .insert_unique(component)
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn register_unique_non_sync<T: 'static + Send>(&self, component: T) {
        self.get_non_sync_mut::<T>()
            .unwrap()
            .insert_unique(component)
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn register_unique_non_send_sync<T: 'static>(&self, component: T) {
        self.get_non_send_sync_mut::<T>()
            .unwrap()
            .insert_unique(component)
    }
    /// Delete an entity and all its components.
    /// Returns `true` if `entity` was alive.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();
    ///
    /// let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    /// let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    /// drop((entities, usizes, u32s));
    /// world.run::<AllStorages, _, _>(|mut all_storages| {
    ///     all_storages.delete(entity1);
    /// });
    ///
    /// world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
    ///     assert_eq!((&usizes).get(entity1), None);
    ///     assert_eq!((&u32s).get(entity1), None);
    ///     assert_eq!(usizes.get(entity2), Some(&2));
    ///     assert_eq!(u32s.get(entity2), Some(&3));
    /// });
    /// ```
    pub fn delete(&mut self, entity: EntityId) -> bool {
        // no need to lock here since we have a unique access
        let mut entities = self.entities_mut().unwrap();

        if entities.delete(entity) {
            drop(entities);

            self.strip(entity);

            true
        } else {
            false
        }
    }
    /// Deletes all components from an entity without deleting it.
    pub fn strip(&mut self, entity: EntityId) {
        // no need to lock here since we have a unique access
        let mut storage_to_unpack = Vec::new();
        // SAFE we have unique access
        let storages = unsafe { &mut *self.storages.get() };

        for storage in storages.values_mut() {
            let observers = storage.delete(entity).unwrap();
            storage_to_unpack.reserve(observers.len());

            let mut i = 0;
            for observer in observers.iter().copied() {
                while i < storage_to_unpack.len() && observer < storage_to_unpack[i] {
                    i += 1;
                }
                if storage_to_unpack.is_empty() || observer != storage_to_unpack[i] {
                    storage_to_unpack.insert(i, observer);
                }
            }
        }

        for storage in storage_to_unpack {
            storages.get_mut(&storage).unwrap().unpack(entity).unwrap();
        }
    }
}
