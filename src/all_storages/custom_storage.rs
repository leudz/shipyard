use crate::all_storages::{AllStorages, StorageMaps};
use crate::atomic_refcell::{ARef, ARefMut, AtomicRefCell};
use crate::entities::Entities;
use crate::error;
use crate::public_transport::RwLock;
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
        storage_id: &StorageId,
    ) -> Result<ARef<'_, &'_ dyn Storage>, error::GetStorage>;
    /// Returns a [`ARefMut`] to the requested `S` storage.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    fn custom_storage_mut<S: 'static>(&self) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`].
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    fn custom_storage_mut_by_id(
        &self,
        storage_id: &StorageId,
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
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    fn custom_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    fn custom_storage_or_insert_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage and create it if it does not exist.
    #[allow(
        clippy::mut_from_ref,
        reason = "ARefMut retains a runtime-checked exclusive borrow"
    )]
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[allow(
        clippy::mut_from_ref,
        reason = "ARefMut retains a runtime-checked exclusive borrow"
    )]
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
    #[allow(
        clippy::mut_from_ref,
        reason = "ARefMut retains a runtime-checked exclusive borrow"
    )]
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[allow(
        clippy::mut_from_ref,
        reason = "ARefMut retains a runtime-checked exclusive borrow"
    )]
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
    #[allow(
        clippy::mut_from_ref,
        reason = "ARefMut retains a runtime-checked exclusive borrow"
    )]
    #[cfg(feature = "thread_local")]
    fn custom_storage_or_insert_non_send_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S;
    /// Returns a [`ARefMut`] to the requested `S` storage using a [`StorageId`] and create it if it does not exist.
    #[allow(
        clippy::mut_from_ref,
        reason = "ARefMut retains a runtime-checked exclusive borrow"
    )]
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
    /// use shipyard::{AllStoragesView, Component, all_storages::CustomStorageAccess, advanced::Storage, World};
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
    /// Returns a mutable list of all the storages present in [`AllStorages`].
    #[allow(clippy::mut_from_ref)]
    fn iter_storages_mut(&self) -> Vec<ARefMut<'_, &mut (dyn Storage + 'static)>>;
}

// Every access below borrows the storage it found *before* releasing the registry lock, see
// `AllStorages` for why that keeps the storage alive. So each `unsafe` block only has to show that
// the `SBox` comes from the registry and that the guard it comes from is still alive.

impl CustomStorageAccess for AllStorages {
    #[inline]
    fn custom_storage<S: 'static>(&self) -> Result<ARef<'_, &'_ S>, error::GetStorage> {
        let storage_id = StorageId::of::<S>();

        // The entities aren't in the registry.
        let storage = if storage_id == StorageId::of::<Entities>() {
            let entities: &AtomicRefCell<dyn Storage> = &self.entities;
            Some(entities.borrow())
        } else {
            let storages = self.storages.read();
            let storage = storages.any(&storage_id).map(|sbox| {
                // SAFE: `sbox` is in the registry and `storages` is still locked.
                unsafe { &*sbox.0 }.borrow()
            });
            drop(storages);

            storage
        };

