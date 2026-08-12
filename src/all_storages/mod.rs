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
use crate::sparse_set::{
    BulkAddEntity, SparseArray, SparseSet, TupleAddComponent, TupleDelete, TupleRemove, BUCKET_SIZE,
};
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
use alloc::vec;
use alloc::vec::Vec;
use core::any::{type_name, TypeId};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use hashbrown::hash_map::Entry;

#[derive(Default)]
struct PendingPageAccumulator {
    masks: Vec<u32>,
    active_pages: Vec<usize>,
}

impl PendingPageAccumulator {
    #[inline]
    fn union_page(&mut self, page_index: usize, mask: u32) {
        if mask == 0 {
            return;
        }

        if page_index >= self.masks.len() {
            self.masks.resize(page_index + 1, 0);
        }

        let aggregate = &mut self.masks[page_index];
        if *aggregate == 0 {
            self.active_pages.push(page_index);
        }
        *aggregate |= mask;
    }

    fn append_live_entities(&mut self, entities: &Entities, out: &mut Vec<EntityId>) {
        debug_assert_eq!(BUCKET_SIZE, u32::BITS as usize);

        self.active_pages.sort_unstable();

        for page_index in self.active_pages.drain(..) {
            let mut mask = core::mem::take(&mut self.masks[page_index]);

            while mask != 0 {
                let bit_index = mask.trailing_zeros() as usize;
                mask &= mask - 1;

                let entity_index = page_index * BUCKET_SIZE + bit_index;
                if let Some(&entity) = entities.data.get(entity_index) {
                    if entity.uindex() == entity_index {
                        out.push(entity);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
enum StorageMask {
    Inline(u64),
    Heap(Box<[u64]>),
}

impl StorageMask {
    fn new(storage_count: usize) -> Self {
        if storage_count <= u64::BITS as usize {
            Self::Inline(0)
        } else {
            Self::Heap(vec![0; storage_count.div_ceil(u64::BITS as usize)].into_boxed_slice())
        }
    }

    fn zero_like(other: &Self) -> Self {
        match other {
            Self::Inline(_) => Self::Inline(0),
            Self::Heap(words) => Self::Heap(vec![0; words.len()].into_boxed_slice()),
        }
    }

    #[inline]
    fn words(&self) -> &[u64] {
        match self {
            Self::Inline(word) => core::slice::from_ref(word),
            Self::Heap(words) => words,
        }
    }

    #[inline]
    fn words_mut(&mut self) -> &mut [u64] {
        match self {
            Self::Inline(word) => core::slice::from_mut(word),
            Self::Heap(words) => words,
        }
    }

    #[inline]
    fn insert(&mut self, storage_index: usize) -> bool {
        let word_index = storage_index / u64::BITS as usize;
        let bit = 1u64 << (storage_index % u64::BITS as usize);
        let word = &mut self.words_mut()[word_index];
        let was_missing = *word & bit == 0;
        *word |= bit;
        was_missing
    }

    #[inline]
    fn intersects(&self, other: &Self) -> bool {
        debug_assert_eq!(self.words().len(), other.words().len());
        self.words()
            .iter()
            .zip(other.words())
            .any(|(&left, &right)| left & right != 0)
    }

    fn union_with(&mut self, other: &Self) {
        debug_assert_eq!(self.words().len(), other.words().len());
        for (word, &other_word) in self.words_mut().iter_mut().zip(other.words()) {
            *word |= other_word;
        }
    }

    fn intersect_with(&mut self, other: &Self) {
        debug_assert_eq!(self.words().len(), other.words().len());
        for (word, &other_word) in self.words_mut().iter_mut().zip(other.words()) {
            *word &= other_word;
        }
    }

    fn difference_with(&mut self, other: &Self) {
        debug_assert_eq!(self.words().len(), other.words().len());
        for (word, &other_word) in self.words_mut().iter_mut().zip(other.words()) {
            *word &= !other_word;
        }
    }

    #[inline]
    fn clear(&mut self) {
        self.words_mut().fill(0);
    }

    fn first_difference(&self, other: &Self) -> Option<usize> {
        debug_assert_eq!(self.words().len(), other.words().len());
        self.words().iter().zip(other.words()).enumerate().find_map(
            |(word_index, (&word, &other_word))| {
                let difference = word & !other_word;

                (difference != 0)
                    .then(|| word_index * u64::BITS as usize + difference.trailing_zeros() as usize)
            },
        )
    }

    fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.words()
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(word_index, mut word)| {
                core::iter::from_fn(move || {
                    if word == 0 {
                        return None;
                    }

                    let bit_index = word.trailing_zeros() as usize;
                    word &= word - 1;

                    Some(word_index * u64::BITS as usize + bit_index)
                })
            })
    }

    fn count_ones(&self) -> usize {
        self.words()
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    #[inline]
    fn is_subset_of(&self, other: &Self) -> bool {
        debug_assert_eq!(self.words().len(), other.words().len());
        self.words()
            .iter()
            .zip(other.words())
            .all(|(&left, &right)| left & !right == 0)
    }
}

struct RegroupCandidate {
    storages: StorageMask,
    expanded_storages: StorageMask,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct RegroupBatchKey {
    moved_storages: StorageMask,
    target_group: StorageMask,
}

struct RemovedGroupBatch {
    type_ids: Vec<TypeId>,
    entities: Vec<EntityId>,
}

type InvalidatedGroups = ShipHashMap<EntityId, StorageMask>;

struct RegroupBatch {
    /// Dense indices into `participating_storages`.
    storage_indices: Vec<usize>,
    /// Sorted storage `TypeId`s passed to each storage as its group signature.
    type_ids: Vec<TypeId>,
    /// Entities that resolved to this exact signature.
    entities: Vec<EntityId>,
}

impl RegroupCandidate {
    fn new(storages: StorageMask) -> Self {
        Self {
            expanded_storages: StorageMask::zero_like(&storages),
            storages,
        }
    }

    fn merge(&mut self, other: RegroupCandidate) {
        self.storages.union_with(&other.storages);
        self.expanded_storages.union_with(&other.expanded_storages);
    }
}

fn insert_regroup_candidate(
    candidates: &mut Vec<RegroupCandidate>,
    mut candidate: RegroupCandidate,
) {
    while let Some(index) = candidates
        .iter()
        .position(|existing| existing.storages.intersects(&candidate.storages))
    {
        candidate.merge(candidates.swap_remove(index));
    }

    candidates.push(candidate);
}

#[inline]
fn seed_regroup_candidates_direct(
    entity: EntityId,
    group_masks: &[StorageMask],
    sparse_arrays: &[Option<&SparseArray>],
    candidates: &mut Vec<RegroupCandidate>,
) {
    for group_mask in group_masks {
        let is_complete = group_mask.indices().all(|storage_index| {
            sparse_arrays[storage_index]
                .map(|sparse| sparse.contains(entity))
                .unwrap_or(false)
        });

        if is_complete {
            insert_regroup_candidate(candidates, RegroupCandidate::new(group_mask.clone()));
        }
    }
}

#[inline]
fn seed_regroup_candidates_from_presence(
    entity: EntityId,
    group_masks: &[StorageMask],
    sparse_arrays: &[Option<&SparseArray>],
    presence_mask: &mut StorageMask,
    candidates: &mut Vec<RegroupCandidate>,
) {
    presence_mask.clear();

    for (storage_index, sparse_array) in sparse_arrays.iter().enumerate() {
        if sparse_array
            .map(|sparse| sparse.contains(entity))
            .unwrap_or(false)
        {
            presence_mask.insert(storage_index);
        }
    }

    for group_mask in group_masks {
        if group_mask.is_subset_of(presence_mask) {
            insert_regroup_candidate(candidates, RegroupCandidate::new(group_mask.clone()));
        }
    }
}

#[cfg(test)]
fn regroup_candidate_capacities(
    candidates: &Vec<RegroupCandidate>,
    merged_candidates: &Vec<RegroupCandidate>,
) -> (usize, usize) {
    (candidates.capacity(), merged_candidates.capacity())
}

#[allow(missing_docs)]
pub struct MissingLock;
#[allow(missing_docs)]
pub struct LockPresent;
#[allow(missing_docs)]
pub struct MissingThreadId;
#[allow(missing_docs)]
pub struct ThreadIdPresent;

/// Contains all storages present in the [`World`](crate::world::World).
// The lock is held very briefly:
// - shared: when trying to find a storage
// - unique: when adding a storage
// once the storage is found or created the lock is released
// this is safe since World is still borrowed and there is no way to delete a storage
// so any access to storages are valid as long as the World exists
// we use a HashMap, it can reallocate, but even in this case the storages won't move since they are boxed
pub struct AllStorages {
    pub(crate) storages: RwLock<ShipHashMap<StorageId, SBox>>,
    mutably_borrowed_since_regroup: AtomicBool,
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
            mutably_borrowed_since_regroup: AtomicBool::new(false),
            #[cfg(feature = "thread_local")]
            main_thread_id: (std_thread_id_generator)(),
            #[cfg(feature = "thread_local")]
            thread_id_generator: Arc::new(std_thread_id_generator),
            counter,
        }
    }

    #[inline]
    fn mark_mutably_borrowed(&self) {
        self.mutably_borrowed_since_regroup
            .store(true, Ordering::Release);
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
    /// [NonSend]: crate::borrow::NonSend
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
    /// [NonSync]: crate::borrow::NonSync
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
    /// [NonSync]: crate::borrow::NonSync
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
            for storage in all_storages.storages.get_mut().values_mut() {
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

            all_storages
                .storages
                .get_mut()
                .par_iter_mut()
                .for_each(|(_, storage)| {
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
            Ok(storage) => {
                self.mark_mutably_borrowed();
                Ok(ARefMut::map(storage, |storage| {
                    storage.as_any_mut().downcast_mut().unwrap()
                }))
            }
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
            self.mutably_borrowed_since_regroup
                .store(true, Ordering::Release);
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

        let storage = unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new(f()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap();

        self.mutably_borrowed_since_regroup
            .store(true, Ordering::Release);
        storage
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

        let storage = unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_send(f(), self.thread_id_generator.clone()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap();

        self.mutably_borrowed_since_regroup
            .store(true, Ordering::Release);
        storage
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

        let storage = unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_sync(f()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap();

        self.mutably_borrowed_since_regroup
            .store(true, Ordering::Release);
        storage
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

        let storage = unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_send_sync(f(), self.thread_id_generator.clone()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap();

        self.mutably_borrowed_since_regroup
            .store(true, Ordering::Release);
        storage
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
        TrackingTimestamp::new(self.counter.fetch_add(1, Ordering::Acquire))
    }

    /// Returns a timestamp used to clear tracking information.
    pub fn get_tracking_timestamp(&self) -> TrackingTimestamp {
        TrackingTimestamp::new(self.counter.load(Ordering::Acquire))
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

    /// Regroups pending components into complete, overlapping storage groups.
    pub fn regroup(&mut self) {
        if !self.mutably_borrowed_since_regroup.load(Ordering::Acquire) {
            return;
        }

        let storage_map = self.storages.get_mut();
        let mut pending_pages = PendingPageAccumulator::default();
        let mut pending_entities = Vec::new();
        let mut group_definitions = Vec::new();
        let mut removed_group_batches = Vec::new();

        for storage in storage_map.values_mut() {
            let storage = unsafe { &mut *storage.0 }.get_mut();

            storage.collect_regroup_pages(
                &mut |page_index, mask| pending_pages.union_page(page_index, mask),
                &mut |group| {
                    let mut group = group.to_vec();
                    group.sort_unstable();
                    group.dedup();

                    if !group.is_empty() {
                        group_definitions.push(group);
                    }
                },
            );

            storage.collect_regroup_removals(&mut |entity, group| {
                if let Some(batch) =
                    removed_group_batches
                        .iter_mut()
                        .find(|batch: &&mut RemovedGroupBatch| {
                            batch.type_ids.iter().all(|type_id| group.contains(type_id))
                                && group.iter().all(|type_id| batch.type_ids.contains(type_id))
                        })
                {
                    batch.entities.push(entity);
                } else {
                    let mut type_ids = group.to_vec();
                    type_ids.sort_unstable();
                    type_ids.dedup();
                    removed_group_batches.push(RemovedGroupBatch {
                        type_ids,
                        entities: vec![entity],
                    });
                }
            });
        }

        let entities = storage_map
            .get_mut(&StorageId::of::<Entities>())
            .expect("Entities storage must always exist");
        let entities = unsafe { &mut *entities.0 }
            .get_mut()
            .as_any()
            .downcast_ref::<Entities>()
            .expect("Entities storage must have the Entities type");
        pending_pages.append_live_entities(entities, &mut pending_entities);

        for batch in &mut removed_group_batches {
            batch.entities.retain(|&entity| {
                if entities.is_alive(entity) {
                    pending_entities.push(entity);
                    true
                } else {
                    false
                }
            });
        }

        pending_entities.sort_unstable();
        pending_entities.dedup();

        group_definitions.sort_unstable();
        group_definitions.dedup();

        if pending_entities.is_empty() || group_definitions.is_empty() {
            self.mutably_borrowed_since_regroup
                .store(false, Ordering::Release);
            return;
        }

        let mut participating_storages: Vec<_> = group_definitions
            .iter()
            .flat_map(|group| group.iter().copied())
            .collect();
        participating_storages.sort_unstable();
        participating_storages.dedup();

        let storage_count = participating_storages.len();
        let storage_indices: ShipHashMap<_, _> = participating_storages
            .iter()
            .copied()
            .enumerate()
            .map(|(index, storage_id)| (storage_id, index))
            .collect();
        let group_masks: Vec<_> = group_definitions
            .iter()
            .map(|group| {
                let mut mask = StorageMask::new(storage_count);

                for storage_id in group {
                    mask.insert(storage_indices[storage_id]);
                }

                mask
            })
            .collect();

        let mut invalidated_groups: InvalidatedGroups = ShipHashMap::new();
        for batch in &removed_group_batches {
            let mut invalidated = StorageMask::new(storage_count);

            for type_id in &batch.type_ids {
                if let Some(&storage_index) = storage_indices.get(type_id) {
                    invalidated.insert(storage_index);
                }
            }

            for &entity in &batch.entities {
                invalidated_groups
                    .entry(entity)
                    .and_modify(|groups| groups.union_with(&invalidated))
                    .or_insert_with(|| invalidated.clone());
            }
        }

        let membership_edge_count: usize = group_masks.iter().map(StorageMask::count_ones).sum();
        let build_presence_mask = membership_edge_count > storage_count;

        let storage_guards: Vec<_> = participating_storages
            .iter()
            .map(|&storage_id| {
                storage_map
                    .get(&StorageId::from(storage_id))
                    .map(|storage| unsafe { &*storage.0 }.borrow().unwrap())
            })
            .collect();
        let sparse_arrays: Vec<_> = storage_guards
            .iter()
            .map(|guard| guard.as_ref().and_then(|storage| storage.sparse_array()))
            .collect();

        let mut signature_indices: ShipHashMap<RegroupBatchKey, usize> = ShipHashMap::new();
        let mut batches = Vec::new();
        let mut candidates = Vec::new();
        let mut merged_candidates = Vec::new();
        let mut presence_mask = build_presence_mask.then(|| StorageMask::new(storage_count));

        for entity in pending_entities {
            candidates.clear();
            merged_candidates.clear();

            if let Some(presence_mask) = presence_mask.as_mut() {
                seed_regroup_candidates_from_presence(
                    entity,
                    &group_masks,
                    &sparse_arrays,
                    presence_mask,
                    &mut candidates,
                );
            } else {
                seed_regroup_candidates_direct(
                    entity,
                    &group_masks,
                    &sparse_arrays,
                    &mut candidates,
                );
            }

            let mut containing_storages = StorageMask::new(storage_count);
            if let Some(presence_mask) = presence_mask.as_ref() {
                containing_storages.union_with(presence_mask);
            } else {
                for (storage_index, sparse_array) in sparse_arrays.iter().enumerate() {
                    if sparse_array
                        .map(|sparse| sparse.contains(entity))
                        .unwrap_or(false)
                    {
                        containing_storages.insert(storage_index);
                    }
                }
            }

            for candidate in &mut candidates {
                while let Some(storage_index) = candidate
                    .storages
                    .first_difference(&candidate.expanded_storages)
                {
                    candidate.expanded_storages.insert(storage_index);

                    if let Some(current_group) = storage_guards[storage_index]
                        .as_ref()
                        .and_then(|storage| storage.entity_group(entity))
                    {
                        for storage_id in current_group {
                            if let Some(&current_storage_index) = storage_indices.get(storage_id) {
                                if sparse_arrays[current_storage_index]
                                    .map(|sparse| sparse.contains(entity))
                                    .unwrap_or(false)
                                {
                                    candidate.storages.insert(current_storage_index);
                                }
                            }
                        }
                    }
                }

                debug_assert!(candidate
                    .expanded_storages
                    .is_subset_of(&candidate.storages));
                debug_assert_eq!(
                    candidate.expanded_storages.count_ones(),
                    candidate.storages.count_ones()
                );
            }

            for candidate in candidates.drain(..) {
                insert_regroup_candidate(&mut merged_candidates, candidate);
            }

            let mut assigned_storages = StorageMask::new(storage_count);
            for candidate in merged_candidates.drain(..) {
                assigned_storages.union_with(&candidate.storages);

                let key = RegroupBatchKey {
                    moved_storages: candidate.storages.clone(),
                    target_group: candidate.storages,
                };
                let signature_index = if let Some(&signature_index) = signature_indices.get(&key) {
                    signature_index
                } else {
                    let signature_index = batches.len();
                    let storage_indices: Vec<_> = key.moved_storages.indices().collect();
                    let type_ids = key
                        .target_group
                        .indices()
                        .map(|storage_index| participating_storages[storage_index])
                        .collect();

                    batches.push(RegroupBatch {
                        storage_indices,
                        type_ids,
                        entities: Vec::new(),
                    });
                    signature_indices.insert(key, signature_index);
                    signature_index
                };

                let batch = &mut batches[signature_index];
                debug_assert_ne!(batch.entities.last(), Some(&entity));
                batch.entities.push(entity);
            }

            if let Some(invalidated) = invalidated_groups.get(&entity) {
                let mut moved_to_unclassified = invalidated.clone();
                moved_to_unclassified.intersect_with(&containing_storages);
                moved_to_unclassified.difference_with(&assigned_storages);

                if moved_to_unclassified.count_ones() != 0 {
                    let key = RegroupBatchKey {
                        moved_storages: moved_to_unclassified,
                        target_group: StorageMask::new(storage_count),
                    };
                    let signature_index =
                        if let Some(&signature_index) = signature_indices.get(&key) {
                            signature_index
                        } else {
                            let signature_index = batches.len();
                            let storage_indices: Vec<_> = key.moved_storages.indices().collect();

                            batches.push(RegroupBatch {
                                storage_indices,
                                type_ids: Vec::new(),
                                entities: Vec::new(),
                            });
                            signature_indices.insert(key, signature_index);
                            signature_index
                        };

                    let batch = &mut batches[signature_index];
                    debug_assert_ne!(batch.entities.last(), Some(&entity));
                    batch.entities.push(entity);
                }
            }
        }

        drop(sparse_arrays);
        drop(storage_guards);

        for batch in batches {
            debug_assert!(!batch.entities.is_empty());

            for storage_index in batch.storage_indices {
                let storage_id = participating_storages[storage_index];
                let storage = storage_map
                    .get_mut(&StorageId::from(storage_id))
                    .expect("Regroup batches must reference existing storages.");
                let storage = unsafe { &mut *storage.0 }.get_mut();

                storage.move_to_group_batch(&batch.entities, &batch.type_ids);
            }
        }

        self.mutably_borrowed_since_regroup
            .store(false, Ordering::Release);
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

#[cfg(test)]
mod regroup_tests {
    use super::*;
    use crate::all_storages::CustomStorageAccess;
    use crate::component::Component;
    use crate::sparse_set::SparseArray;
    use crate::track;
    use crate::{View, ViewMut, World};
    use core::hash::{Hash, Hasher};

    #[cfg(feature = "std")]
    mod allocation_counter {
        use core::cell::Cell;
        use std::alloc::{GlobalAlloc, Layout, System};

        struct CountingAllocator;

        std::thread_local! {
            static ENABLED: Cell<bool> = const { Cell::new(false) };
            static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
        }

        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                record_allocation();
                unsafe { System.alloc(layout) }
            }

            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                record_allocation();
                unsafe { System.alloc_zeroed(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                unsafe { System.dealloc(ptr, layout) }
            }

            unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
                record_allocation();
                unsafe { System.realloc(ptr, layout, new_size) }
            }
        }

        #[global_allocator]
        static ALLOCATOR: CountingAllocator = CountingAllocator;

        fn record_allocation() {
            let enabled = ENABLED.try_with(Cell::get).unwrap_or(false);
            if enabled {
                let _ = ALLOCATIONS.try_with(|allocations| allocations.set(allocations.get() + 1));
            }
        }

        pub(super) fn count(f: impl FnOnce()) -> usize {
            ALLOCATIONS.with(|allocations| allocations.set(0));
            ENABLED.with(|enabled| enabled.set(true));
            f();
            ENABLED.with(|enabled| enabled.set(false));
            ALLOCATIONS.with(Cell::get)
        }
    }

    fn mask(storage_count: usize, indices: &[usize]) -> StorageMask {
        let mut mask = StorageMask::new(storage_count);
        for &index in indices {
            assert!(mask.insert(index));
        }
        mask
    }

    #[test]
    fn pending_page_accumulator_unions_pages_and_emits_sorted_unique_entities() {
        let mut entities = Entities::new();
        let live_entities = entities.bulk_generate(3 * BUCKET_SIZE).to_vec();
        let mut pending_pages = PendingPageAccumulator::default();

        pending_pages.union_page(2, (1_u32 << 5) | (1_u32 << 31));
        pending_pages.union_page(0, 1_u32 << 31);
        pending_pages.union_page(2, (1_u32 << 0) | (1_u32 << 5));
        pending_pages.union_page(1, 0);

        let mut emitted = Vec::new();
        pending_pages.append_live_entities(&entities, &mut emitted);

        assert_eq!(
            emitted,
            [
                live_entities[31],
                live_entities[64],
                live_entities[69],
                live_entities[95],
            ]
        );
        assert!(pending_pages.active_pages.is_empty());
        assert_eq!(pending_pages.masks, [0, 0, 0]);
    }

    #[test]
    fn pending_page_accumulator_ignores_deleted_entities_and_uses_current_generation() {
        let mut entities = Entities::new();
        let deleted = entities.generate();
        let mut pending_pages = PendingPageAccumulator::default();
        pending_pages.union_page(0, 1);

        assert!(entities.delete_unchecked(deleted));

        let mut emitted = Vec::new();
        pending_pages.append_live_entities(&entities, &mut emitted);
        assert!(emitted.is_empty());

        let recycled = entities.generate();
        assert_eq!(recycled.index(), deleted.index());
        assert_ne!(recycled.gen(), deleted.gen());

        pending_pages.union_page(0, 1);
        pending_pages.append_live_entities(&entities, &mut emitted);
        assert_eq!(emitted, [recycled]);
    }

    #[test]
    fn storage_mask_inline_boundaries() {
        for storage_count in [0, 1, u64::BITS as usize - 1, u64::BITS as usize] {
            let mask = StorageMask::new(storage_count);
            assert!(matches!(&mask, StorageMask::Inline(0)));
            assert_eq!(mask.words(), &[0]);
        }
    }

    #[test]
    fn storage_mask_heap_boundaries() {
        for (storage_count, word_count) in [
            (u64::BITS as usize + 1, 2),
            (2 * u64::BITS as usize, 2),
            (2 * u64::BITS as usize + 1, 3),
        ] {
            let mask = StorageMask::new(storage_count);
            assert!(matches!(&mask, StorageMask::Heap(_)));
            assert_eq!(mask.words(), vec![0; word_count]);
        }
    }

    #[test]
    fn storage_mask_cross_word_operations() {
        let storage_count = 3 * u64::BITS as usize + 1;
        let boundary_indices = [0, 63, 64, 127, 128, 191, 192];
        let mut left = mask(storage_count, &boundary_indices);
        assert!(!left.insert(64));
        assert_eq!(left.count_ones(), boundary_indices.len());

        let intersecting = mask(storage_count, &[63, 65, 127, 129, 192]);
        let disjoint = mask(storage_count, &[1, 62, 66, 126, 130, 190]);
        assert!(left.intersects(&intersecting));
        assert!(!left.intersects(&disjoint));
        assert_eq!(left.first_difference(&intersecting), Some(0));

        let left_before_union = left.clone();
        left.union_with(&intersecting);
        assert!(left_before_union.is_subset_of(&left));
        assert!(intersecting.is_subset_of(&left));
        assert_eq!(
            left.indices().collect::<Vec<_>>(),
            vec![0, 63, 64, 65, 127, 128, 129, 191, 192]
        );
    }

    #[test]
    fn storage_mask_indices_are_ascending() {
        let inline = mask(u64::BITS as usize, &[63, 1, 17, 0]);
        assert_eq!(inline.indices().collect::<Vec<_>>(), vec![0, 1, 17, 63]);

        let heap = mask(2 * u64::BITS as usize + 1, &[128, 65, 127, 0, 64, 2]);
        assert_eq!(
            heap.indices().collect::<Vec<_>>(),
            vec![0, 2, 64, 65, 127, 128]
        );
    }

    #[test]
    fn storage_mask_hashing_is_canonical() {
        struct TestHasher(u64);

        impl Hasher for TestHasher {
            fn finish(&self) -> u64 {
                self.0
            }

            fn write(&mut self, bytes: &[u8]) {
                for &byte in bytes {
                    self.0 ^= u64::from(byte);
                    self.0 = self.0.wrapping_mul(0x100_0000_01b3);
                }
            }
        }

        fn hash(mask: &StorageMask) -> u64 {
            let mut hasher = TestHasher(0xcbf2_9ce4_8422_2325);
            mask.hash(&mut hasher);
            hasher.finish()
        }

        for (storage_count, ascending_indices, descending_indices) in [
            (u64::BITS as usize, &[0, 1, 17, 63][..], &[63, 17, 1, 0][..]),
            (
                2 * u64::BITS as usize + 1,
                &[0, 1, 63, 64, 128][..],
                &[128, 64, 63, 1, 0][..],
            ),
        ] {
            let ascending = mask(storage_count, ascending_indices);
            let descending = mask(storage_count, descending_indices);
            assert!(ascending == descending);
            assert_eq!(hash(&ascending), hash(&descending));
        }
    }

    #[test]
    fn regroup_candidates_preserve_inline_and_multiword_behavior() {
        for (storage_count, chain, disjoint) in [
            (8, [0, 1, 2], [4, 5, 6, 7]),
            (130, [63, 64, 65], [0, 1, 128, 129]),
        ] {
            let mut chained = Vec::new();
            insert_regroup_candidate(
                &mut chained,
                RegroupCandidate::new(mask(storage_count, &chain[..2])),
            );
            insert_regroup_candidate(
                &mut chained,
                RegroupCandidate::new(mask(storage_count, &chain[1..])),
            );
            assert_eq!(chained.len(), 1);
            assert_eq!(chained[0].storages.indices().collect::<Vec<_>>(), chain);

            let mut separate = Vec::new();
            insert_regroup_candidate(
                &mut separate,
                RegroupCandidate::new(mask(storage_count, &disjoint[..2])),
            );
            insert_regroup_candidate(
                &mut separate,
                RegroupCandidate::new(mask(storage_count, &disjoint[2..])),
            );
            assert_eq!(separate.len(), 2);
        }
    }

    #[test]
    fn regroup_candidate_scratch_capacities_are_reused() {
        let mut candidates = Vec::new();
        let mut merged_candidates = Vec::new();
        let mut previous_capacities = (0, 0);

        for candidate_count in [1, 8, 2] {
            candidates.clear();
            merged_candidates.clear();

            for index in 0..candidate_count {
                candidates.push(RegroupCandidate::new(mask(16, &[index])));
            }
            for candidate in candidates.drain(..) {
                insert_regroup_candidate(&mut merged_candidates, candidate);
            }
            for candidate in merged_candidates.drain(..) {
                core::hint::black_box(candidate);
            }

            let capacities = regroup_candidate_capacities(&candidates, &merged_candidates);
            assert!(capacities.0 >= previous_capacities.0);
            assert!(capacities.1 >= previous_capacities.1);
            previous_capacities = capacities;
        }

        assert!(previous_capacities.0 >= 8);
        assert!(previous_capacities.1 >= 8);
    }

    #[cfg(feature = "std")]
    #[test]
    fn inline_candidate_clone_and_expansion_do_not_allocate() {
        let group_mask = mask(u64::BITS as usize, &[0, 1]);
        let allocations = allocation_counter::count(|| {
            let mut candidate = RegroupCandidate::new(group_mask.clone());
            while let Some(storage_index) = candidate
                .storages
                .first_difference(&candidate.expanded_storages)
            {
                candidate.expanded_storages.insert(storage_index);
            }
            core::hint::black_box(candidate);
        });

        assert_eq!(allocations, 0);
    }

    struct A;

    impl Component for A {
        type Tracking = track::Untracked;
    }

    struct RegroupProbe;

    impl Storage for RegroupProbe {}

    fn is_dirty(world: &World) -> bool {
        world
            .all_storages
            .borrow()
            .unwrap()
            .mutably_borrowed_since_regroup
            .load(Ordering::Acquire)
    }

    #[test]
    fn dirty_gate_marks_successful_mutable_borrows_and_skips_clean_regroups() {
        let mut world = World::new();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert(|| RegroupProbe)
                    .unwrap(),
            );
        }

        assert!(!is_dirty(&world));
        world.regroup();
        assert!(!is_dirty(&world));

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(all_storages.custom_storage_mut::<RegroupProbe>().unwrap());
            assert!(all_storages
                .mutably_borrowed_since_regroup
                .load(Ordering::Acquire));
        }

        world.regroup();
        assert!(!is_dirty(&world));

        world.regroup();
        assert!(!is_dirty(&world));
    }

    #[test]
    fn shared_and_failed_mutable_borrows_do_not_mark_dirty() {
        let mut world = World::new();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert(|| RegroupProbe)
                    .unwrap(),
            );
        }
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            let shared = all_storages.custom_storage::<RegroupProbe>().unwrap();

            assert!(all_storages.custom_storage_mut::<RegroupProbe>().is_err());
            assert!(!all_storages
                .mutably_borrowed_since_regroup
                .load(Ordering::Acquire));

            drop(shared);
            drop(all_storages.custom_storage::<RegroupProbe>().unwrap());
            assert!(!all_storages
                .mutably_borrowed_since_regroup
                .load(Ordering::Acquire));
        }
    }

    #[test]
    fn direct_component_entity_and_iteration_access_mark_dirty() {
        let mut world = World::new();

        let entity = world.add_entity(A);
        assert!(is_dirty(&world));
        world.regroup();
        assert!(!is_dirty(&world));

        drop(world.borrow::<View<'_, A>>().unwrap());
        assert!(!is_dirty(&world));

        drop(world.borrow::<ViewMut<'_, A>>().unwrap());
        assert!(is_dirty(&world));
        world.regroup();

        world.add_component(entity, (A,));
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(all_storages.iter_storages_mut());
        }
        assert!(is_dirty(&world));
    }

    fn sparse_set_with_entities(entities: &[EntityId]) -> SparseSet<A> {
        let mut sparse_set = SparseSet::new_custom_storage();
        for &entity in entities {
            let _ = sparse_set.insert(entity, A, TrackingTimestamp::new(1));
        }
        sparse_set
    }

    fn normalized_seed_batches(
        entities: &[EntityId],
        group_masks: &[StorageMask],
        sparse_arrays: &[Option<&SparseArray>],
        use_presence_mask: bool,
    ) -> Vec<(Vec<usize>, Vec<EntityId>)> {
        let mut batches: ShipHashMap<StorageMask, Vec<EntityId>> = ShipHashMap::new();
        let mut candidates = Vec::new();
        let mut presence_mask = StorageMask::new(sparse_arrays.len());

        for &entity in entities {
            candidates.clear();

            if use_presence_mask {
                seed_regroup_candidates_from_presence(
                    entity,
                    group_masks,
                    sparse_arrays,
                    &mut presence_mask,
                    &mut candidates,
                );
            } else {
                seed_regroup_candidates_direct(entity, group_masks, sparse_arrays, &mut candidates);
            }

            for candidate in candidates.drain(..) {
                batches.entry(candidate.storages).or_default().push(entity);
            }
        }

        let mut batches: Vec<_> = batches
            .into_iter()
            .map(|(signature, entities)| (signature.indices().collect(), entities))
            .collect();
        batches.sort_unstable();
        batches
    }

    #[test]
    fn direct_and_presence_seeding_produce_identical_signatures_and_batches() {
        let entities = [0, 1, 2, 3].map(EntityId::new);
        let storage0 = sparse_set_with_entities(&entities[..3]);
        let storage1 = sparse_set_with_entities(&entities[..3]);
        let storage2 = sparse_set_with_entities(&[entities[0], entities[2]]);
        let storage3 = sparse_set_with_entities(&[entities[1], entities[3]]);
        let storage4 = sparse_set_with_entities(&[entities[1], entities[3]]);
        let sparse_arrays = [
            Storage::sparse_array(&storage0),
            Storage::sparse_array(&storage1),
            Storage::sparse_array(&storage2),
            Storage::sparse_array(&storage3),
            Storage::sparse_array(&storage4),
            None,
        ];
        let group_masks = [
            mask(sparse_arrays.len(), &[0, 1]),
            mask(sparse_arrays.len(), &[1, 2]),
            mask(sparse_arrays.len(), &[3, 4]),
            mask(sparse_arrays.len(), &[0, 5]),
        ];

        let direct = normalized_seed_batches(&entities, &group_masks, &sparse_arrays, false);
        let presence = normalized_seed_batches(&entities, &group_masks, &sparse_arrays, true);

        assert_eq!(direct, presence);
        assert_eq!(
            direct,
            vec![
                (vec![0, 1], vec![entities[1]]),
                (vec![0, 1, 2], vec![entities[0], entities[2]]),
                (vec![3, 4], vec![entities[1], entities[3]]),
            ]
        );
    }

    struct IgnoredRegroupStorage(SparseSet<A>);

    impl Storage for IgnoredRegroupStorage {
        fn sparse_array(&self) -> Option<&SparseArray> {
            Some(&self.0.sparse)
        }
    }

    #[test]
    fn non_sparse_set_storage_is_ignored_by_regroup_collection() {
        let mut world = World::new();
        let entity = world.add_entity(());
        let mut sparse_set = SparseSet::new_custom_storage();
        sparse_set.add_group(&[TypeId::of::<IgnoredRegroupStorage>()]);
        let _ = sparse_set.insert(entity, A, TrackingTimestamp::new(1));

        world
            .all_storages
            .get_mut()
            .exclusive_storage_or_insert_mut(StorageId::of::<IgnoredRegroupStorage>(), || {
                IgnoredRegroupStorage(sparse_set)
            });

        world.regroup();

        let all_storages = world.all_storages.borrow().unwrap();
        let storage = all_storages
            .custom_storage::<IgnoredRegroupStorage>()
            .unwrap();
        assert_eq!(storage.0.sparse.pending_placement_pages(), &[0]);
        assert_eq!(storage.0.sparse.pending_placement_mask(0), 1);
    }

    #[cfg(feature = "thread_local")]
    struct NonSendStorage(core::marker::PhantomData<alloc::rc::Rc<()>>);

    #[cfg(feature = "thread_local")]
    unsafe impl Sync for NonSendStorage {}

    #[cfg(feature = "thread_local")]
    impl Storage for NonSendStorage {}

    #[cfg(feature = "thread_local")]
    struct NonSyncStorage(core::cell::Cell<()>);

    #[cfg(feature = "thread_local")]
    impl Storage for NonSyncStorage {}

    #[cfg(feature = "thread_local")]
    #[allow(dead_code)]
    struct NonSendSyncStorage(alloc::rc::Rc<()>);

    #[cfg(feature = "thread_local")]
    impl Storage for NonSendSyncStorage {}

    #[cfg(feature = "thread_local")]
    #[test]
    fn all_thread_local_mutable_access_variants_mark_dirty() {
        let mut world = World::new();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert_non_send_mut(|| {
                        NonSendStorage(core::marker::PhantomData)
                    })
                    .unwrap(),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert_non_send_mut_by_id(
                        StorageId::of::<NonSendStorage>(),
                        || NonSendStorage(core::marker::PhantomData),
                    )
                    .unwrap(),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert_non_sync_mut(|| {
                        NonSyncStorage(core::cell::Cell::new(()))
                    })
                    .unwrap(),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert_non_sync_mut_by_id(
                        StorageId::of::<NonSyncStorage>(),
                        || NonSyncStorage(core::cell::Cell::new(())),
                    )
                    .unwrap(),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert_non_send_sync_mut(|| {
                        NonSendSyncStorage(alloc::rc::Rc::new(()))
                    })
                    .unwrap(),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.borrow().unwrap();
            drop(
                all_storages
                    .custom_storage_or_insert_non_send_sync_mut_by_id(
                        StorageId::of::<NonSendSyncStorage>(),
                        || NonSendSyncStorage(alloc::rc::Rc::new(())),
                    )
                    .unwrap(),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.get_mut();
            let _ = all_storages.exclusive_storage_or_insert_non_send_mut(
                StorageId::of::<NonSendStorage>(),
                || NonSendStorage(core::marker::PhantomData),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.get_mut();
            let _ = all_storages.exclusive_storage_or_insert_non_sync_mut(
                StorageId::of::<NonSyncStorage>(),
                || NonSyncStorage(core::cell::Cell::new(())),
            );
        }
        assert!(is_dirty(&world));
        world.regroup();

        {
            let all_storages = world.all_storages.get_mut();
            let _ = all_storages.exclusive_storage_or_insert_non_send_sync_mut(
                StorageId::of::<NonSendSyncStorage>(),
                || NonSendSyncStorage(alloc::rc::Rc::new(())),
            );
        }
        assert!(is_dirty(&world));
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
