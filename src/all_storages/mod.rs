mod builder;
mod clone;
mod custom_storage;
mod delete_any;
mod retain;

pub use custom_storage::CustomStorageAccess;
pub use delete_any::{CustomDeleteAny, TupleDeleteAny};
pub use retain::TupleRetainStorage;

pub(crate) use builder::AllStoragesBuilder;
pub(crate) use clone::TupleClone;

use crate::atomic_refcell::{ARef, ARefMut, AtomicRefCell};
use crate::borrow::Borrow;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::{Component, Unique};
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::get_component::GetComponent;
use crate::get_unique::GetUnique;
use crate::iter::{ShiperatorCaptain, ShiperatorSailor};
use crate::iter_component::{into_iter, IntoIterRef, IterComponent};
use crate::memory_usage::AllStoragesMemoryUsage;
use crate::public_transport::RwLock;
use crate::r#mut::Mut;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::{BulkAddEntity, SparseSet, TupleAddComponent, TupleDelete, TupleRemove};
#[cfg(feature = "thread_local")]
use crate::std_thread_id_generator;
use crate::storage::{SBox, Storage, StorageId};
use crate::system::AllSystem;
use crate::tracking::{TrackingTimestamp, TupleTrack};
use crate::unique::UniqueStorage;
use crate::views::EntitiesViewMut;
use crate::{error, ShipHashMap};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::type_name;
use core::sync::atomic::AtomicU64;
use hashbrown::hash_map::Entry;

#[allow(missing_docs)]
pub struct MissingLock;
#[allow(missing_docs)]
pub struct LockPresent;
#[allow(missing_docs)]
pub struct MissingThreadId;
#[allow(missing_docs)]
pub struct ThreadIdPresent;

/// Contains all storages present in the `World`.
// The lock is held very briefly:
// - shared: when trying to find a storage
// - unique: when adding a storage
// once the storage is found or created the lock is released
// this is safe since World is still borrowed and there is no way to delete a storage
// so any access to storages are valid as long as the World exists
// we use a HashMap, it can reallocate, but even in this case the storages won't move since they are boxed
pub struct AllStorages {
    pub(crate) storages: RwLock<ShipHashMap<StorageId, SBox>>,
    #[cfg(feature = "thread_local")]
    main_thread_id: u64,
    #[cfg(feature = "thread_local")]
    thread_id_generator: Arc<dyn Fn() -> u64 + Send + Sync>,
    counter: Arc<AtomicU64>,
}

#[cfg(not(feature = "thread_local"))]
unsafe impl Send for AllStorages {}

unsafe impl Sync for AllStorages {}

impl AllStorages {
    #[cfg(feature = "std")]
    pub(crate) fn new(counter: Arc<AtomicU64>) -> Self {
        let mut storages = ShipHashMap::new();

        storages.insert(StorageId::of::<Entities>(), SBox::new(Entities::new()));

        AllStorages {
            storages: RwLock::new_std(storages),
            #[cfg(feature = "thread_local")]
            main_thread_id: (std_thread_id_generator)(),
            #[cfg(feature = "thread_local")]
            thread_id_generator: Arc::new(std_thread_id_generator),
            counter,
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [`UniqueView`] or [`UniqueViewMut`].  
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Unique, World};
    ///
    /// #[derive(Unique)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.add_unique(USIZE(0));
    /// ```
    ///
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    pub fn add_unique<T: Send + Sync + Unique>(&self, component: T) {
        let storage_id = StorageId::of::<UniqueStorage<T>>();

        self.storages
            .write()
            .entry(storage_id)
            .insert(SBox::new(UniqueStorage::new(
                component,
                self.get_tracking_timestamp(),
            )));
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSend] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSend]: crate::NonSend
    /// [UniqueView]: crate::UniqueView
    /// [UniqueViewMut]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    pub fn add_unique_non_send<T: Sync + Unique>(&self, component: T) {
        if (self.thread_id_generator)() == self.main_thread_id {
            let storage_id = StorageId::of::<UniqueStorage<T>>();

            self.storages.write().entry(storage_id).or_insert_with(|| {
                SBox::new_non_send(
                    NonSend(UniqueStorage::new(component, self.get_tracking_timestamp())),
                    self.thread_id_generator.clone(),
                )
            });
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSync]: crate::NonSync
    /// [UniqueView]: crate::UniqueView
    /// [UniqueViewMut]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    pub fn add_unique_non_sync<T: Send + Unique>(&self, component: T) {
        let storage_id = StorageId::of::<UniqueStorage<T>>();

