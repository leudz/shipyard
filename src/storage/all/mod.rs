mod delete_any;

pub use delete_any::DeleteAny;

use super::{Entities, EntityId, Storage, StorageId, Unique};
use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::borrow::AllStoragesBorrow;
use crate::entity_builder::EntityBuilder;
use crate::error;
use crate::sparse_set::SparseSet;
use crate::type_id::TypeId;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use hashbrown::{hash_map::Entry, HashMap};
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
    storages: UnsafeCell<HashMap<StorageId, Storage>>,
    #[cfg(feature = "non_send")]
    thread_id: std::thread::ThreadId,
}

#[cfg(not(feature = "non_send"))]
unsafe impl Send for AllStorages {}

unsafe impl Sync for AllStorages {}

impl AllStorages {
    pub(crate) fn new() -> Self {
        let mut storages = HashMap::default();

        let entities = Entities::new();

        storages.insert(
            TypeId::of::<Entities>().into(),
            Storage(Box::new(AtomicRefCell::new(entities))),
        );

        AllStorages {
            storages: UnsafeCell::new(storages),
            lock: RawRwLock::INIT,
            #[cfg(feature = "non_send")]
            thread_id: std::thread::current().id(),
        }
    }
    pub(crate) fn entities(&self) -> Result<Ref<'_, Entities>, error::Borrow> {
        let type_id = TypeId::of::<Entities>().into();
        self.lock.lock_shared();
        // SAFE we locked
        let storages = unsafe { &*self.storages.get() };
        // AllStorages is always created with Entities so there's no way to not find it
        let storage = &storages[&type_id];
        match storage.entities() {
            Ok(entities) => {
                unsafe { self.lock.unlock_shared() };
                Ok(entities)
            }
            Err(err) => {
                unsafe { self.lock.unlock_shared() };
                Err(err)
            }
        }
    }
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, Entities>, error::Borrow> {
        let type_id = TypeId::of::<Entities>().into();
        self.lock.lock_shared();
        // SAFE we locked
        let storages = unsafe { &*self.storages.get() };
        // AllStorages is always created with Entities so there's no way to not find it
        let storage = &storages[&type_id];
        match storage.entities_mut() {
            Ok(entities) => {
                unsafe { self.lock.unlock_shared() };
                Ok(entities)
            }
            Err(err) => {
                unsafe { self.lock.unlock_shared() };
                Err(err)
            }
        }
    }
    pub(crate) fn sparse_set<T: 'static + Send + Sync>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::GetStorage> {
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new::<T>)
            .sparse_set::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    pub(crate) fn sparse_set_mut<T: 'static + Send + Sync>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::GetStorage> {
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new::<T>)
            .sparse_set_mut::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn sparse_set_non_send<T: 'static + Sync>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::GetStorage> {
        // Sync components can be accessed by any thread with a shared access
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_non_send::<T>(self.thread_id))
            .sparse_set::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn sparse_set_non_send_mut<T: 'static + Sync>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::GetStorage> {
        // Sync components can only be accessed by the thread they were created in with a unique access
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_non_send::<T>(self.thread_id))
            .sparse_set_mut::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn sparse_set_non_sync<T: 'static + Send>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::GetStorage> {
        // Send components can be accessed by one thread at a time
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_sync::<T>)
            .sparse_set::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn sparse_set_non_sync_mut<T: 'static + Send>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::GetStorage> {
        // Send components can be accessed by one thread at a time
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(Storage::new_non_sync::<T>)
            .sparse_set_mut::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn sparse_set_non_send_sync<T: 'static>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::GetStorage> {
        // !Send + !Sync components can only be accessed by the thread they were created in
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_non_send_sync::<T>(self.thread_id))
            .sparse_set::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn sparse_set_non_send_sync_mut<T: 'static>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::GetStorage> {
        // !Send + !Sync components can only be accessed by the thread they were created in
        let type_id = TypeId::of::<T>().into();
        {
            self.lock.lock_shared();
            // SAFE we locked
            let storages = unsafe { &*self.storages.get() };
            if let Some(storage) = storages.get(&type_id) {
                let sparse_set = storage.sparse_set_mut::<T>();
                unsafe { self.lock.unlock_shared() };
                return sparse_set;
            }
        }
        unsafe { self.lock.unlock_shared() };
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        let sparse_set = storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_non_send_sync::<T>(self.thread_id))
            .sparse_set_mut::<T>();
        unsafe { self.lock.unlock_exclusive() };
        sparse_set
    }
    pub(crate) fn unique<T: 'static>(&self) -> Result<Ref<'_, T>, error::GetStorage> {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_shared();
        // SAFE we locked
        let storages = unsafe { &*self.storages.get() };
        if let Some(storage) = storages.get(&type_id) {
            let unique = storage.unique::<T>();
            unsafe { self.lock.unlock_shared() };
            unique
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingUnique(core::any::type_name::<T>()))
        }
    }
    pub(crate) fn unique_mut<T: 'static>(&self) -> Result<RefMut<'_, T>, error::GetStorage> {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_shared();
        // SAFE we locked
        let storages = unsafe { &*self.storages.get() };
        if let Some(storage) = storages.get(&type_id) {
            let unique = storage.unique_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            unique
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingUnique(core::any::type_name::<T>()))
        }
    }
    /// Removes a unique storage.  
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `T` storage borrow failed.
    /// - `T` storage did not exist.
    pub fn try_remove_unique<T: 'static>(&self) -> Result<T, error::UniqueRemove> {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        if let Entry::Occupied(entry) = storages.entry(type_id) {
            // `.err()` to avoid borrowing `entry` in the `Ok` case
            if let Some(get_storage) = entry.get().unique_mut::<T>().err() {
                unsafe { self.lock.unlock_exclusive() };
                match get_storage {
                    error::GetStorage::StorageBorrow(infos) => {
                        Err(error::UniqueRemove::StorageBorrow(infos))
                    }
                    _ => unreachable!(),
                }
            } else {
                // We were able to lock the storage, we've still got exclusive access even though
                // we released that lock as we're still holding the `AllStorages` lock.
                let storage = entry.remove();
                unsafe { self.lock.unlock_exclusive() };
                // SAFE T is a unique storage
                unsafe { Ok(AtomicRefCell::into_unique::<T>(storage.0)) }
            }
        } else {
            unsafe { self.lock.unlock_exclusive() };
            Err(error::UniqueRemove::MissingUnique(
                core::any::type_name::<T>(),
            ))
        }
    }
    /// Removes a unique storage.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `T` storage borrow failed.
    /// - `T` storage did not exist.
    #[track_caller]
    pub fn remove_unique<T: 'static>(&self) -> T {
        match self.try_remove_unique::<T>() {
            Ok(unique) => unique,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [UniqueView] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    ///
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_unique::<T>(component));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSend] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSend]: struct.NonSend.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "non_send")]
    pub fn add_unique_non_send<T: 'static + Sync>(&self, component: T) {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_unique_non_send::<T>(component, self.thread_id));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSync]: struct.NonSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "non_sync")]
    pub fn add_unique_non_sync<T: 'static + Send>(&self, component: T) {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_unique_non_sync::<T>(component));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    ///
    /// [NonSync]: struct.NonSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub fn add_unique_non_send_sync<T: 'static>(&self, component: T) {
        let type_id = TypeId::of::<Unique<T>>().into();
        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        // another thread might have initialized the storage before this thread so we use entry
        storages
            .entry(type_id)
            .or_insert_with(|| Storage::new_unique_non_send_sync::<T>(component, self.thread_id));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Delete an entity and all its components.
    /// Returns `true` if `entity` was alive.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{AllStoragesViewMut, EntitiesViewMut, Get, View, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// let [entity1, entity2] = world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         [
    ///             entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
    ///             entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
    ///         ]
    ///     },
    /// );
    ///
    /// world.run(|mut all_storages: AllStoragesViewMut| {
    ///     all_storages.delete(entity1);
    /// });
    ///
    /// world.run(|usizes: View<usize>, u32s: View<u32>| {
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
            storages
                .get_mut(&StorageId::TypeId(storage))
                .unwrap()
                .unpack(entity)
                .unwrap();
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
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{AllStoragesViewMut, EntitiesView, View, ViewMut, World};

