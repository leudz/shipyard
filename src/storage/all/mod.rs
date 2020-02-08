mod hasher;

use super::{Entities, EntityId, Storage};
use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::run::StorageBorrow;
use crate::sparse_set::SparseSet;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::cell::UnsafeCell;
use core::hash::BuildHasherDefault;
use hashbrown::HashMap;
pub(crate) use hasher::TypeIdHasher;
use parking_lot::{lock_api::RawRwLock as _, RawRwLock};

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

        #[cfg(feature = "std")]
        {
            storages.insert(
                TypeId::of::<Entities>(),
                Storage(Box::new(AtomicRefCell::new(entities, None, true))),
            );
        }
        #[cfg(not(feature = "std"))]
        {
            storages.insert(
                TypeId::of::<Entities>(),
                Storage(Box::new(AtomicRefCell::new(entities))),
            );
        }

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
    ///     assert!((&usizes).get(entity1).is_err());
    ///     assert!((&u32s).get(entity1).is_err());
    ///     assert_eq!(usizes.get(entity2), Ok(&2));
    ///     assert_eq!(u32s.get(entity2), Ok(&3));
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
            // we have unique access to all storages so we can unwrap
            storage.delete(entity, &mut storage_to_unpack).unwrap();
        }

        for storage in storage_to_unpack {
            storages.get_mut(&storage).unwrap().unpack(entity).unwrap();
        }
    }
    /// Deletes all entities and their components.
    pub fn clear(&mut self) {
        // SAFE we have unique access
        let storages = unsafe { &mut *self.storages.get() };

        for storage in storages.values_mut() {
            // we have unique access to all storages so we can unwrap
            storage.clear().unwrap()
        }
    }
    /// Borrows the requested storages and runs `f`, this is an unnamed system.  
    /// You can use a tuple to get multiple storages at once.
    ///
    /// You can use:
    /// * `&T` for a shared access to `T` storage
    /// * `&mut T` for an exclusive access to `T` storage
    /// * [Entities] for a shared access to the entity storage
    /// * [EntitiesMut] for an exclusive reference to the entity storage
    /// * [AllStorages] for an exclusive access to the storage of all components
    /// * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]
    /// * [Unique]<&T> for a shared access to a `T` unique storage
    /// * [Unique]<&mut T> for an exclusive access to a `T` unique storage
    /// * [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
    /// * [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`
    /// * [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
    /// * [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`
    /// * [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    /// * [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`
    ///
    /// [Unique] and [NonSend]/[NonSync]/[NonSendSync] can be used together to access a unique storage missing `Send` and/or `Sync` bound(s).
    ///
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new();
    /// let all_storages = world.borrow::<AllStorages>();
    /// let u32s = all_storages.try_borrow::<&u32>().unwrap();
    /// ```
    /// [Entities]: struct.Entities.html
    /// [EntitiesMut]: struct.Entities.html
    /// [AllStorages]: struct.AllStorages.html
    /// [ThreadPool]: struct.ThreadPool.html
    /// [World]: struct.World.html
    /// [Unique]: struct.Unique.html
    /// [NonSend]: struct.NonSend.html
    /// [NonSync]: struct.NonSync.html
    /// [NonSendSync]: struct.NonSendSync.html
    pub fn try_borrow<'a, C: StorageBorrow<'a>>(
        &'a self,
    ) -> Result<<C as StorageBorrow<'a>>::View, error::GetStorage> {
        <C as StorageBorrow<'a>>::try_borrow(self)
    }
    /// Borrows the requested storages and runs `f`, this is an unnamed system.  
    /// You can use a tuple to get multiple storages at once.  
    /// Unwraps errors.
    ///
    /// You can use:
    /// * `&T` for a shared access to `T` storage
    /// * `&mut T` for an exclusive access to `T` storage
    /// * [Entities] for a shared access to the entity storage
    /// * [EntitiesMut] for an exclusive reference to the entity storage
    /// * [AllStorages] for an exclusive access to the storage of all components
    /// * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]
    /// * [Unique]<&T> for a shared access to a `T` unique storage
    /// * [Unique]<&mut T> for an exclusive access to a `T` unique storage
    /// * [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
    /// * [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`
    /// * [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
    /// * [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`
    /// * [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    /// * [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`
    ///
    /// [Unique] and [NonSend]/[NonSync]/[NonSendSync] can be used together to access a unique storage missing `Send` and/or `Sync` bound(s).
    ///
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new();
    /// let all_storages = world.borrow::<AllStorages>();
    /// let u32s = all_storages.borrow::<&u32>();
    /// ```
    /// [Entities]: struct.Entities.html
    /// [EntitiesMut]: struct.Entities.html
    /// [AllStorages]: struct.AllStorages.html
    /// [ThreadPool]: struct.ThreadPool.html
    /// [World]: struct.World.html
    /// [Unique]: struct.Unique.html
    /// [NonSend]: struct.NonSend.html
    /// [NonSync]: struct.NonSync.html
    /// [NonSendSync]: struct.NonSendSync.html
    pub fn borrow<'a, C: StorageBorrow<'a>>(&'a self) -> <C as StorageBorrow<'a>>::View {
        self.try_borrow::<C>().unwrap()
    }
}
