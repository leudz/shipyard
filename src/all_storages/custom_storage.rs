use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ARef, ARefMut};
use crate::error;
use crate::storage::{SBox, Storage, StorageId};
use alloc::vec::Vec;
use core::any::type_name;

/// Low level access to storage.
///
/// Useful with custom storage or to define custom views.
pub trait CustomStorageAccess {
    /// Returns a [`ARef`] to the requested `S` storage.
    fn custom_storage<S: 'static>(&self) -> Result<ARef<'_, &'_ S>, error::GetStorage>;
    /// Returns a [`ARef`] to the requested `S` storage using a [`StorageId`].
    fn custom_storage_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<ARef<'_, &'_ dyn Storage>, error::GetStorage>;
    /// Returns a [`ARefMut`] to the requested `S` storage.
    fn custom_storage_mut<S: 'static>(&self) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`].
    fn custom_storage_mut_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<ARefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage>;
    /// Returns a [`ARef`] to the requested `S` storage and create it if it does not exist.
    fn custom_storage_or_insert<S, F>(&self, f: F) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    fn custom_storage_or_insert_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`ARef`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage and create it if it does not exist.
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    fn custom_storage_or_insert_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a list of all the storages present in [`AllStorages`].
    ///
    /// This can be used to list the names of an entity's components for example.
    ///
    /// ```rust
    /// use shipyard::{AllStoragesView, Component, all_storages::CustomStorageAccess, Storage, World};
    ///
    /// #[derive(Component)]
    /// struct MyComponent;
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity(MyComponent);
    ///
    /// let all_storages = world.borrow::<AllStoragesView>().unwrap();
    ///
    /// for storage in all_storages.iter_storages() {
    ///     let has_component = storage
    ///         .sparse_array()
    ///         .map(|sparse_array| sparse_array.contains(entity))
    ///         .unwrap_or(false);
    ///
    ///     let storage_name = storage.name();
    ///
    ///     if has_component {
    ///         println!("{entity:?} entity has a component in {storage_name}",);
    ///     }
    /// }
    /// ```
    ///
    /// Which prints: "EId(0.0) entity has a component in shipyard::sparse_set::SparseSet<test_project::MyComponent>".
    /// Then you can trim the storage name to only have the component name.
    fn iter_storages(&self) -> Vec<ARef<'_, &dyn Storage>>;
}

impl CustomStorageAccess for AllStorages {
    #[inline]
    fn custom_storage<S: 'static>(&self) -> Result<ARef<'_, &'_ S>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            drop(storages);
            match storage {
                Ok(storage) => Ok(ARef::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
            })
        }
    }
    fn custom_storage_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<ARef<'_, &'_ dyn Storage>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            drop(storages);
            storage.map_err(|err| error::GetStorage::StorageBorrow {
                name: None,
                id: storage_id,
                borrow: err,
            })
        } else {
            Err(error::GetStorage::MissingStorage {
                name: None,
                id: storage_id,
            })
        }
    }
    #[inline]
    fn custom_storage_mut<S: 'static>(&self) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.get(&StorageId::of::<S>());
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            drop(storages);
            match storage {
                Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
            })
        }
    }
    fn custom_storage_mut_by_id(
        &self,
        storage_id: StorageId,
    ) -> Result<ARefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            drop(storages);
            storage.map_err(|err| error::GetStorage::StorageBorrow {
                name: None,
                id: storage_id,
                borrow: err,
            })
        } else {
            Err(error::GetStorage::MissingStorage {
                name: None,
                id: storage_id,
            })
        }
    }
    #[inline]
    fn custom_storage_or_insert<S, F>(&self, f: F) -> Result<ARef<'_, &'_ S>, error::GetStorage>
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
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();
            drop(storages);
            match storage {
                Ok(storage) => Ok(ARef::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            drop(storages);
            let mut storages = self.storages.write();

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

            Ok(ARef::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
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
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();

            match storage {
                Ok(storage) => Ok(ARef::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if (self.thread_id_generator)() != self.main_thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            drop(storages);
            let mut storages = self.storages.write();

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_send(f(), self.thread_id_generator.clone()))
                    .0
            }
            .borrow()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            Ok(ARef::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
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
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();

            match storage {
                Ok(storage) => Ok(ARef::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            drop(storages);
            let mut storages = self.storages.write();

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

            Ok(ARef::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
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
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow();

            match storage {
                Ok(storage) => Ok(ARef::map(storage, |storage| {
                    storage.as_any().downcast_ref().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if (self.thread_id_generator)() != self.main_thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            drop(storages);
            let mut storages = self.storages.write();

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| {
                        SBox::new_non_send_sync(f(), self.thread_id_generator.clone())
                    })
                    .0
            }
            .borrow()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            Ok(ARef::map(storage?, |storage| {
                storage.as_any().downcast_ref::<S>().unwrap()
            }))
        }
    }
    #[inline]
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
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
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();
            drop(storages);
            match storage {
                Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            drop(storages);
            let mut storages = self.storages.write();

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

            Ok(ARefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_send_mut_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();

            match storage {
                Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if (self.thread_id_generator)() != self.main_thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            drop(storages);
            let mut storages = self.storages.write();

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| SBox::new_non_send(f(), self.thread_id_generator.clone()))
                    .0
            }
            .borrow_mut()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            Ok(ARefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_sync_mut_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();

            match storage {
                Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            drop(storages);
            let mut storages = self.storages.write();

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

            Ok(ARefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    #[cfg(feature = "thread_local")]
    #[inline]
    fn custom_storage_or_insert_non_send_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.custom_storage_or_insert_non_send_sync_mut_by_id(StorageId::of::<S>(), f)
    }
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        let storages = self.storages.read();
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = unsafe { &*storage.0 }.borrow_mut();

            match storage {
                Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                })),
                Err(err) => Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: err,
                }),
            }
        } else {
            if (self.thread_id_generator)() != self.main_thread_id {
                return Err(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<S>()),
                    id: StorageId::of::<S>(),
                    borrow: error::Borrow::WrongThread,
                });
            }

            drop(storages);
            let mut storages = self.storages.write();

            let storage = unsafe {
                &*storages
                    .entry(storage_id)
                    .or_insert_with(|| {
                        SBox::new_non_send_sync(f(), self.thread_id_generator.clone())
                    })
                    .0
            }
            .borrow_mut()
            .map_err(|err| error::GetStorage::StorageBorrow {
                name: Some(type_name::<S>()),
                id: StorageId::of::<S>(),
                borrow: err,
            });

            Ok(ARefMut::map(storage?, |storage| {
                storage.as_any_mut().downcast_mut::<S>().unwrap()
            }))
        }
    }
    fn iter_storages(&self) -> Vec<ARef<'_, &dyn Storage>> {
        self.storages
            .read()
            .iter()
            .flat_map(|(storage_id, storage)| unsafe {
                (*storage.0)
                    .borrow()
                    .map_err(|err| error::GetStorage::StorageBorrow {
                        name: None,
                        id: *storage_id,
                        borrow: err,
                    })
            })
            .collect()
    }
}