        self.storages.write().entry(storage_id).or_insert_with(|| {
            SBox::new_non_sync(NonSync(UniqueStorage::new(
                component,
                self.get_tracking_timestamp(),
            )))
        });
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    ///
    /// [NonSync]: crate::NonSync
    /// [UniqueView]: crate::UniqueView
    /// [UniqueViewMut]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    pub fn add_unique_non_send_sync<T: Unique>(&self, component: T) {
        if (self.thread_id_generator)() == self.main_thread_id {
            let storage_id = StorageId::of::<UniqueStorage<T>>();

            self.storages.write().entry(storage_id).or_insert_with(|| {
                SBox::new_non_send_sync(
                    NonSendSync(UniqueStorage::new(component, self.get_tracking_timestamp())),
                    self.thread_id_generator.clone(),
                )
            });
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
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Unique, World};
    ///
    /// #[derive(Unique)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.add_unique(USIZE(0));
    /// let i = all_storages.remove_unique::<USIZE>().unwrap();
    /// ```
    pub fn remove_unique<T: Unique>(&self) -> Result<T, error::UniqueRemove> {
        let storage_id = StorageId::of::<UniqueStorage<T>>();

        {
            let mut storages = self.storages.write();

            let storage = if let Entry::Occupied(entry) = storages.entry(storage_id) {
                // `.err()` to avoid borrowing `entry` in the `Ok` case
                if let Some(err) = unsafe { &*entry.get().0 }.borrow_mut().err() {
                    return Err(error::UniqueRemove::StorageBorrow((type_name::<T>(), err)));
                } else {
                    // We were able to lock the storage, we've still got exclusive access even though
                    // we released that lock as we're still holding the `AllStorages` lock.
                    entry.remove()
                }
            } else {
                return Err(error::UniqueRemove::MissingUnique(type_name::<T>()));
            };

            let unique: Box<AtomicRefCell<UniqueStorage<T>>> =
                unsafe { Box::from_raw(storage.0 as *mut AtomicRefCell<UniqueStorage<T>>) };

            core::mem::forget(storage);

            Ok(unique.into_inner().value)
        }
    }
    /// Delete an entity and all its components.
    /// Returns `true` if `entity` was alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, Get, View, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity1 = all_storages.add_entity((USIZE(0), U32(1)));
    /// let entity2 = all_storages.add_entity((USIZE(2), U32(3)));
    ///
    /// all_storages.delete_entity(entity1);
    ///
    /// all_storages.run(|usizes: View<USIZE>, u32s: View<U32>| {
    ///     assert!((&usizes).get(entity1).is_err());
    ///     assert!((&u32s).get(entity1).is_err());
    ///     assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    ///     assert_eq!(u32s.get(entity2), Ok(&U32(3)));
    /// });
    /// ```
    pub fn delete_entity(&mut self, entity: EntityId) -> bool {
        // no need to lock here since we have a unique access
        let mut entities = self.entities_mut().unwrap();

        if entities.delete_unchecked(entity) {
            drop(entities);

            self.strip(entity);

            true
        } else {
            false
        }
    }
    /// Deletes all components from an entity without deleting it.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity = all_storages.add_entity((U32(0), USIZE(1)));
    ///
    /// all_storages.strip(entity);
    /// ```
    #[track_caller]
    pub fn strip(&mut self, entity: EntityId) {
        let current = self.get_current();

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().delete(entity, current);
        }
    }
    /// Deletes all components of an entity except the ones passed in `S`.  
    /// The storage's type has to be used and not the component.  
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, sparse_set::SparseSet, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity = all_storages.add_entity((U32(0), USIZE(1)));
    ///
    /// all_storages.retain_storage::<SparseSet<U32>>(entity);
    /// ```
    pub fn retain_storage<S: TupleRetainStorage>(&mut self, entity: EntityId) {
        S::retain(self, entity);
    }
    /// Deletes all components of an entity except the ones passed in `S`.  
    /// This is identical to `retain_storage` but uses `StorageId` and not generics.  
    /// You should only use this method if you use a custom storage with a runtime id.
    #[track_caller]
    pub fn retain_storage_by_id(&mut self, entity: EntityId, excluded_storage: &[StorageId]) {
        let current = self.get_current();

        for (storage_id, storage) in self.storages.get_mut().iter_mut() {
            if !excluded_storage.contains(storage_id) {
                unsafe { &mut *storage.0 }.get_mut().delete(entity, current);
            }
        }
    }
    /// Deletes all entities and components in the `World`.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.clear();
    /// ```
    #[track_caller]
    pub fn clear(&mut self) {
        let current = self.get_current();

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().clear(current);
        }
    }
    /// Clear all deletion and removal tracking data.
    #[track_caller]
    pub fn clear_all_removed_and_deleted(&mut self) {
        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }
                .get_mut()
                .clear_all_removed_and_deleted();
        }
    }
    /// Clear all deletion and removal tracking data older than some timestamp.
    #[track_caller]
    pub fn clear_all_removed_and_deleted_older_than_timestamp(
        &mut self,
        timestamp: TrackingTimestamp,
    ) {
        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }
                .get_mut()
                .clear_all_removed_and_deleted_older_than_timestamp(timestamp);
        }
    }

    /// Clear all insertion tracking data.
    #[track_caller]
    pub fn clear_all_inserted(&mut self) {
        let now = self.get_tracking_timestamp();

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().clear_all_inserted(now);
        }
    }

    /// Clear all modification tracking data.
    #[track_caller]
    pub fn clear_all_modified(&mut self) {
        let now = self.get_tracking_timestamp();

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().clear_all_modified(now);
        }
    }

    /// Clear all insertion tracking data.
    #[track_caller]
    pub fn clear_all_inserted_and_modified(&mut self) {
        let now = self.get_tracking_timestamp();

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().clear_all_inserted(now);
            unsafe { &mut *storage.0 }.get_mut().clear_all_modified(now);
        }
    }

    /// Deletes all components for which `f(id, &component)` returns `false`.
    ///
    /// # Panics
    ///
    /// - Storage borrow failed.
    #[track_caller]
    pub fn retain<T: Component + Send + Sync>(&mut self, f: impl FnMut(EntityId, &T) -> bool) {
        let current = self.get_current();

        self.exclusive_storage_mut::<SparseSet<T>>()
            .unwrap()
            .private_retain(current, f);
    }

    /// Deletes all components for which `f(id, Mut<component>)` returns `false`.
    ///
    /// # Panics
    ///
    /// - Storage borrow failed.
    #[track_caller]
    pub fn retain_mut<T: Component + Send + Sync>(
        &mut self,
        f: impl FnMut(EntityId, Mut<'_, T>) -> bool,
    ) {
        let current = self.get_current();

        self.exclusive_storage_mut::<SparseSet<T>>()
            .unwrap()
            .private_retain_mut(current, f);
    }

    /// Creates a new entity with the components passed as argument and returns its `EntityId`.  
    /// `component` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity0 = all_storages.add_entity((U32(0),));
    /// let entity1 = all_storages.add_entity((U32(1), USIZE(11)));
    /// ```
    #[inline]
    pub fn add_entity<T: TupleAddComponent>(&mut self, component: T) -> EntityId {
        let current = self.get_current();

        let entity = self.exclusive_storage_mut::<Entities>().unwrap().generate();
        component.add_component(self, entity, current);

        entity
    }
    /// Creates multiple new entities and returns an iterator yielding the new `EntityId`s.  
    /// `source` must always yield a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let new_entities = all_storages.bulk_add_entity((10..20).map(|i| (U32(i as u32), USIZE(i))));
    /// ```
    #[inline]
    pub fn bulk_add_entity<T: BulkAddEntity>(&mut self, source: T) -> BulkEntityIter<'_> {
        source.bulk_add_entity(self)
    }
    /// Adds components to an existing entity.  
    /// If the entity already owned a component it will be replaced.  
    /// `component` must always be a tuple, even for a single component.  
    ///
    /// ### Panics
    ///
    /// - `entity` is not alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// // make an empty entity
    /// let entity = all_storages.add_entity(());
    ///
    /// all_storages.add_component(entity, (U32(0),));
    /// // entity already had a `u32` component so it will be replaced
    /// all_storages.add_component(entity, (U32(1), USIZE(11)));
    /// ```
    #[track_caller]
    #[inline]
    pub fn add_component<T: TupleAddComponent>(&mut self, entity: EntityId, component: T) {
        let current = self.get_current();

        if self
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(entity)
        {
            component.add_component(self, entity, current);
        } else {
            panic!("{:?}", error::AddComponent::EntityIsNotAlive);
        }
    }
    /// Deletes components from an entity. As opposed to `remove`, `delete` doesn't return anything.  
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity = all_storages.add_entity((U32(0), USIZE(1)));
    ///
    /// all_storages.delete_component::<(U32,)>(entity);
    /// ```
    #[inline]
    pub fn delete_component<C: TupleDelete>(&mut self, entity: EntityId) {
        C::delete(self, entity);
    }
    /// Removes components from an entity.  
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity = all_storages.add_entity((U32(0), USIZE(1)));
    ///
    /// let (i,) = all_storages.remove::<(U32,)>(entity);
    /// assert_eq!(i, Some(U32(0)));
    /// ```
    #[inline]
    pub fn remove<C: TupleRemove>(&mut self, entity: EntityId) -> C::Out {
        C::remove(self, entity)
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
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSync: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{AllStoragesViewMut, Component, EntitiesView, View, ViewMut, World};

#[derive(Component)]
struct U32(u32);

#[derive(Component)]
struct USIZE(usize);

let world = World::new();

let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let u32s = all_storages.borrow::<View<U32>>().unwrap();
let (entities, mut usizes) = all_storages
    .borrow::<(EntitiesView, ViewMut<USIZE>)>()
    .unwrap();
```
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    pub fn borrow<V: Borrow>(&self) -> Result<V::View<'_>, error::GetStorage> {
        let current = self.get_current();

        V::borrow(self, None, None, current)
    }
    #[doc = "Borrows the requested storages, runs the function and evaluates to the function's return value.  
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
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSync: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Panics

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[World]: crate::World
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    #[track_caller]
    pub fn run_with_data<Data, B, S: AllSystem<(Data,), B>>(
        &self,
        system: S,
        data: Data,
    ) -> S::Return {
        #[cfg(feature = "tracing")]
        let system_span = tracing::info_span!("system", name = ?type_name::<S>());
        #[cfg(feature = "tracing")]
        let _system_span = system_span.enter();

        system
            .run((data,), self)
            .map_err(error::Run::GetStorage)
            .unwrap()
    }
    #[doc = "Borrows the requested storages, runs the function and evaluates to the function's return value.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSync: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Panics

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{AllStoragesViewMut, Component, View, ViewMut, World};

#[derive(Component)]
struct I32(i32);

#[derive(Component)]
struct U32(u32);

#[derive(Component)]
struct USIZE(usize);

fn sys1(i32s: View<I32>) -> i32 {
    0
}

let world = World::new();

let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

all_storages
    .run(|usizes: View<USIZE>, mut u32s: ViewMut<U32>| {
        // -- snip --
    });

let i = all_storages.run(sys1);
```
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    #[track_caller]
    pub fn run<B, S: AllSystem<(), B>>(&self, system: S) -> S::Return {
        #[cfg(feature = "tracing")]
        let system_span = tracing::info_span!("system", name = ?type_name::<S>());
        #[cfg(feature = "tracing")]
        let _system_span = system_span.enter();

        system
            .run((), self)
            .map_err(error::Run::GetStorage)
            .unwrap()
    }
    /// Deletes any entity with at least one of the given type(s).  
    /// The storage's type has to be used and not the component.  
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, sparse_set::SparseSet, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// #[derive(Component)]
    /// struct STR(&'static str);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity0 = all_storages.add_entity((U32(0),));
    /// let entity1 = all_storages.add_entity((USIZE(1),));
    /// let entity2 = all_storages.add_entity((STR("2"),));
    ///
    /// // deletes `entity2`
    /// all_storages.delete_any::<SparseSet<STR>>();
    /// // deletes `entity0` and `entity1`
    /// all_storages.delete_any::<(SparseSet<U32>, SparseSet<USIZE>)>();
    /// ```
    pub fn delete_any<T: TupleDeleteAny>(&mut self) {
        T::delete_any(self);
    }
    pub(crate) fn entities(&self) -> Result<ARef<'_, &'_ Entities>, error::GetStorage> {
        let storage_id = StorageId::of::<Entities>();

        let storages = self.storages.read();
        let storage = storages.get(&storage_id).unwrap();
        let storage = unsafe { &*storage.0 }.borrow();
        drop(storages);
        match storage {
            Ok(storage) => Ok(ARef::map(storage, |storage| {
                storage.as_any().downcast_ref().unwrap()
            })),
            Err(err) => Err(error::GetStorage::Entities(err)),
        }
    }
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    pub(crate) fn entities_mut(&self) -> Result<ARefMut<'_, &'_ mut Entities>, error::GetStorage> {
        let storage_id = StorageId::of::<Entities>();

        let storages = self.storages.read();
        let storage = storages.get(&storage_id).unwrap();
        let storage = unsafe { &*storage.0 }.borrow_mut();
        drop(storages);
        match storage {
            Ok(storage) => Ok(ARefMut::map(storage, |storage| {
                storage.as_any_mut().downcast_mut().unwrap()
            })),
            Err(err) => Err(error::GetStorage::Entities(err)),
        }
    }
    pub(crate) fn exclusive_storage_mut<T: 'static>(
        &mut self,
    ) -> Result<&mut T, error::GetStorage> {
        self.exclusive_storage_mut_by_id(StorageId::of::<T>())
    }
    #[track_caller]
    pub(crate) fn exclusive_storage_mut_by_id<T: 'static>(
        &mut self,
        storage_id: StorageId,
    ) -> Result<&mut T, error::GetStorage> {
        if let Some(storage) = self.storages.get_mut().get_mut(&storage_id) {
            let storage = unsafe { &mut *storage.0 }
                .get_mut()
                .as_any_mut()
                .downcast_mut()
                .unwrap();
            Ok(storage)
        } else {
            Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<T>()),
                id: StorageId::of::<T>(),
            })
        }
    }
    pub(crate) fn exclusive_storage_or_insert_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Send + Sync,
        F: FnOnce() -> T,
    {
        let storages = self.storages.get_mut();

        unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new(f()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
    }
    #[cfg(feature = "thread_local")]
    #[track_caller]
    pub(crate) fn exclusive_storage_or_insert_non_send_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Sync,
        F: FnOnce() -> T,
    {
        let storages = self.storages.get_mut();

        unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_send(f(), self.thread_id_generator.clone()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
    }
    #[cfg(feature = "thread_local")]
    pub(crate) fn exclusive_storage_or_insert_non_sync_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Send,
        F: FnOnce() -> T,
    {
        let storages = self.storages.get_mut();

        unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_sync(f()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
    }
    #[cfg(feature = "thread_local")]
    #[track_caller]
    pub(crate) fn exclusive_storage_or_insert_non_send_sync_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage,
        F: FnOnce() -> T,
    {
        let storages = self.storages.get_mut();

        unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_send_sync(f(), self.thread_id_generator.clone()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
    }
    /// Make the given entity alive.  
    /// Does nothing if an entity with a greater generation is already at this index.  
    /// Returns `true` if the entity is successfully spawned.
    #[inline]
    pub fn spawn(&mut self, entity: EntityId) -> bool {
        self.exclusive_storage_mut::<Entities>()
            .unwrap()
            .spawn(entity)
    }
    /// Displays storages memory information.
    pub fn memory_usage(&self) -> AllStoragesMemoryUsage<'_> {
        AllStoragesMemoryUsage(self)
    }

    #[inline]
    pub(crate) fn get_current(&self) -> TrackingTimestamp {
        TrackingTimestamp::new(
            self.counter
                .fetch_add(1, core::sync::atomic::Ordering::Acquire),
        )
    }

    /// Returns a timestamp used to clear tracking information.
    pub fn get_tracking_timestamp(&self) -> TrackingTimestamp {
        TrackingTimestamp::new(self.counter.load(core::sync::atomic::Ordering::Acquire))
    }

    /// Enable insertion tracking for the given components.
    pub fn track_insertion<T: TupleTrack>(&mut self) -> &mut AllStorages {
        T::track_insertion(self);
        self
    }

    /// Enable modification tracking for the given components.
    pub fn track_modification<T: TupleTrack>(&mut self) -> &mut AllStorages {
        T::track_modification(self);
        self
    }

    /// Enable deletion tracking for the given components.
    pub fn track_deletion<T: TupleTrack>(&mut self) -> &mut AllStorages {
        T::track_deletion(self);
        self
    }

    /// Enable removal tracking for the given components.
    pub fn track_removal<T: TupleTrack>(&mut self) -> &mut AllStorages {
        T::track_removal(self);
        self
    }

    /// Enable insertion, deletion and removal tracking for the given components.
    pub fn track_all<T: TupleTrack>(&mut self) {
        T::track_all(self);
    }

    #[doc = "Retrieve components of `entity`.

Multiple components can be queried at the same time using a tuple.

You can use:
* `&T` for a shared access to `T` component
* `&mut T` for an exclusive access to `T` component"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local"),
        doc = "* [NonSend]<&T> for a shared access to a `T` component where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send`
* [NonSync]<&T> for a shared access to a `T` component where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Sync`
* [NonSendSync]<&T> for a shared access to a `T` component where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send` nor `Sync`"
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature
* NonSync: must activate the *thread_local* feature
* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- [AllStorages] (shared) + storage (exclusive or shared)

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Entity does not have the component.

### Example
```
use shipyard::{AllStoragesViewMut, Component, World};

#[derive(Component, Debug, PartialEq, Eq)]
struct U32(u32);

#[derive(Component, Debug, PartialEq, Eq)]
struct USIZE(usize);

let world = World::new();
let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let entity = all_storages.add_entity((USIZE(0), U32(1)));

let (i, j) = all_storages.get::<(&USIZE, &mut U32)>(entity).unwrap();

assert!(*i == &USIZE(0));
assert!(*j == &U32(1));
```"]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSend]: crate::NonSend
[NonSync]: crate::NonSync
[NonSendSync]: crate::NonSendSync"
    )]
    #[inline]
    pub fn get<T: GetComponent>(
        &self,
        entity: EntityId,
    ) -> Result<T::Out<'_>, error::GetComponent> {
        let current = self.get_current();

        T::get(self, None, current, entity)
    }

    #[doc = "Retrieve a unique component.