let world = World::new();

world.run(|all_storages: AllStoragesViewMut| {
    let u32s = all_storages.try_borrow::<View<u32>>().unwrap();
    let (entities, mut usizes) = all_storages
        .try_borrow::<(EntitiesView, ViewMut<usize>)>()
        .unwrap();
});
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_borrow<'s, V: AllStoragesBorrow<'s>>(&'s self) -> Result<V, error::GetStorage> {
        V::try_borrow(self)
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{AllStoragesViewMut, EntitiesView, View, ViewMut, World};

let world = World::new();

world.run(|all_storages: AllStoragesViewMut| {
    let u32s = all_storages.try_borrow::<View<u32>>().unwrap();
    let (entities, mut usizes) = all_storages.borrow::<(EntitiesView, ViewMut<usize>)>();
});
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn borrow<'s, V: AllStoragesBorrow<'s>>(&'s self) -> V {
        match self.try_borrow::<V>() {
            Ok(views) => views,
            Err(err) => panic!("{:?}", err),
        }
    }
    #[doc = "Borrows the requested storages and runs the function.  
Data can be passed to the function, this always has to be a single type but you can use a tuple if needed.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[World]: struct.World.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_run_with_data<'s, Data, B, R, S: crate::system::AllSystem<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> Result<R, error::Run> {
        Ok(s.run((data,), S::try_borrow(self)?))
    }
    #[doc = "Borrows the requested storages and runs the function.  