        match storage {
            Some(Ok(storage)) => Ok(ARef::map(storage, |storage| {
                storage.as_any().downcast_ref().unwrap()
            })),
            Some(Err(err)) => Err(error::GetStorage::storage_borrow::<S>(err)),
            None => Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: storage_id,
            }),
        }
    }
    #[inline]
    fn custom_storage_by_id(
        &self,
        storage_id: &StorageId,
    ) -> Result<ARef<'_, &'_ dyn Storage>, error::GetStorage> {
        // The entities aren't in the registry.
        let storage = if storage_id == &StorageId::of::<Entities>() {
            let entities: &AtomicRefCell<dyn Storage> = &self.entities;
            Some(entities.borrow())
        } else {
            let storages = self.storages.read();
            let storage = storages.any(storage_id).map(|sbox| {
                // SAFE: `sbox` is in the registry and `storages` is still locked.
                unsafe { &*sbox.0 }.borrow()
            });
            drop(storages);

            storage
        };

        match storage {
            Some(Ok(storage)) => Ok(storage),
            Some(Err(err)) => Err(error::GetStorage::StorageBorrow {
                name: None,
                id: *storage_id,
                borrow: err,
            }),
            None => Err(error::GetStorage::MissingStorage {
                name: None,
                id: *storage_id,
            }),
        }
    }
    #[inline]
    fn custom_storage_mut<S: 'static>(&self) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage> {
        let storage_id = StorageId::of::<S>();

        // The entities aren't in the registry.
        let storage = if storage_id == StorageId::of::<Entities>() {
            let entities: &AtomicRefCell<dyn Storage> = &self.entities;
            Some(entities.borrow_mut())
        } else {
            let storages = self.storages.read();
            let storage = storages.any(&storage_id).map(|sbox| {
                // SAFE: `sbox` is in the registry and `storages` is still locked.
                unsafe { &*sbox.0 }.borrow_mut()
            });
            drop(storages);

            storage
        };

        match storage {
            Some(Ok(storage)) => Ok(ARefMut::map(storage, |storage| {
                storage.as_any_mut().downcast_mut().unwrap()
            })),
            Some(Err(err)) => Err(error::GetStorage::storage_borrow::<S>(err)),
            None => Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: storage_id,
            }),
        }
    }
    #[inline]
    fn custom_storage_mut_by_id(
        &self,
        storage_id: &StorageId,
    ) -> Result<ARefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage> {
        // The entities aren't in the registry.
        let storage = if storage_id == &StorageId::of::<Entities>() {
            let entities: &AtomicRefCell<dyn Storage> = &self.entities;
            Some(entities.borrow_mut())
        } else {
            let storages = self.storages.read();
            let storage = storages.any(storage_id).map(|sbox| {
                // SAFE: `sbox` is in the registry and `storages` is still locked.
                unsafe { &*sbox.0 }.borrow_mut()
            });
            drop(storages);

            storage
        };

        match storage {
            Some(Ok(storage)) => Ok(storage),
            Some(Err(err)) => Err(error::GetStorage::StorageBorrow {
                name: None,
                id: *storage_id,
                borrow: err,
            }),
            None => Err(error::GetStorage::MissingStorage {
                name: None,
                id: *storage_id,
            }),
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
    #[inline]
    fn custom_storage_or_insert_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.generic_or_insert(storage_id, false, || SBox::new(f()))
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
    #[inline]
    fn custom_storage_or_insert_non_send_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.generic_or_insert(storage_id, true, || {
            SBox::new_non_send(f(), self.thread_id_generator.clone())
        })
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
    #[inline]
    fn custom_storage_or_insert_non_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.generic_or_insert(storage_id, false, || SBox::new_non_sync(f()))
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
    #[inline]
    fn custom_storage_or_insert_non_send_sync_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.generic_or_insert(storage_id, true, || {
            SBox::new_non_send_sync(f(), self.thread_id_generator.clone())
        })
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
    #[inline]
    fn custom_storage_or_insert_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.generic_or_insert_mut(storage_id, false, || SBox::new(f()))
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
    #[inline]
    fn custom_storage_or_insert_non_send_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.generic_or_insert_mut(storage_id, true, || {
            SBox::new_non_send(f(), self.thread_id_generator.clone())
        })
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
    #[inline]
    fn custom_storage_or_insert_non_sync_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.generic_or_insert_mut(storage_id, false, || SBox::new_non_sync(f()))
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
    #[inline]
    fn custom_storage_or_insert_non_send_sync_mut_by_id<S, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.generic_or_insert_mut(storage_id, true, || {
            SBox::new_non_send_sync(f(), self.thread_id_generator.clone())
        })
    }
    fn iter_storages(&self) -> Vec<ARef<'_, &dyn Storage>> {
        // Storages we can't borrow are skipped. The entities aren't in the registry, they come first.
        let entities: &AtomicRefCell<dyn Storage> = &self.entities;
        let mut storages: Vec<_> = entities.borrow().into_iter().collect();

        let registry = self.storages.read();
        storages.extend(registry.values().flat_map(|sbox| {
            // SAFE: `sbox` is in the registry and `registry` is still locked.
            unsafe { &*sbox.0 }.borrow()
        }));
        drop(registry);

        storages
    }

    fn iter_storages_mut(&self) -> Vec<ARefMut<'_, &mut (dyn Storage + 'static)>> {
        // Storages we can't borrow are skipped. The entities aren't in the registry, they come first.
        let entities: &AtomicRefCell<dyn Storage> = &self.entities;
        let mut storages: Vec<_> = entities.borrow_mut().into_iter().collect();

        let registry = self.storages.read();
        storages.extend(registry.values().flat_map(|sbox| {
            // SAFE: `sbox` is in the registry and `registry` is still locked.
            unsafe { &*sbox.0 }.borrow_mut()
        }));
        drop(registry);

        storages
    }
}