You can use:
* `&T` for a shared access to `T` unique component
* `&mut T` for an exclusive access to `T` unique component"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local"),
        doc = "* [NonSend]<&T> for a shared access to a `T` unique component where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` unique component where `T` isn't `Send`
* [NonSync]<&T> for a shared access to a `T` unique component where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` unique component where `T` isn't `Sync`
* [NonSendSync]<&T> for a shared access to a `T` unique component where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` unique component where `T` isn't `Send` nor `Sync`"
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature
* NonSync: must activate the *thread_local* feature
* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- [AllStorages] (shared) + storage (exclusive or shared)

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.

### Example
```
use shipyard::{AllStoragesViewMut, Unique, World};

#[derive(Unique, Debug, PartialEq, Eq)]
struct U32(u32);

let world = World::new();
let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

all_storages.add_unique(U32(0));

let i = all_storages.get_unique::<&U32>().unwrap();

assert!(*i == U32(0));
```"]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSend]: crate::NonSend
[NonSync]: crate::NonSync
[NonSendSync]: crate::NonSendSync"
    )]
    #[inline]
    pub fn get_unique<T: GetUnique>(&self) -> Result<T::Out<'_>, error::GetStorage> {
        T::get_unique(self, None)
    }

    #[doc = "Iterate components.

