mod builder;
mod clone;
mod custom_storage;
mod delete_any;
mod group;
mod regroup;
mod retain;
mod storage_maps;
#[cfg(test)]
mod tests;

pub use custom_storage::CustomStorageAccess;
pub(crate) use custom_storage::component_storage_or_insert_mut_in;
pub use delete_any::{CustomDeleteAny, TupleDeleteAny};
pub use group::GroupMember;
#[doc(hidden)]
pub use group::{MemberInfo, TupleGroup};
pub use retain::TupleRetainStorage;

pub(crate) use builder::AllStoragesBuilder;
pub(crate) use clone::TupleClone;
pub(crate) use group::{ConnectedSet, Grouping};
pub(crate) use storage_maps::StorageMaps;

use crate::atomic_refcell::{ARef, ARefMut, AtomicRefCell};
use crate::borrow::Borrow;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::{Component, Unique};
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::get_component::GetComponent;
use crate::get_unique::GetUnique;
use crate::iter::Shiperator;
use crate::iter_component::{IntoIterRef, IterComponent, into_iter};
use crate::memory_usage::AllStoragesMemoryUsage;
use crate::r#mut::Mut;
use crate::public_transport::RwLock;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::{
    BulkAddEntity, Groups, SparseSet, TupleAddComponent, TupleDelete, TupleRemove,
};
#[cfg(all(feature = "std", feature = "thread_local"))]
use crate::std_thread_id_generator;
use crate::storage::{SBox, Storage, StorageId};
use crate::system::AllSystem;
use crate::tracking::{TrackingTimestamp, TupleTrack};
use crate::unique::UniqueStorage;
use crate::views::EntitiesViewMut;
use crate::{ShipHashMap, error};
use alloc::boxed::Box;
use alloc::sync::Arc;
use clone::adopt_groups;
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

/// Contains all storages present in the [`World`](crate::world::World).
// The registry is two maps under one lock, see `StorageMaps`: the components and custom storages,
// and the uniques. The lock is held very briefly:
// - shared: when looking up a storage
// - exclusive: when adding or removing a storage
// Storages are boxed, so they don't move when a map reallocates.
//
// Once we find a storage, we borrow it *before* releasing the lock. From then on, the runtime
// borrow is what keeps the storage valid, not the lock:
// - through `&AllStorages`, a storage can only be removed (`remove_unique`) or have its value
//   replaced (`add_unique*`) after borrowing it exclusively, which fails while any guard is alive
// - everything else that drops storages takes `&mut AllStorages`, which can't coexist with a guard
// So a storage guard stays valid for as long as it's alive, even without the lock.
//
// `Entities` isn't in the registry, it has its own field. Every world has exactly one and it's never
// removed, so entity access doesn't need a lookup or the lock. Generic access finds it under its
// canonical id, `StorageId::of::<Entities>()`, which the registry never contains.
pub struct AllStorages {
    pub(crate) entities: AtomicRefCell<Entities>,
    pub(crate) storages: RwLock<StorageMaps>,
    #[cfg(feature = "thread_local")]
    main_thread_id: u64,
    #[cfg(feature = "thread_local")]
    thread_id_generator: Arc<dyn Fn() -> u64 + Send + Sync>,
    counter: Arc<AtomicU64>,
    /// The groups declared when building the world, see [`Grouping`].
    pub(crate) grouping: Grouping,
}

#[cfg(not(feature = "thread_local"))]
unsafe impl Send for AllStorages {}

unsafe impl Sync for AllStorages {}