/// Specialized access, for built-in views and operations.
///
/// Built-in code asks for a component or a unique storage by name, so it never depends on the
/// generic API's lookup order. Component access only searches the components and custom storages,
/// unique access searches the uniques first. Only the generic API finds the entities.
impl AllStorages {
    /// Returns a [`ARef`] to the `S` component or custom storage, inserting `f()` if it doesn't exist.
    #[inline]
    pub(crate) fn component_storage_or_insert<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.component_or_insert(StorageId::of::<S>(), false, || SBox::new(f()))
    }

    /// Returns a [`ARef`] to the `!Send` `S` component storage, inserting `f()` if it doesn't exist.
    ///
    /// Only the world's thread can insert it.
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn component_storage_or_insert_non_send<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.component_or_insert(StorageId::of::<S>(), true, || {
            SBox::new_non_send(f(), self.thread_id_generator.clone())
        })
    }

    /// Returns a [`ARef`] to the `!Sync` `S` component storage, inserting `f()` if it doesn't exist.
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn component_storage_or_insert_non_sync<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.component_or_insert(StorageId::of::<S>(), false, || SBox::new_non_sync(f()))
    }

    /// Returns a [`ARef`] to the `!Send + !Sync` `S` component storage, inserting `f()` if it doesn't exist.
    ///
    /// Only the world's thread can insert it.
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn component_storage_or_insert_non_send_sync<S, F>(
        &self,
        f: F,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.component_or_insert(StorageId::of::<S>(), true, || {
            SBox::new_non_send_sync(f(), self.thread_id_generator.clone())
        })
    }

    /// Returns a [`ARefMut`] to the `S` component or custom storage, inserting `f()` if it doesn't exist.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[inline]
    pub(crate) fn component_storage_or_insert_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send + Sync,
        F: FnOnce() -> S,
    {
        self.component_or_insert_mut(StorageId::of::<S>(), false, || SBox::new(f()))
    }

    /// Returns a [`ARefMut`] to the `!Send` `S` component storage, inserting `f()` if it doesn't exist.
    ///
    /// Only the world's thread can insert it.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn component_storage_or_insert_non_send_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Sync,
        F: FnOnce() -> S,
    {
        self.component_or_insert_mut(StorageId::of::<S>(), true, || {
            SBox::new_non_send(f(), self.thread_id_generator.clone())
        })
    }

    /// Returns a [`ARefMut`] to the `!Sync` `S` component storage, inserting `f()` if it doesn't exist.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn component_storage_or_insert_non_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage + Send,
        F: FnOnce() -> S,
    {
        self.component_or_insert_mut(StorageId::of::<S>(), false, || SBox::new_non_sync(f()))
    }

    /// Returns a [`ARefMut`] to the `!Send + !Sync` `S` component storage, inserting `f()` if it doesn't exist.
    ///
    /// Only the world's thread can insert it.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn component_storage_or_insert_non_send_sync_mut<S, F>(
        &self,
        f: F,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
    where
        S: 'static + Storage,
        F: FnOnce() -> S,
    {
        self.component_or_insert_mut(StorageId::of::<S>(), true, || {
            SBox::new_non_send_sync(f(), self.thread_id_generator.clone())
        })
    }

    /// Returns a [`ARefMut`] to the component or custom storage registered under `storage_id`.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    pub(crate) fn component_storage_mut_by_id(
        &self,
        storage_id: &StorageId,
    ) -> Result<ARefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.component(storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow_mut()
        });
        drop(storages);

        match storage {
            Some(Ok(storage)) => Ok(storage),
            Some(Err(err)) => Err(error::GetStorage::StorageBorrow {
                name: None,
                id: *storage_id,
                borrow: err,
            }),
            None => Err(error::GetStorage::MissingStorage {
                name: None,
                id: *storage_id,
            }),
        }
    }

    /// Returns a [`ARef`] to the `S` unique storage.
    #[inline]
    pub(crate) fn unique_storage<S: 'static>(&self) -> Result<ARef<'_, &'_ S>, error::GetStorage> {
        let storage_id = StorageId::of::<S>();

        let storages = self.storages.read();
        let storage = storages.unique(&storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow()
        });
        drop(storages);

        match storage {
            Some(Ok(storage)) => Ok(ARef::map(storage, |storage| {
                storage.as_any().downcast_ref().unwrap()
            })),
            Some(Err(err)) => Err(error::GetStorage::storage_borrow::<S>(err)),
            None => Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: storage_id,
            }),
        }
    }

    /// Returns a [`ARef`] to the unique storage registered under `storage_id`.
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn unique_storage_by_id(
        &self,
        storage_id: &StorageId,
    ) -> Result<ARef<'_, &'_ dyn Storage>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.unique(storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow()
        });
        drop(storages);

        match storage {
            Some(Ok(storage)) => Ok(storage),
            Some(Err(err)) => Err(error::GetStorage::StorageBorrow {
                name: None,
                id: *storage_id,
                borrow: err,
            }),
            None => Err(error::GetStorage::MissingStorage {
                name: None,
                id: *storage_id,
            }),
        }
    }

    /// Returns a [`ARefMut`] to the `S` unique storage.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[inline]
    pub(crate) fn unique_storage_mut<S: 'static>(
        &self,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage> {
        let storage_id = StorageId::of::<S>();

        let storages = self.storages.read();
        let storage = storages.unique(&storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow_mut()
        });
        drop(storages);

        match storage {
            Some(Ok(storage)) => Ok(ARefMut::map(storage, |storage| {
                storage.as_any_mut().downcast_mut().unwrap()
            })),
            Some(Err(err)) => Err(error::GetStorage::storage_borrow::<S>(err)),
            None => Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<S>()),
                id: storage_id,
            }),
        }
    }

    /// Returns a [`ARefMut`] to the unique storage registered under `storage_id`.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn unique_storage_mut_by_id(
        &self,
        storage_id: &StorageId,
    ) -> Result<ARefMut<'_, &'_ mut (dyn Storage + 'static)>, error::GetStorage> {
        let storages = self.storages.read();
        let storage = storages.unique(storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow_mut()
        });
        drop(storages);

        match storage {
            Some(Ok(storage)) => Ok(storage),
            Some(Err(err)) => Err(error::GetStorage::StorageBorrow {
                name: None,
                id: *storage_id,
                borrow: err,
            }),
            None => Err(error::GetStorage::MissingStorage {
                name: None,
                id: *storage_id,
            }),
        }
    }
}