Data can be passed to the function, this always has to be a single type but you can use a tuple if needed.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[World]: struct.World.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn run_with_data<'s, Data, B, R, S: crate::system::AllSystem<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> R {
        match self.try_run_with_data(s, data) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    #[doc = "Borrows the requested storages and runs the function.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{AllStoragesViewMut, View, ViewMut, World};

fn sys1(i32s: View<i32>) -> i32 {
    0
}

let world = World::new();

let all_storages = world.borrow::<AllStoragesViewMut>();

all_storages
    .try_run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
        // -- snip --
    })
    .unwrap();

let i = all_storages.try_run(sys1).unwrap();
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_run<'s, B, R, S: crate::system::AllSystem<'s, (), B, R>>(
        &'s self,
        s: S,
    ) -> Result<R, error::Run> {
        Ok(s.run((), S::try_borrow(self)?))
    }
    #[doc = "Borrows the requested storages and runs the function.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{AllStoragesViewMut, View, ViewMut, World};

fn sys1(i32s: View<i32>) -> i32 {
    0
}

let world = World::new();

let all_storages = world.borrow::<AllStoragesViewMut>();

all_storages.run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
    // -- snip --
});

let i = all_storages.run(sys1);
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn run<'s, B, R, S: crate::system::AllSystem<'s, (), B, R>>(&'s self, s: S) -> R {
        match self.try_run(s) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Deletes any entity with at least one of the given type(s).
    ///
    /// `T` has to be a tuple even for a single type.  
    /// In this case use (T,).
    pub fn delete_any<T: DeleteAny>(&mut self) {
        T::delete_any(self)
    }
    /// Used to create an entity without having to borrow its storage explicitly.  
    /// The entity is only added when [EntityBuilder::try_build] or [EntityBuilder::build] is called.  
    ///
    /// [EntityBuilder::try_build]: struct.EntityBuilder.html#method.try_build
    /// [EntityBuilder::build]: struct.EntityBuilder.html#method.build
    /// [AllStorages]: struct.AllStorages.html
    pub fn entity_builder(&self) -> EntityBuilder<'_, (), ()> {
        EntityBuilder::new_from_reference(self)
    }
    // #[cfg(feature = "serde1")]
    // pub(crate) fn storages(&mut self) -> &mut HashMap<StorageId, Storage> {
    //     // SAFE we have exclusive access
    //     unsafe { &mut *self.storages.get() }
    // }
}

// #[cfg(feature = "serde1")]
// pub(crate) struct AllStoragesSerializer<'a> {
//     pub(crate) all_storages: RefMut<'a, AllStorages>,
//     pub(crate) ser_config: crate::serde_setup::GlobalSerConfig,
// }

// #[cfg(feature = "serde1")]
// impl serde::Serialize for AllStoragesSerializer<'_> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         use serde::ser::SerializeStruct;

//         let all_storages = unsafe { &*self.all_storages.storages.get() };

//         let mut storages = Vec::with_capacity(all_storages.len());

//         let ser_infos = crate::serde_setup::SerInfos {
//             same_binary: self.ser_config.same_binary,
//             with_entities: self.ser_config.with_entities,
//         };

//         let mut state = serializer.serialize_struct("AllStorages", 3)?;

//         if ser_infos.same_binary {
//             let metadata = all_storages
//                 .iter()
//                 .filter_map(|(type_id, storage)| {
//                     let storage = storage.0.try_borrow().unwrap();

//                     if storage.should_serialize(self.ser_config) {
//                         let result = match storage.deserialize() {
//                             Some(deserialize) => Ok((
//                                 type_id,
//                                 crate::unknown_storage::deserialize_ptr(deserialize),
//                             )),
//                             None => Err(serde::ser::Error::custom(
//                                 "Unknown storage's implementation is incorrect.",
//                             )),
//                         };

//                         storages.push(storage);

//                         Some(result)
//                     } else {
//                         None
//                     }
//                 })
//                 .collect::<Result<Vec<_>, S::Error>>()?;

//             state.serialize_field("ser_infos", &ser_infos)?;
//             state.serialize_field("metadata", &metadata)?;
//             state.serialize_field(
//                 "storages",
//                 &storages
//                     .iter()
//                     .map(|storage| crate::unknown_storage::StorageSerializer {
//                         unknown_storage: &**storage,
//                         ser_config: self.ser_config,
//                     })
//                     .collect::<Vec<_>>(),
//             )?;
//         } else {
//             todo!()
//         }

//         state.end()
//     }
// }