impl AllStorages {
    #[cfg(feature = "std")]
    pub(crate) fn new(counter: Arc<AtomicU64>) -> Self {
        AllStorages {
            entities: AtomicRefCell::new(Entities::new()),
            storages: RwLock::new_std(StorageMaps::new()),
            #[cfg(feature = "thread_local")]
            main_thread_id: (std_thread_id_generator)(),
            #[cfg(feature = "thread_local")]
            thread_id_generator: Arc::new(std_thread_id_generator),
            counter,
            grouping: Grouping::default(),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.\
    /// To access a unique storage value, use [`UniqueView`] or [`UniqueViewMut`].
    ///
    /// If `T`'s unique storage already exists, `component` replaces its value instead.\
    /// The replacement is tracked as a modification, not an insertion.\
    /// Everything else about the storage is preserved: its insertion tracking, its registered
    /// clone function, and the wrapper and thread restrictions it was first inserted with.
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive), only if it already exists
    ///
    /// ### Panics
    ///
    /// - `T` storage borrow failed, only if it already exists.
    ///   This happens when the storage is already borrowed, or when it was inserted as `!Send`
    ///   and we're not on the thread it was inserted on.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Unique, UniqueView, World};
    ///
    /// #[derive(Unique)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.add_unique(USIZE(0));
    /// all_storages.add_unique(USIZE(1));
    ///
    /// let i = all_storages.borrow::<UniqueView<USIZE>>().unwrap();
    /// assert_eq!(i.0, 1);
    /// ```
    ///
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    #[track_caller]
    pub fn add_unique<T: Send + Sync + Unique>(&self, component: T) {
        self.add_or_replace_unique(component, SBox::new);
    }
    /// Adds `T`'s unique storage, built by `new_storage`, or replaces the value of the existing one.
    ///
    /// ### Panics
    ///
    /// - `T`'s existing storage can't be borrowed exclusively.
    #[track_caller]
    fn add_or_replace_unique<T: Unique>(
        &self,
        component: T,
        new_storage: impl FnOnce(UniqueStorage<T>) -> SBox,
    ) {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        let current = self.get_tracking_timestamp();
        let mut storages = self.storages.write();

        // A unique registered through a custom path is found too, its value is replaced in place.
        let Some(sbox) = storages.unique(storage_id) else {
            storages.uniques.insert(
                storage_id,
                new_storage(UniqueStorage::new(component, current)),
            );
            return;
        };

        // We borrow the storage before releasing the registry lock, so nobody can remove it in between.
        // SAFE: `sbox.0` comes from the `SBox` we just found in the registry, so it points to a live
        // boxed `AtomicRefCell`. We only use `storage` while that box stays alive. While we hold the
        // registry lock nobody can remove it, and we drop our borrow before taking it out ourselves.
        // After releasing the lock we only touch it through a successful exclusive borrow, which
        // keeps `remove_unique` from taking the box out. Freeing it any other way requires
        // `&mut AllStorages`, which can't coexist with our `&self`.
        let storage = unsafe { &*sbox.0 };
        let mut storage = match storage.borrow_mut() {
            Ok(storage) => storage,
            Err(err) => {
                // We panic after releasing the registry lock, so we don't poison it.
                drop(storages);

                panic!(
                    "{:?}",
                    error::GetStorage::storage_borrow::<UniqueStorage<T>>(err)
                )
            }
        };

        // The first insertion determines the storage's wrapper and thread-access policy.
        // Replacing its value preserves both, even when called through a different
        // add_unique* variant. The existing storage's thread restrictions still apply.
        let Some(unique) = unique_storage_mut::<T>(&mut *storage) else {
            // Someone registered a custom storage that isn't a unique storage under `T`'s unique
            // storage id. We hold the only borrow of it, so we can take it out and insert a fresh
            // unique storage in its place. Its destructor might be user code, we run it once the
            // registry is unlocked.
            // The fresh storage goes with the uniques, it was registered through the unique API.
            // The foreign one leaves whichever map it was in, so the id stays in a single map.
            drop(storage);
            let foreign = storages
                .uniques
                .remove(&storage_id)
                .or_else(|| storages.components_and_custom.remove(&storage_id));
            storages.uniques.insert(
                storage_id,
                new_storage(UniqueStorage::new(component, current)),
            );
            drop(storages);
            drop(foreign);

            return;
        };

        drop(storages);

        let displaced = core::mem::replace(&mut unique.value, component);
        unique.modification = current;

        // The displaced value's destructor is user code, it could panic or access the world again.
        // We release our borrow first, so it only ever sees the storage with its new value, unborrowed.
        drop(storage);
        drop(displaced);
    }
    /// Adds a new `!Send` unique storage, unique storages store exactly one `T` at any time.\
    /// To access a unique storage value, use [`NonSend`] with [`UniqueView`] or [`UniqueViewMut`].
    ///
    /// If `T`'s unique storage already exists, `component` replaces its value instead.\
    /// The replacement is tracked as a modification, not an insertion.\
    /// Everything else about the storage is preserved: its insertion tracking, its registered
    /// clone function, and the wrapper and thread restrictions it was first inserted with.
    ///
    /// The first insertion decides which wrapper views have to use, whichever `add_unique*` method
    /// replaces the value later. A unique added with this method is always borrowed with
    /// [`NonSend`], even after [`add_unique`](AllStorages::add_unique) replaces its value.
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive), only if it already exists
    ///
    /// ### Panics
    ///
    /// - Not called from the [`World`]'s thread.
    /// - `T` storage borrow failed, only if it already exists.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{borrow::NonSend, AllStoragesViewMut, Unique, UniqueView, World};
    ///
    /// #[derive(Unique)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    /// let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// // I'm using `u32` here but imagine it's a `!Send` type
    /// all_storages.add_unique_non_send(U32(0));
    /// all_storages.add_unique(U32(1));
    ///
    /// // The storage is still `NonSend`, it was first inserted with `add_unique_non_send`.
    /// let i = all_storages.borrow::<NonSend<UniqueView<U32>>>().unwrap();
    /// assert_eq!(i.0, 1);
    /// ```
    ///
    /// [`NonSend`]: crate::borrow::NonSend
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`World`]: crate::World
    #[cfg(feature = "thread_local")]
    #[track_caller]
    pub fn add_unique_non_send<T: Sync + Unique>(&self, component: T) {
        assert!(
            (self.thread_id_generator)() == self.main_thread_id,
            "Cannot add {} unique from other thread than the World's because it's !Send.",
            type_name::<T>()
        );

        self.add_or_replace_unique(component, |unique| {
            SBox::new_non_send(NonSend(unique), self.thread_id_generator.clone())
        });
    }
    /// Adds a new `!Sync` unique storage, unique storages store exactly one `T` at any time.\
    /// To access a unique storage value, use [`NonSync`] with [`UniqueView`] or [`UniqueViewMut`].
    ///
    /// If `T`'s unique storage already exists, `component` replaces its value instead.\
    /// The replacement is tracked as a modification, not an insertion.\
    /// Everything else about the storage is preserved: its insertion tracking, its registered
    /// clone function, and the wrapper and thread restrictions it was first inserted with.
    ///
    /// The first insertion decides which wrapper views have to use, whichever `add_unique*` method
    /// replaces the value later. See [`add_unique_non_send`](AllStorages::add_unique_non_send)
    /// for an example.
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive), only if it already exists
    ///
    /// ### Panics
    ///
    /// - `T` storage borrow failed, only if it already exists.
    ///   This happens when the storage is already borrowed, or when it was inserted as `!Send`
    ///   and we're not on the thread it was inserted on.
    ///
    /// [`NonSync`]: crate::borrow::NonSync
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    #[track_caller]
    pub fn add_unique_non_sync<T: Send + Unique>(&self, component: T) {
        self.add_or_replace_unique(component, |unique| SBox::new_non_sync(NonSync(unique)));
    }
    /// Adds a new `!Send + !Sync` unique storage, unique storages store exactly one `T` at any time.\
    /// To access a unique storage value, use [`NonSendSync`] with [`UniqueView`] or [`UniqueViewMut`].
    ///
    /// If `T`'s unique storage already exists, `component` replaces its value instead.\
    /// The replacement is tracked as a modification, not an insertion.\
    /// Everything else about the storage is preserved: its insertion tracking, its registered
    /// clone function, and the wrapper and thread restrictions it was first inserted with.
    ///
    /// The first insertion decides which wrapper views have to use, whichever `add_unique*` method
    /// replaces the value later. See [`add_unique_non_send`](AllStorages::add_unique_non_send)
    /// for an example.
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive), only if it already exists
    ///
    /// ### Panics
    ///
    /// - Not called from the [`World`]'s thread.
    /// - `T` storage borrow failed, only if it already exists.
    ///
    /// [`NonSendSync`]: crate::borrow::NonSendSync
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`World`]: crate::World
    #[cfg(feature = "thread_local")]
    #[track_caller]
    pub fn add_unique_non_send_sync<T: Unique>(&self, component: T) {
        assert!(
            (self.thread_id_generator)() == self.main_thread_id,
            "Cannot add {} unique from other thread than the World's because it's !Send.",
            type_name::<T>()
        );

        self.add_or_replace_unique(component, |unique| {
            SBox::new_non_send_sync(NonSendSync(unique), self.thread_id_generator.clone())
        });
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

            // Uniques first, then a unique registered through a custom path.
            let map = if storages.uniques.contains_key(&storage_id) {
                &mut storages.uniques
            } else {
                &mut storages.components_and_custom
            };

            let storage = if let Entry::Occupied(entry) = map.entry(storage_id) {
                // `.err()` to avoid borrowing `entry` in the `Ok` case
                // SAFE: `entry` is in the registry and we hold its write lock, so the `SBox` points
                // to a live boxed `AtomicRefCell`. The borrow guard is dropped right away.
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

            // Depending on how it was added, the unique storage is wrapped in a thread-local type
            // or not. We check which type it is to take it out as the type it was created with.
            // SAFE: `storage.0` comes from `Box::into_raw` and we hold the only `SBox` owning it,
            // we just removed it from the map. No borrow of it is alive: `borrow_mut` succeeded
            // and we still hold the map's lock, nobody could have found it since.
            let stored = unsafe { &mut *storage.0 }.get_mut().any();

            if stored.is::<UniqueStorage<T>>() {
                // SAFE: We just checked `storage` holds a `UniqueStorage<T>`.
                return Ok(unsafe { take_storage::<UniqueStorage<T>>(storage) }.value);
            }

            #[cfg(feature = "thread_local")]
            {
                if stored.is::<NonSend<UniqueStorage<T>>>() {
                    // SAFE: We just checked `storage` holds a `NonSend<UniqueStorage<T>>`.
                    let unique = unsafe { take_storage::<NonSend<UniqueStorage<T>>>(storage) };
                    return Ok(unique.0.value);
                }
                if stored.is::<NonSync<UniqueStorage<T>>>() {
                    // SAFE: We just checked `storage` holds a `NonSync<UniqueStorage<T>>`.
                    let unique = unsafe { take_storage::<NonSync<UniqueStorage<T>>>(storage) };
                    return Ok(unique.0.value);
                }
                if stored.is::<NonSendSync<UniqueStorage<T>>>() {
                    // SAFE: We just checked `storage` holds a `NonSendSync<UniqueStorage<T>>`.
                    let unique = unsafe { take_storage::<NonSendSync<UniqueStorage<T>>>(storage) };
                    return Ok(unique.0.value);
                }
            }

            unreachable!("unique storages are only added with these types")
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
        if self.entities.get_mut().delete_unchecked(entity) {
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

        // Uniques don't hold per entity data, we only visit components and custom storages.
        for storage in self.storages.get_mut().components_and_custom.values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
            unsafe { &mut *storage.0 }.get_mut().delete(entity, current);
        }
    }

    /// Deletes all components of multiple entities without deleting them.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, View, World};
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
    /// let eid0 = all_storages.add_entity((U32(0), USIZE(1)));
    /// let eid1 = all_storages.add_entity(USIZE(10));
    /// let eid2 = all_storages.add_entity(U32(30));
    ///
    /// all_storages.bulk_strip([eid0, eid1, eid2]);
    ///
    /// let (v_u32, v_usize) = all_storages.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    /// assert_eq!(v_u32.len(), 0);
    /// assert_eq!(v_usize.len(), 0);
    /// ```
    #[track_caller]
    pub fn bulk_strip<I: IntoIterator<Item = EntityId>>(&mut self, entities: I)
    where
        I::IntoIter: Clone,
    {
        #[inline]
        fn inner<I: Iterator<Item = EntityId> + Clone>(
            all_storages: &mut AllStorages,
            iter: &I,
            current: TrackingTimestamp,
        ) {
            // Uniques don't hold per entity data, we only visit components and custom storages.
            let storages = &mut all_storages.storages.get_mut().components_and_custom;

            for storage in storages.values_mut() {
                // SAFE: `storage` is in the registry and we hold `&mut AllStorages`, nobody
                // else can reach it.
                let storage = unsafe { &mut *storage.0 }.get_mut();

                for entity_id in iter.clone() {
                    storage.delete(entity_id, current);
                }
            }
        }

        let current = self.get_current();

        let iter = entities.into_iter();
        inner(self, &iter, current);
    }

    /// Deletes in parallel all components of multiple entities without deleting them.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, View, World};
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
    /// let eid0 = all_storages.add_entity((U32(0), USIZE(1)));
    /// let eid1 = all_storages.add_entity(USIZE(10));
    /// let eid2 = all_storages.add_entity(U32(30));
    ///
    /// all_storages.par_strip([eid0, eid1, eid2]);
    ///
    /// let (v_u32, v_usize) = all_storages.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    /// assert_eq!(v_u32.len(), 0);
    /// assert_eq!(v_usize.len(), 0);
    /// ```
    #[track_caller]
    #[cfg(all(feature = "parallel", not(feature = "thread_local")))]
    pub fn par_strip<I: IntoIterator<Item = EntityId>>(&mut self, entities: I)
    where
        I::IntoIter: Clone + Sync,
    {
        #[inline]
        fn inner<I: Iterator<Item = EntityId> + Clone + Sync>(
            all_storages: &mut AllStorages,
            iter: &I,
            current: TrackingTimestamp,
        ) {
            use rayon::prelude::*;

            // Uniques don't hold per entity data, we only visit components and custom storages.
            let storages = &mut all_storages.storages.get_mut().components_and_custom;

            storages.par_iter_mut().for_each(|(_, storage)| {
                // SAFE: `storage` is in the registry and we hold `&mut AllStorages`, each task
                // gets a different storage.
                let storage = unsafe { &mut *storage.0 }.get_mut();

                for entity_id in iter.clone() {
                    storage.delete(entity_id, current);
                }
            });
        }

        let current = self.get_current();

        let iter = entities.into_iter();
        inner(self, &iter, current);
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

        // Uniques don't hold per entity data, we only visit components and custom storages.
        let storages = &mut self.storages.get_mut().components_and_custom;

        for (storage_id, storage) in storages.iter_mut() {
            if !excluded_storage.contains(storage_id) {
                // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
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

        // The entities aren't in the registry, we clear them on their own.
        self.entities.get_mut().clear(current);

        // Uniques can't be cleared, we only visit components and custom storages.
        for storage in self.storages.get_mut().components_and_custom.values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
            unsafe { &mut *storage.0 }.get_mut().clear(current);
        }
    }
    /// Clear all deletion and removal tracking data.
    #[track_caller]
    pub fn clear_all_removed_and_deleted(&mut self) {
        // Uniques can't be deleted or removed, we only visit components and custom storages.
        for storage in self.storages.get_mut().components_and_custom.values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
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
        // Uniques can't be deleted or removed, we only visit components and custom storages.
        for storage in self.storages.get_mut().components_and_custom.values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
            unsafe { &mut *storage.0 }
                .get_mut()
                .clear_all_removed_and_deleted_older_than_timestamp(timestamp);
        }
    }

    /// Clear all insertion tracking data.
    #[track_caller]
    pub fn clear_all_inserted(&mut self) {
        let now = self.get_tracking_timestamp();

        // Uniques track insertion too, we visit both maps.
        for storage in self.storages.get_mut().values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
            unsafe { &mut *storage.0 }.get_mut().clear_all_inserted(now);
        }
    }

    /// Clear all modification tracking data.
    #[track_caller]
    pub fn clear_all_modified(&mut self) {
        let now = self.get_tracking_timestamp();

        // Uniques track modification too, we visit both maps.
        for storage in self.storages.get_mut().values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
            unsafe { &mut *storage.0 }.get_mut().clear_all_modified(now);
        }
    }

    /// Clear all insertion tracking data.
    #[track_caller]
    pub fn clear_all_inserted_and_modified(&mut self) {
        let now = self.get_tracking_timestamp();

        // Uniques track insertion and modification too, we visit both maps.
        for storage in self.storages.get_mut().values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
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

        self.exclusive_component_mut::<SparseSet<T>>()
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

        self.exclusive_component_mut::<SparseSet<T>>()
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

        let entity = self.entities.get_mut().generate();
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

        if self.entities.get_mut().is_alive(entity) {
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
[EntitiesView]: crate::EntitiesView
[EntitiesViewMut]: crate::EntitiesViewMut
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::borrow::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::borrow::NonSync")]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSendSync]: crate::borrow::NonSendSync"
    )]
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