/// Insertion, shared by the variants of the generic and component access.
//
// Each one looks under the read lock first. On a miss, it takes the write lock and looks again:
// someone could have inserted the storage in between. New storages always go with the components
// and custom storages, only `add_unique*` adds to the uniques.
impl AllStorages {
    /// Borrows the `S` storage registered under `storage_id`, or inserts `new_storage()` there
    /// and borrows it.
    ///
    /// The canonical entities id borrows the entities, they always exist.
    /// With `world_thread_only`, only the world's thread can insert the storage.
    /// Finding an existing one works from any thread, its own borrow rules apply.
    #[inline]
    fn generic_or_insert<S: 'static>(
        &self,
        storage_id: StorageId,
        world_thread_only: bool,
        new_storage: impl FnOnce() -> SBox,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage> {
        let storage = if storage_id == StorageId::of::<Entities>() {
            let entities: &AtomicRefCell<dyn Storage> = &self.entities;
            entities.borrow()
        } else {
            let storages = self.storages.read();
            let found = storages.any(&storage_id).map(|sbox| {
                // SAFE: `sbox` is in the registry and `storages` is still locked.
                unsafe { &*sbox.0 }.borrow()
            });
            drop(storages);

            match found {
                Some(storage) => storage,
                None if world_thread_only && !self.on_world_thread() => {
                    Err(error::Borrow::WrongThread)
                }
                None => {
                    let mut storages = self.storages.write();
                    let cell = match storages.any(&storage_id) {
                        Some(sbox) => sbox.0,
                        None => {
                            storages
                                .components_and_custom
                                .get_or_insert_with(storage_id, new_storage)
                                .0
                        }
                    };
                    // SAFE: `cell` comes from an `SBox` in the registry and `storages` is still locked.
                    let storage = unsafe { &*cell }.borrow();
                    drop(storages);

                    storage
                }
            }
        };

        match storage {
            Ok(storage) => Ok(ARef::map(storage, |storage| {
                storage.as_any().downcast_ref().unwrap()
            })),
            Err(err) => Err(error::GetStorage::storage_borrow::<S>(err)),
        }
    }