Multiple components can be iterated at the same time using a tuple.

You can use:
* `&T` for a shared access to `T` component
* `&mut T` for an exclusive access to `T` component"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local"),
        doc = "* [NonSend]<&T> for a shared access to a `T` component where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send`
* [NonSync]<&T> for a shared access to a `T` component where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Sync`
* [NonSendSync]<&T> for a shared access to a `T` component where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send` nor `Sync`"
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature
* NonSync: must activate the *thread_local* feature
* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- [AllStorages] (shared)

### Panics

- [AllStorages] borrow failed.

### Example
```
use shipyard::{AllStoragesViewMut, Component, World};

#[derive(Component, Debug, PartialEq, Eq)]
struct U32(u32);

#[derive(Component, Debug, PartialEq, Eq)]
struct USIZE(usize);

let world = World::new();
let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let entity = all_storages.add_entity((USIZE(0), U32(1)));

let mut iter = all_storages.iter::<(&USIZE, &mut U32)>();

for (i, j) in &mut iter {
    // <-- SNIP -->
}
```"]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSend]: crate::NonSend
[NonSync]: crate::NonSync
[NonSendSync]: crate::NonSendSync"
    )]
    #[inline]
    #[track_caller]
    pub fn iter<'a, T: IterComponent>(&'a self) -> IntoIterRef<'a, T>
    where
        <T as IterComponent>::Shiperator<'a>: ShiperatorCaptain + ShiperatorSailor,
    {
        let current = self.get_current();

        into_iter(self, None, current).unwrap()
    }

    /// Sets the on entity deletion callback.
    ///
    /// ### Borrows
    ///
    /// - Entities (exclusive)
    ///
    /// ### Panics
    ///
    /// - Entities borrow failed.
    #[track_caller]
    pub fn on_deletion(&self, f: impl FnMut(EntityId) + Send + Sync + 'static) {
        let mut entities = self.borrow::<EntitiesViewMut<'_>>().unwrap();

        entities.on_deletion(f);
    }

    /// Returns true if entity matches a living entity.
    pub fn is_entity_alive(&mut self, entity: EntityId) -> bool {
        self.exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(entity)
    }

    /// Moves an entity from a `World` to another.
    ///
    /// ### Panics
    ///
    /// - `entity` is not alive
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let world1 = World::new();
    /// let world2 = World::new();
    ///
    /// let mut all_storages1 = world1.borrow::<AllStoragesViewMut>().unwrap();
    /// let mut all_storages2 = world2.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity = all_storages1.add_entity(USIZE(1));
    ///
    /// all_storages1.move_entity(&mut all_storages2, entity);
    ///
    /// assert!(!all_storages1.is_entity_alive(entity));
    /// assert_eq!(all_storages2.get::<&USIZE>(entity).as_deref(), Ok(&&USIZE(1)));
    /// ```
    #[track_caller]
    pub fn move_entity(&mut self, other: &mut AllStorages, entity: EntityId) {
        let current = self.get_current();
        let other_current = other.get_current();

        if !self
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .delete_unchecked(entity)
        {
            panic!(
                "Entity {:?} has to be alive to move it to another World.",
                entity
            );
        };

        assert!(
            other
                .exclusive_storage_mut::<Entities>()
                .unwrap()
                .spawn(entity),
            "Other World already has an entity at {:?}'s index.",
            entity
        );

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().move_component_from(
                other,
                entity,
                entity,
                current,
                other_current,
            );
        }
    }

    /// Moves all components from an entity to another in another `World`.
    ///
    /// ### Panics
    ///
    /// - `from` is not alive
    /// - `to` is not alive
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let world1 = World::new();
    /// let world2 = World::new();
    ///
    /// let mut all_storages1 = world1.borrow::<AllStoragesViewMut>().unwrap();
    /// let mut all_storages2 = world2.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let from = all_storages1.add_entity(USIZE(1));
    /// let to = all_storages2.add_entity(());
    ///
    /// all_storages1.move_components(&mut all_storages2, from, to);
    ///
    /// assert!(all_storages1.get::<&USIZE>(from).is_err());
    /// assert_eq!(all_storages2.get::<&USIZE>(to).as_deref(), Ok(&&USIZE(1)));
    /// ```
    #[track_caller]
    pub fn move_components(&mut self, other: &mut AllStorages, from: EntityId, to: EntityId) {
        let current = self.get_current();
        let other_current = other.get_current();

        if !self
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(from)
        {
            panic!(
                "Entity {:?} has to be alive to move its components to another World.",
                from
            );
        };

        if !other
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(to)
        {
            panic!(
                "Entity {:?} has to be alive to receive components from another World.",
                to
            );
        };

        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().move_component_from(
                other,
                from,
                to,
                current,
                other_current,
            );
        }
    }

    /// Registers the function to clone these components.
    #[inline]
    pub fn register_clone<T: TupleClone>(&mut self) {
        T::register_clone(self);
    }

    /// Clones all storages with a registered clone function from this `AllStorages` to `other`.
    ///
    /// Tracking is not cloned. Components will count as inserted in `other`.
    #[track_caller]
    pub fn clone_storages_to(&self, other: &mut AllStorages) {
        let other_current = other.get_current();
        let other_storages = other.storages.get_mut();

        for (storage_id, storage) in self.storages.read().iter() {
            let storage = unsafe { &*storage.0 };
            #[cfg(feature = "thread_local")]
            let other_thread_id_generator = other.thread_id_generator.clone();
            #[cfg(feature = "thread_local")]
            let is_send = storage.is_send();
            #[cfg(feature = "thread_local")]
            let is_sync = storage.is_sync();

            #[cfg(feature = "thread_local")]
            {
                if !is_send || !is_sync {
                    let thread_id = (self.thread_id_generator)();

                    if thread_id != self.main_thread_id {
                        panic!("Cannot clone !Send or !Sync storage in another thread.")
                    }
                }

                if !is_send {
                    let other_thread_id = (other_thread_id_generator)();

                    if other_thread_id != other.main_thread_id {
                        panic!("Cannot clone !Send storage to World from other thread.")
                    }
                }
            }

            let storage_borrow = storage.borrow().unwrap();
            if let Some(storage_builder) = Storage::try_clone(&*storage_borrow, other_current) {
                let storage = {
                    #[cfg(not(feature = "thread_local"))]
                    {
                        storage_builder.build()
                    }
                    #[cfg(feature = "thread_local")]
                    {
                        let storage_type_id = storage_borrow.any().type_id();
                        let storage_clone_type_id = unsafe { &*storage_builder.sbox.0 }
                            .borrow()
                            .unwrap()
                            .any()
                            .type_id();

                        // The implementor could return another type when cloning as the type is erased.
                        // We only check in the thread_local case because the other type could be !Send/!Sync.
                        // If the type doesn't match there will be a panic when creating views in any case.
                        if storage_type_id != storage_clone_type_id {
                            panic!("Storage clone is not of the same type as original.")
                        }

                        // SAFE
                        // We pass the information from the original storage and the types are the same.
                        unsafe {
                            storage_builder.build(other_thread_id_generator, is_send, is_sync)
                        }
                    }
                };

                other_storages.insert(*storage_id, storage);
            }
        }
    }

    /// Clones `entity` from this `AllStorages` to `other_all_storages` alongside all its with a registered clone function.
    #[track_caller]
    pub fn clone_entity_to(&self, other_all_storages: &mut AllStorages, entity: EntityId) {
        let other_current = other_all_storages.get_current();

        if !self.entities().unwrap().is_alive(entity) {
            panic!(
                "Entity {:?} has to be alive to move it to another World.",
                entity
            );
        };

        assert!(
            other_all_storages
                .exclusive_storage_mut::<Entities>()
                .unwrap()
                .spawn(entity),
            "Other World already has an entity at {:?}'s index.",
            entity
        );

        for storage in self.storages.read().values() {
            unsafe { &mut *storage.0 }
                .borrow()
                .unwrap()
                .clone_component_to(other_all_storages, entity, entity, other_current);
        }
    }

    /// Clones all components of `from` entity with a registered clone function from
    /// this `AllStorages` to `other_all_storages`'s `to` entity.
    #[track_caller]
    pub fn clone_components_to(
        &self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
    ) {
        let other_current = other_all_storages.get_current();

        if !self.entities().unwrap().is_alive(from) {
            panic!(
                "Entity {:?} has to be alive to move its components to another World.",
                from
            );
        };

        if !other_all_storages
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(to)
        {
            panic!(
                "Entity {:?} has to be alive to receive components from another World.",
                to
            );
        };

        for storage in self.storages.read().values() {
            unsafe { &mut *storage.0 }
                .borrow()
                .unwrap()
                .clone_component_to(other_all_storages, from, to, other_current);
        }
    }
}

impl core::fmt::Debug for AllStorages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("AllStorages");

        let storages = self.storages.read();

        debug_struct.field("storage_count", &storages.len());
        debug_struct.field("storages", &storages.values());

        debug_struct.finish()
    }
}

impl core::fmt::Debug for AllStoragesMemoryUsage<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut borrowed_storages = 0;

        let mut debug_struct = f.debug_list();

        let storages = self.0.storages.read();

        debug_struct.entries(storages.values().filter_map(|storage| {
            match unsafe { &*(storage.0) }.borrow() {
                Ok(storage) => storage.memory_usage(),
                Err(_) => {
                    borrowed_storages += 1;
                    None
                }
            }
        }));

        if borrowed_storages != 0 {
            debug_struct.entry(&format_args!(
                "{} storages could not be borrored",
                borrowed_storages
            ));
        }

        debug_struct.finish()
    }
}
