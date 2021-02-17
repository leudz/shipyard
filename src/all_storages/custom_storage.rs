use crate::all_storages::AllStorages;
use crate::atomic_refcell::{Ref, RefMut};
use crate::error;
use crate::storage::{Storage, StorageId};
use crate::unknown_storage::UnknownStorage;
use core::any::type_name;
use parking_lot::lock_api::RawRwLock;

pub trait CustomStorageAccess {
    fn custom_storage<T: 'static>(&self) -> Result<Ref<'_, &'_ T>, error::GetStorage>;
    fn custom_storage_by_id<T: 'static>(
        &self,
        storage_id: StorageId,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>;
    fn custom_storage_mut<T: 'static>(&self) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>;
    fn custom_storage_mut_by_id<T: 'static>(
        &self,
        storage_id: StorageId,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>;
    fn custom_storage_or_insert<T, F>(&self, f: F) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T;
    fn custom_storage_or_insert_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T;
    fn custom_storage_or_insert_mut<T, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T;
    fn custom_storage_or_insert_mut_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T;
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T;
}

impl CustomStorageAccess for AllStorages {
    fn custom_storage<T: 'static>(&self) -> Result<Ref<'_, &'_ T>, error::GetStorage> {
        self.custom_storage_by_id(StorageId::of::<T>())
    }
    fn custom_storage_by_id<T: 'static>(
        &self,
        storage_id: StorageId,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage(type_name::<T>().into()))
        }
    }
    fn custom_storage_mut<T: 'static>(&self) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage> {
        self.custom_storage_mut_by_id(StorageId::of::<T>())
    }
    fn custom_storage_mut_by_id<T: 'static>(
        &self,
        storage_id: StorageId,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage(type_name::<T>().into()))
        }
    }
    fn custom_storage_or_insert<T, F>(&self, f: F) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_by_id(StorageId::of::<T>(), f)
    }
    fn custom_storage_or_insert_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new(f()))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send(f(), self.thread_id))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_sync_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_sync(f()))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_sync_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send_sync(f(), self.thread_id))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    fn custom_storage_or_insert_mut<T, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_mut_by_id(StorageId::of::<T>(), f)
    }
    fn custom_storage_or_insert_mut_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new(f()))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_mut_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send(f(), self.thread_id))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_sync_mut_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_sync(f()))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_sync_mut_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send_sync(f(), self.thread_id))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
}