    /// Exclusively borrows the `S` storage registered under `storage_id`, or inserts
    /// `new_storage()` there and borrows it.
    ///
    /// The canonical entities id borrows the entities, they always exist.
    /// With `world_thread_only`, only the world's thread can insert the storage.
    /// Finding an existing one works from any thread, its own borrow rules apply.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[inline]
    fn generic_or_insert_mut<S: 'static>(
        &self,
        storage_id: StorageId,
        world_thread_only: bool,
        new_storage: impl FnOnce() -> SBox,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage> {
        let storage = if storage_id == StorageId::of::<Entities>() {
            let entities: &AtomicRefCell<dyn Storage> = &self.entities;
            entities.borrow_mut()
        } else {
            let storages = self.storages.read();
            let found = storages.any(&storage_id).map(|sbox| {
                // SAFE: `sbox` is in the registry and `storages` is still locked.
                unsafe { &*sbox.0 }.borrow_mut()
            });
            drop(storages);

            match found {
                Some(storage) => storage,
                None if world_thread_only && !self.on_world_thread() => {
                    Err(error::Borrow::WrongThread)
                }
                None => {
                    let mut storages = self.storages.write();
                    let cell = match storages.any(&storage_id) {
                        Some(sbox) => sbox.0,
                        None => {
                            storages
                                .components_and_custom
                                .get_or_insert_with(storage_id, new_storage)
                                .0
                        }
                    };
                    // SAFE: `cell` comes from an `SBox` in the registry and `storages` is still locked.
                    let storage = unsafe { &*cell }.borrow_mut();
                    drop(storages);

                    storage
                }
            }
        };

        match storage {
            Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                storage.as_any_mut().downcast_mut().unwrap()
            })),
            Err(err) => Err(error::GetStorage::storage_borrow::<S>(err)),
        }
    }

    /// Borrows the `S` component or custom storage registered under `storage_id`, or inserts
    /// `new_storage()` there and borrows it.
    ///
    /// With `world_thread_only`, only the world's thread can insert the storage.
    /// Finding an existing one works from any thread, its own borrow rules apply.
    #[inline]
    fn component_or_insert<S: 'static>(
        &self,
        storage_id: StorageId,
        world_thread_only: bool,
        new_storage: impl FnOnce() -> SBox,
    ) -> Result<ARef<'_, &'_ S>, error::GetStorage> {
        let storages = self.storages.read();
        let found = storages.component(&storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow()
        });
        drop(storages);

        let storage = match found {
            Some(storage) => storage,
            None if world_thread_only && !self.on_world_thread() => Err(error::Borrow::WrongThread),
            None => {
                let mut storages = self.storages.write();
                let components = &mut storages.components_and_custom;
                let cell = components.get_or_insert_with(storage_id, new_storage).0;
                // SAFE: `cell` comes from an `SBox` in the registry and `storages` is still locked.
                let storage = unsafe { &*cell }.borrow();
                drop(storages);

                storage
            }
        };

        match storage {
            Ok(storage) => Ok(ARef::map(storage, |storage| {
                storage.as_any().downcast_ref().unwrap()
            })),
            Err(err) => Err(error::GetStorage::storage_borrow::<S>(err)),
        }
    }

    /// Exclusively borrows the `S` component or custom storage registered under `storage_id`, or
    /// inserts `new_storage()` there and borrows it.
    ///
    /// With `world_thread_only`, only the world's thread can insert the storage.
    /// Finding an existing one works from any thread, its own borrow rules apply.
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[inline]
    fn component_or_insert_mut<S: 'static>(
        &self,
        storage_id: StorageId,
        world_thread_only: bool,
        new_storage: impl FnOnce() -> SBox,
    ) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage> {
        let storages = self.storages.read();
        let found = storages.component(&storage_id).map(|sbox| {
            // SAFE: `sbox` is in the registry and `storages` is still locked.
            unsafe { &*sbox.0 }.borrow_mut()
        });
        drop(storages);

        let storage = match found {
            Some(storage) => storage,
            None if world_thread_only && !self.on_world_thread() => Err(error::Borrow::WrongThread),
            None => {
                let mut storages = self.storages.write();
                let components = &mut storages.components_and_custom;
                let cell = components.get_or_insert_with(storage_id, new_storage).0;
                // SAFE: `cell` comes from an `SBox` in the registry and `storages` is still locked.
                let storage = unsafe { &*cell }.borrow_mut();
                drop(storages);

                storage
            }
        };

        match storage {
            Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                storage.as_any_mut().downcast_mut().unwrap()
            })),
            Err(err) => Err(error::GetStorage::storage_borrow::<S>(err)),
        }
    }

    /// Returns `true` when called from the world's thread, the only one that can create `!Send`
    /// storages.
    fn on_world_thread(&self) -> bool {
        #[cfg(feature = "thread_local")]
        {
            (self.thread_id_generator)() == self.main_thread_id
        }
        #[cfg(not(feature = "thread_local"))]
        {
            true
        }
    }
}

