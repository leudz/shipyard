use crate::all_storages::AllStorages;
use crate::atomic_refcell::{Ref, RefMut};
use crate::error;
use crate::storage::{Storage, StorageId};
use crate::unknown_storage::UnknownStorage;
use core::any::type_name;
use parking_lot::lock_api::RawRwLock;

/// Low level access to storage.
///
/// Useful with custom storage or to define custom views.
pub trait CustomStorageAccess {
    /// Returns a `Ref` to the requested `S` storage.
    fn custom_storage<S: 'static>(&self) -> Result<Ref<'_, &'_ S>, error::GetStorage>;
    /// Returns a `RefMut` to the requested `S` storage.
    fn custom_storage_mut<S: 'static>(&self) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>;
    /// Returns a `Ref` to the requested `S` storage and create it if it does not exist.
    fn custom_storage_or_insert<S, F>(&self, f: F) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a `Ref` to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Sync,
        F: FnOnce() -> S;
    /// Returns a `Ref` to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send,
        F: FnOnce() -> S;
    /// Returns a `Ref` to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage,
        F: FnOnce() -> S;
    /// Returns a `RefMut` to the requested `S` storage and create it if it does not exist.
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a `RefMut` to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Sync,
        F: FnOnce() -> S;
    /// Returns a `RefMut` to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send,
        F: FnOnce() -> S;
    /// Returns a `RefMut` to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage,
        F: FnOnce() -> S;
}

impl CustomStorageAccess for AllStorages {
    fn custom_storage<S: 'static>(&self) -> Result<Ref<'_, &'_ S>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage(type_name::<S>()))
        }
    }
    fn custom_storage_mut<S: 'static>(&self) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get_mut::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage(type_name::<S>()))
        }
    }
    fn custom_storage_or_insert<S, F>(&self, f: F) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new(f()))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new_non_send(f(), self.thread_id))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new_non_sync(f()))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + UnknownStorage,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new_non_send_sync(f(), self.thread_id))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get_mut::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new(f()))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get_mut::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new_non_send(f(), self.thread_id))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage + Send,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get_mut::<S>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new_non_sync(f()))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + UnknownStorage,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = storage.get_mut::<S>();
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(StorageId::of::<S>())
                .or_insert_with(|| Storage::new_non_send_sync(f(), self.thread_id))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<S>(), err)))
        }
    }
}
