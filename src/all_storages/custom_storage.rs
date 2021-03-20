use crate::all_storages::AllStorages;
use crate::atomic_refcell::{Ref, RefMut};
use crate::error;
use crate::storage::{SBox, Storage, StorageId};
use core::any::type_name;
use parking_lot::lock_api::RawRwLock;

/// Low level access to storage.
///
/// Useful with custom storage or to define custom views.
pub trait CustomStorageAccess {
    /// Returns a [`Ref`] to the requested `S` storage.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    fn custom_storage<S: 'static>(&self) -> Result<Ref<'_, &'_ S>, error::GetStorage>;
    /// Returns a [`Ref`] to the requested `S` storage using a [`StorageId`].
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    /// [`StorageId`]: crate::storage::StorageId
    fn custom_storage_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<Ref<'_, &'_ dyn Storage>, error::GetStorage>;
    /// Returns a [`RefMut`] to the requested `S` storage.
    fn custom_storage_mut<S: 'static>(&self) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>;
    /// Returns a [`RefMut`] to the requested `S` storage using a [`StorageId`].
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    /// [`StorageId`]: crate::storage::StorageId
    fn custom_storage_mut_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<RefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage>;
    /// Returns a [`Ref`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    fn custom_storage_or_insert<S, F>(&self, f: F) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    /// [`StorageId`]: crate::storage::StorageId
    fn custom_storage_or_insert_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    /// [`StorageId`]: crate::storage::StorageId
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    /// [`StorageId`]: crate::storage::StorageId
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`Ref`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`Ref`]: crate::atomic_refcell::Ref
    /// [`StorageId`]: crate::storage::StorageId
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    /// [`StorageId`]: crate::storage::StorageId
    fn custom_storage_or_insert_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    /// [`StorageId`]: crate::storage::StorageId
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut_by_id<'a, S, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    /// [`StorageId`]: crate::storage::StorageId
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut_by_id<'a, S, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`RefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    ///
    /// [`RefMut`]: crate::atomic_refcell::RefMut
    /// [`StorageId`]: crate::storage::StorageId
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<'a, S, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
}

impl CustomStorageAccess for AllStorages {
    #[inline]
    fn custom_storage<S: 'static>(&self) -> Result<Ref<'_, &'_ S>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(Ref::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
            })
        }
    }
    fn custom_storage_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<Ref<'_, &'_ dyn Storage>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow {
                name: None,
                id: storage_id,
                borrow: err,
            })
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage {
                name: None,
                id: storage_id,
            })
        }
    }
    #[inline]
    fn custom_storage_mut<S: 'static>(&self) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(RefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
            })
        }
    }
    fn custom_storage_mut_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<RefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow {
                name: None,
                id: storage_id,
                borrow: err,
            })
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage {
                name: None,
                id: storage_id,
            })
        }
    }
    #[inline]
    fn custom_storage_or_insert<S, F>(&self, f: F) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_by_id(StorageId::of::<S>(), f)
    }
    fn custom_storage_or_insert_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(Ref::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new(f()))
                    .0
            }
            .borrow()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(Ref::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_send_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(Ref::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if std::thread::current().id() != self.thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_send(f(), self.thread_id))
                    .0
            }
            .borrow()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(Ref::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_sync_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(Ref::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_sync(f()))
                    .0
            }
            .borrow()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(Ref::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_send_sync_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(Ref::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if std::thread::current().id() != self.thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_send_sync(f(), self.thread_id))
                    .0
            }
            .borrow()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(Ref::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[inline]
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_mut_by_id(StorageId::of::<S>(), f)
    }
    fn custom_storage_or_insert_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(RefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new(f()))
                    .0
            }
            .borrow_mut()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(RefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_send_mut_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut_by_id<'a, S, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(RefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if std::thread::current().id() != self.thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_send(f(), self.thread_id))
                    .0
            }
            .borrow_mut()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(RefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_sync_mut_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut_by_id<'a, S, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(RefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_sync(f()))
                    .0
            }
            .borrow_mut()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(RefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send_sync_mut<'a, S, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_send_sync_mut_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<'a, S, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            unsafe { self.lock.unlock_shared() };
            match storage {
                Ok(storage) => Ok(RefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if std::thread::current().id() != self.thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_send_sync(f(), self.thread_id))
                    .0
            }
            .borrow_mut()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            unsafe { self.lock.unlock_exclusive() };

            Ok(RefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
}