/// Exclusively borrows the `S` component storage in `registry`, inserting `f()` if it doesn't exist.
///
/// Takes the registry instead of `&AllStorages`, so bulk insertion can hold `&mut Entities` at the
/// same time. Only for `Send + Sync` storages, the others can only be inserted from the world's
/// thread and checking it needs `AllStorages`.
#[allow(clippy::mut_from_ref, reason = "Interior mutability")]
pub(crate) fn component_storage_or_insert_mut_in<S, F>(
    registry: &RwLock<StorageMaps>,
    f: F,
) -> Result<ARefMut<'_, &'_ mut S>, error::GetStorage>
where
    S: 'static + Storage + Send + Sync,
    F: FnOnce() -> S,
{
    let storage_id = StorageId::of::<S>();

    let storages = registry.read();
    let found = storages.component(&storage_id).map(|sbox| {
        // SAFE: `sbox` is in the registry and `storages` is still locked.
        unsafe { &*sbox.0 }.borrow_mut()
    });
    drop(storages);

    let storage = match found {
        Some(storage) => storage,
        None => {
            let mut storages = registry.write();
            let components = &mut storages.components_and_custom;
            let cell = components
                .get_or_insert_with(storage_id, || SBox::new(f()))
                .0;
            // SAFE: `cell` comes from an `SBox` in the registry and `storages` is still locked.
            let storage = unsafe { &*cell }.borrow_mut();
            drop(storages);

            storage
        }
    };

    match storage {
        Ok(storage) => Ok(ARefMut::map(storage, |storage| {
            storage.as_any_mut().downcast_mut().unwrap()
        })),
        Err(err) => Err(error::GetStorage::storage_borrow::<S>(err)),
    }
}