[EntitiesView]: crate::EntitiesView
[EntitiesViewMut]: crate::EntitiesViewMut
[World]: crate::World
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::borrow::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::borrow::NonSync")]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSendSync]: crate::borrow::NonSendSync"
    )]
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
[EntitiesView]: crate::EntitiesView
[EntitiesViewMut]: crate::EntitiesViewMut
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::borrow::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::borrow::NonSync")]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSendSync]: crate::borrow::NonSendSync"
    )]
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
    #[inline]
    pub(crate) fn entities(&self) -> Result<ARef<'_, &'_ Entities>, error::GetStorage> {
        self.entities.borrow().map_err(error::GetStorage::Entities)
    }
    #[allow(clippy::mut_from_ref, reason = "Interior mutability")]
    #[inline]
    pub(crate) fn entities_mut(&self) -> Result<ARefMut<'_, &'_ mut Entities>, error::GetStorage> {
        self.entities
            .borrow_mut()
            .map_err(error::GetStorage::Entities)
    }

    /// Returns the `T` component storage.
    #[inline]
    pub(crate) fn exclusive_component_mut<T: 'static>(
        &mut self,
    ) -> Result<&mut T, error::GetStorage> {
        let Some(sbox) = self.storages.get_mut().component(StorageId::of::<T>()) else {
            return Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<T>()),
                id: StorageId::of::<T>(),
            });
        };

        // SAFE: `sbox` is in the registry and we hold `&mut self` for as long as the returned
        // reference lives, so nobody can remove the storage or borrow it in the meantime.
        let storage = unsafe { &mut *sbox.0 }.get_mut();

        Ok(storage.as_any_mut().downcast_mut().unwrap())
    }

    /// Returns the unique storage registered under `storage_id`, as a `T`.
    #[track_caller]
    pub(crate) fn exclusive_unique_mut_by_id<T: 'static>(
        &mut self,
        storage_id: StorageId,
    ) -> Result<&mut T, error::GetStorage> {
        let Some(sbox) = self.storages.get_mut().unique(storage_id) else {
            return Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<T>()),
                id: StorageId::of::<T>(),
            });
        };

        // SAFE: `sbox` is in the registry and we hold `&mut self` for as long as the returned
        // reference lives, so nobody can remove the storage or borrow it in the meantime.
        let storage = unsafe { &mut *sbox.0 }.get_mut();

        Ok(storage.as_any_mut().downcast_mut().unwrap())
    }

    /// Returns the `T` component or custom storage registered under `storage_id`,
    /// inserting `f()` if it doesn't exist.
    #[inline]
    pub(crate) fn exclusive_component_or_insert_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Send + Sync,
        F: FnOnce() -> T,
    {
        component_or_insert_with(
            &mut self.storages.get_mut().components_and_custom,
            storage_id,
            || SBox::new(f()),
        )
    }

    /// Returns the `!Send` `T` component storage registered under `storage_id`,
    /// inserting `f()` if it doesn't exist.
    #[cfg(feature = "thread_local")]
    #[track_caller]
    #[inline]
    pub(crate) fn exclusive_component_or_insert_non_send_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Sync,
        F: FnOnce() -> T,
    {
        component_or_insert_with(
            &mut self.storages.get_mut().components_and_custom,
            storage_id,
            || SBox::new_non_send(f(), self.thread_id_generator.clone()),
        )
    }

    /// Returns the `!Sync` `T` component storage registered under `storage_id`,
    /// inserting `f()` if it doesn't exist.
    #[cfg(feature = "thread_local")]
    #[inline]
    pub(crate) fn exclusive_component_or_insert_non_sync_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Send,
        F: FnOnce() -> T,
    {
        component_or_insert_with(
            &mut self.storages.get_mut().components_and_custom,
            storage_id,
            || SBox::new_non_sync(f()),
        )
    }

    /// Returns the `!Send + !Sync` `T` component storage registered under `storage_id`,
    /// inserting `f()` if it doesn't exist.
    #[cfg(feature = "thread_local")]
    #[track_caller]
    #[inline]
    pub(crate) fn exclusive_component_or_insert_non_send_sync_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage,
        F: FnOnce() -> T,
    {
        component_or_insert_with(
            &mut self.storages.get_mut().components_and_custom,
            storage_id,
            || SBox::new_non_send_sync(f(), self.thread_id_generator.clone()),
        )
    }

    /// Make the given entity alive.  
    /// Does nothing if an entity with a greater generation is already at this index.  
    /// Returns `true` if the entity is successfully spawned.
    #[inline]
    pub fn spawn(&mut self, entity: EntityId) -> bool {
        self.entities.get_mut().spawn(entity)
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
        doc = "[NonSend]: crate::borrow::NonSend
[NonSync]: crate::borrow::NonSync
[NonSendSync]: crate::borrow::NonSendSync"
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
        doc = "[NonSend]: crate::borrow::NonSend
[NonSync]: crate::borrow::NonSync
[NonSendSync]: crate::borrow::NonSendSync"
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
        doc = "[NonSend]: crate::borrow::NonSend
[NonSync]: crate::borrow::NonSync
[NonSendSync]: crate::borrow::NonSendSync"
    )]
    #[inline]
    #[track_caller]
    pub fn iter<'a, T: IterComponent>(&'a self) -> IntoIterRef<'a, T>
    where
        <T as IterComponent>::Shiperator<'a>: Shiperator,
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
    #[inline]
    pub fn is_entity_alive(&mut self, entity: EntityId) -> bool {
        self.entities.get_mut().is_alive(entity)
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

        if !self.entities.get_mut().delete_unchecked(entity) {
            panic!(
                "Entity {:?} has to be alive to move it to another World.",
                entity
            );
        };

        assert!(
            other.entities.get_mut().spawn(entity),
            "Other World already has an entity at {:?}'s index.",
            entity
        );

        // Uniques don't hold per entity data, we only visit components and custom storages.
        for storage in self.storages.get_mut().components_and_custom.values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
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

        if !self.entities.get_mut().is_alive(from) {
            panic!(
                "Entity {:?} has to be alive to move its components to another World.",
                from
            );
        };

        if !other.entities.get_mut().is_alive(to) {
            panic!(
                "Entity {:?} has to be alive to receive components from another World.",
                to
            );
        };

        // Uniques don't hold per entity data, we only visit components and custom storages.
        for storage in self.storages.get_mut().components_and_custom.values_mut() {
            // SAFE: `storage` is in the registry and we hold `&mut self`, nobody else can reach it.
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
    /// Tracking is not cloned. Components will count as inserted in `other`.\
    /// `other` keeps its own groups, copied components only move to them at `other`'s next regroup.
    #[track_caller]
    pub fn clone_storages_to(&self, other: &mut AllStorages) {
        let other_current = other.get_current();

        // The entities aren't in the registry, their clone replaces `other`'s like any other storage.
        // It doesn't come with the deletion callback, `other`'s is dropped with the entities it replaces.
        *other.entities.get_mut() = self.entities.borrow().unwrap().clone_without_on_deletion();

        let other_storages = other.storages.get_mut();

        // We need to know which map each storage comes from, its clone goes in the same one.
        let source = self.storages.read();
        let source_storages = source
            .components_and_custom
            .iter()
            .map(|entry| (entry, false))
            .chain(source.uniques.iter().map(|entry| (entry, true)));

        for ((storage_id, storage), is_unique) in source_storages {
            // SAFE: `storage` is in our registry and we hold its lock for the whole loop, so it
            // points to a live boxed `AtomicRefCell`. We only borrow it through its runtime checks.
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
                let mut storage = {
                    #[cfg(not(feature = "thread_local"))]
                    {
                        storage_builder.build()
                    }
                    #[cfg(feature = "thread_local")]
                    {
                        let storage_type_id = storage_borrow.any().type_id();
                        // SAFE: `storage_builder` owns the clone's box and keeps it alive, nobody
                        // else can reach it yet and we only borrow it through its runtime checks.
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

                // The clone keeps the original's registration, a unique stays with the uniques
                // and everything else with the components and custom storages, whatever the
                // clone's type. An id is only ever in one map, so a storage under the same id in
                // `other`'s other map is replaced too.
                let (target, opposite) = if is_unique {
                    (
                        &mut other_storages.uniques,
                        &mut other_storages.components_and_custom,
                    )
                } else {
                    (
                        &mut other_storages.components_and_custom,
                        &mut other_storages.uniques,
                    )
                };
                let mut displaced = opposite.remove(storage_id);

                // The destination's groups stay, peers still list this storage as a member.
                // Nothing here runs user code, user values were cloned already and the
                // replaced storage is only dropped once the replacement is in place.
                if let Some(destination) = target.get_mut(storage_id).or(displaced.as_mut()) {
                    adopt_groups(
                        destination.storage_mut(),
                        storage.storage_mut(),
                        *storage_id,
                    );
                }

                target.insert(*storage_id, storage);
                drop(displaced);
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
            other_all_storages.entities.get_mut().spawn(entity),
            "Other World already has an entity at {:?}'s index.",
            entity
        );

        // Uniques don't hold per entity data, we only visit components and custom storages.
        // A borrowed unique doesn't get in the way.
        let storages = self.storages.read();
        for storage in storages.components_and_custom.values() {
            // SAFE: `storage` is in the registry and `storages` is still locked.
            unsafe { &*storage.0 }.borrow().unwrap().clone_component_to(
                other_all_storages,
                entity,
                entity,
                other_current,
            );
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

        if !other_all_storages.entities.get_mut().is_alive(to) {
            panic!(
                "Entity {:?} has to be alive to receive components from another World.",
                to
            );
        };

        // Uniques don't hold per entity data, we only visit components and custom storages.
        // A borrowed unique doesn't get in the way.
        let storages = self.storages.read();
        for storage in storages.components_and_custom.values() {
            // SAFE: `storage` is in the registry and `storages` is still locked.
            unsafe { &*storage.0 }.borrow().unwrap().clone_component_to(
                other_all_storages,
                from,
                to,
                other_current,
            );
        }
    }
}

/// Takes the storage out of `sbox` and frees its allocation.
///
/// ### Safety
///
/// `sbox` must hold an `S`.
unsafe fn take_storage<S>(sbox: SBox) -> S {
    let ptr = sbox.0 as *mut AtomicRefCell<S>;
    // `sbox` would free the allocation as a `dyn Storage`, we free it as an `S` below instead.
    core::mem::forget(sbox);

    // SAFE: `ptr` comes from `Box::into_raw` of an `AtomicRefCell` holding an `S`, as our caller
    // guarantees, so it's an `AtomicRefCell<S>` allocation. We forgot `sbox`, this box is its only owner.
    let cell = unsafe { Box::from_raw(ptr) };

    cell.into_inner()
}

/// Returns the `T` component or custom storage registered under `storage_id` in
/// `components_and_custom`, inserting `new_storage()` if it doesn't exist.
///
/// Takes the map instead of `&mut AllStorages` so callers can build `new_storage` from other
/// `AllStorages` fields.
#[inline]
fn component_or_insert_with<T: 'static>(
    storages: &mut ShipHashMap<StorageId, SBox>,
    storage_id: StorageId,
    new_storage: impl FnOnce() -> SBox,
) -> &mut T {
    // We only keep the pointer out of the lookup, so the miss branch can insert.
    let storage = match storages.get(&storage_id) {
        Some(sbox) => sbox.0,
        None => storages.entry(storage_id).or_insert_with(new_storage).0,
    };

    // SAFE: `storage` comes from an `SBox` in the registry, found or just inserted, so it points
    // to a live boxed `AtomicRefCell`. We hold the registry exclusively for as long as the returned
    // reference lives, so nobody can remove the storage or borrow it in the meantime.
    unsafe { &mut *storage }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

/// Returns `T`'s unique storage, looking through the thread-local wrapper it might be stored in.
///
/// Returns `None` if `storage` isn't one of `T`'s unique storages.
fn unique_storage_mut<T: Unique>(storage: &mut dyn Storage) -> Option<&mut UniqueStorage<T>> {
    // `SizedAny` is implemented for every type, unlike `Storage::any_mut` custom storages can't
    // override it. So this check can't run user code and always sees the real type.
    let storage = storage.as_any_mut();

    if storage.is::<UniqueStorage<T>>() {
        return storage.downcast_mut();
    }

    #[cfg(feature = "thread_local")]
    {
        if storage.is::<NonSend<UniqueStorage<T>>>() {
            return Some(&mut storage.downcast_mut::<NonSend<UniqueStorage<T>>>()?.0);
        }
        if storage.is::<NonSync<UniqueStorage<T>>>() {
            return Some(&mut storage.downcast_mut::<NonSync<UniqueStorage<T>>>()?.0);
        }
        if storage.is::<NonSendSync<UniqueStorage<T>>>() {
            return Some(&mut storage.downcast_mut::<NonSendSync<UniqueStorage<T>>>()?.0);
        }
    }

    None
}

/// Returns the grouping metadata of the storage stored under `storage_id`.
///
/// ### Panics
///
/// - The storage doesn't exist.
/// - The storage doesn't support grouping.
#[track_caller]
fn member_groups(
    storages: &mut ShipHashMap<StorageId, SBox>,
    storage_id: StorageId,
) -> &mut Groups {
    let sbox = storages
        .get_mut(&storage_id)
        .expect("group members are created before being grouped");

    // SAFE: `sbox.0` comes from a `Box` that lives as long as its entry in the map,
    // and we borrow the map mutably for the whole lifetime of the returned reference,
    // so no other reference to this storage can exist.
    unsafe { &mut *sbox.0 }
        .get_mut()
        .groups_mut()
        .expect("grouping only supports SparseSet storages")
}

impl core::fmt::Debug for AllStorages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("AllStorages");

        let storages = self.storages.read();

        // The entities aren't in the registry, they count as one more storage.
        debug_struct.field("storage_count", &(storages.len() + 1));
        debug_struct.field(
            "storages",
            &StorageNames {
                entities: &self.entities,
                registry: &storages,
            },
        );

        debug_struct.finish()
    }
}

/// Lists the name of every storage, the entities first.
///
/// Each storage is shown like `SBox` shows it: its name, or a note when it's borrowed exclusively.
struct StorageNames<'a> {
    entities: &'a AtomicRefCell<Entities>,
    registry: &'a StorageMaps,
}

impl core::fmt::Debug for StorageNames<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let entities = match self.entities.borrow() {
            Ok(entities) => entities.name(),
            Err(_) => "Could not borrow storage".into(),
        };

        f.debug_list()
            .entry(&format_args!("{entities}"))
            .entries(self.registry.values())
            .finish()
    }
}

impl core::fmt::Debug for AllStoragesMemoryUsage<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut borrowed_storages = 0;

        let mut debug_struct = f.debug_list();

        // The entities aren't in the registry, we report them first.
        match self.0.entities.borrow() {
            Ok(entities) => {
                debug_struct.entries(entities.memory_usage());
            }
            Err(_) => borrowed_storages += 1,
        }

        let storages = self.0.storages.read();

        debug_struct.entries(storages.values().filter_map(|storage| {
            // SAFE: `storage` is in the registry and `storages` is still locked.
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

#[cfg(feature = "serde1")]
impl AllStorages {
    /// Serializes the view using the provided serializer.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, View, World};
    ///
    /// #[derive(Component, serde::Serialize)]
    /// struct Name(String);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let eid1 = all_storages.add_entity(Name("Alice".to_string()));
    ///
    /// let mut serialized = Vec::new();
    /// all_storages
    ///     .serialize::<_, View<Name>>(&mut serde_json::ser::Serializer::new(&mut serialized))
    ///     .unwrap_or_else(|_| panic!());
    ///
    /// let serialized_str = String::from_utf8(serialized).unwrap();
    /// assert_eq!(serialized_str, r#"[[{"index":0,"gen":0},"Alice"]]"#);
    /// ```
    pub fn serialize<'a, S: serde::Serializer, V: Borrow>(
        &'a self,
        serializer: S,
    ) -> Result<S::Ok, error::Serialize<S>>
    where
        V::View<'a>: serde::Serialize,
    {
        use serde::Serialize;

        match self.borrow::<V>() {
            Ok(view) => match view.serialize(serializer) {
                Ok(ok) => Ok(ok),
                Err(err) => Err(error::Serialize::Serialization(err)),
            },
            Err(err) => Err(error::Serialize::Borrow(err)),
        }
    }

    /// Deserializes the view using the provided deserializer.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Component, EntityId, ViewMut, World};
    ///
    /// #[derive(Component, serde::Deserialize)]
    /// struct Name(String);
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let mut serialized = r#"[[{"index":0,"gen":0},"Alice"]]"#;
    /// all_storages
    ///     .deserialize::<_, ViewMut<Name>>(&mut serde_json::de::Deserializer::from_str(serialized))
    ///     .unwrap_or_else(|_| panic!());
    ///
    /// let alice_eid = EntityId::new_from_index_and_gen(0, 0);
    /// assert_eq!(all_storages.get::<&Name>(alice_eid).unwrap().0, "Alice");
    ///
    /// // Careful here, the World is not in a stable state
    ///
    /// assert_eq!(all_storages.is_entity_alive(alice_eid), false);
    ///
    /// // We can use World::spawn for example to fix the problem
    /// // another solution would be to serialize EntitiesViewMut
    ///
    /// all_storages.spawn(alice_eid);
    ///
    /// assert_eq!(all_storages.is_entity_alive(alice_eid), true);
    /// ```
    pub fn deserialize<'a, 'de, D: serde::Deserializer<'de>, V: Borrow>(
        &'a self,
        deserializer: D,
    ) -> Result<(), error::Deserialize<'de, D>>
    where
        V::View<'a>: serde::Deserialize<'de>,
    {
        match self.borrow::<V>() {
            Ok(mut view) => match serde::Deserialize::deserialize_in_place(deserializer, &mut view)
            {
                Ok(ok) => Ok(ok),
                Err(err) => Err(error::Deserialize::Deserialization(err)),
            },
            Err(err) => Err(error::Deserialize::Borrow(err)),
        }
    }
}
