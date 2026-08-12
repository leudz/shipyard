mod add_component;
mod bucket_index;
mod bulk_add_entity;
mod component_bucket;
mod delete;
mod drain;
mod group_page;
mod groups;
mod memory_usage;
mod remove;
mod sparse_array;
#[cfg(feature = "thread_local")]
mod thread_local;
mod window;

pub use add_component::TupleAddComponent;
pub use bulk_add_entity::BulkAddEntity;
pub use delete::TupleDelete;
pub use drain::SparseSetDrain;
pub use memory_usage::{SparseSetMemory, SparseSetMemoryUsage};
pub use remove::TupleRemove;
pub use sparse_array::SparseArray;
#[doc(hidden)]
pub use window::RawEntityIdAccess;

pub(crate) use window::{FullRawWindow, FullRawWindowMut};

use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::error;
use crate::memory_usage::StorageMemoryUsage;
use crate::r#mut::Mut;
use crate::sparse_set::bucket_index::BucketIndex;
use crate::storage::{SBoxBuilder, Storage, StorageId};
use crate::tracking::{Tracking, TrackingTimestamp};
use alloc::boxed::Box;
use alloc::vec::Vec;
use component_bucket::ComponentBucket;
use core::any::{type_name, TypeId};
use core::mem::size_of;
use core::{
    cmp::{Ord, Ordering},
    fmt,
};
use groups::Groups;

pub(crate) const BUCKET_SIZE: usize = 256 / size_of::<EntityId>();

/// Default component storage.
// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.
pub struct SparseSet<T: Component> {
    pub(crate) sparse: SparseArray,
    pub(crate) unclassified_bucket: ComponentBucket<T>,
    pub(crate) groups: Groups,
    pub(crate) group_buckets: Vec<ComponentBucket<T>>,
    pub(crate) last_insert: TrackingTimestamp,
    pub(crate) last_modified: TrackingTimestamp,
    pub(crate) deletion_data: Vec<(EntityId, TrackingTimestamp, T)>,
    pub(crate) removal_data: Vec<(EntityId, TrackingTimestamp)>,
    pub(crate) is_tracking_insertion: bool,
    pub(crate) is_tracking_modification: bool,
    pub(crate) is_tracking_deletion: bool,
    pub(crate) is_tracking_removal: bool,
    #[allow(clippy::type_complexity)]
    on_insertion: Option<Box<dyn FnMut(EntityId, &T) + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    on_removal: Option<Box<dyn FnMut(EntityId, &T) + Send + Sync>>,
    clone: Option<fn(&T) -> T>,
}

impl<T: fmt::Debug + Component> fmt::Debug for SparseSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(
                self.unclassified_bucket
                    .dense
                    .iter()
                    .zip(&self.unclassified_bucket.data),
            )
            .finish()
    }
}

impl<T: Component> SparseSet<T> {
    #[inline]
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: SparseArray::new(),
            unclassified_bucket: ComponentBucket::new(),
            groups: Groups::new(),
            group_buckets: Vec::new(),
            last_insert: TrackingTimestamp::new(0),
            last_modified: TrackingTimestamp::new(0),
            deletion_data: Vec::new(),
            removal_data: Vec::new(),
            is_tracking_insertion: T::Tracking::track_insertion(),
            is_tracking_modification: T::Tracking::track_modification(),
            is_tracking_deletion: T::Tracking::track_deletion(),
            is_tracking_removal: T::Tracking::track_removal(),
            on_insertion: None,
            on_removal: None,
            clone: None,
        }
    }
    /// Returns a new [`SparseSet`] to be used in custom storage.
    #[inline]
    pub fn new_custom_storage() -> Self {
        SparseSet::new()
    }
    /// Returns a slice of all the components in this storage.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.unclassified_bucket.data
    }

    #[inline]
    pub(crate) fn add_group(&mut self, group: &[TypeId]) {
        self.groups.add(group);
        self.group_buckets.push(ComponentBucket::new());
    }

    fn move_entity_to_bucket(&mut self, entity: EntityId, target: BucketIndex) {
        let dense_index = self.index_of(entity).unwrap();
        let source_bucket_index = self.sparse.bucket_index(entity);

        if source_bucket_index == target {
            // I'm not sure if this should be silent, it could be an assert instead
            return;
        }

        let is_tracking_insertion = self.is_tracking_insertion;
        let is_tracking_modification = self.is_tracking_modification;

        let (component, insertion, modification, replacement) = {
            let source_bucket = if let Some(source_group_index) = source_bucket_index.group_index()
            {
                self.group_buckets
                    .get_mut(source_group_index)
                    .expect("A component's bucket index must reference an existing group.")
            } else {
                &mut self.unclassified_bucket
            };

            source_bucket.dense.swap_remove(dense_index);

            let insertion = is_tracking_insertion
                .then(|| source_bucket.insertion_data.swap_remove(dense_index));
            let modification = is_tracking_modification
                .then(|| source_bucket.modification_data.swap_remove(dense_index));
            let component = source_bucket.data.swap_remove(dense_index);
            let replacement = source_bucket.dense.get(dense_index).copied();

            (component, insertion, modification, replacement)
        };

        if let Some(replacement) = replacement {
            unsafe {
                self.sparse
                    .get_mut_unchecked(replacement)
                    .set_index(dense_index as u64);
            }
        }

        let target_bucket = if let Some(group_index) = target.group_index() {
            unsafe { self.group_buckets.get_unchecked_mut(group_index) }
        } else {
            &mut self.unclassified_bucket
        };
        let target_dense_index = target_bucket.dense.len();

        target_bucket.dense.push(entity);
        target_bucket.data.push(component);
        if let Some(insertion) = insertion {
            target_bucket.insertion_data.push(insertion);
        }
        if let Some(modification) = modification {
            target_bucket.modification_data.push(modification);
        }

        unsafe {
            self.sparse
                .get_mut_unchecked(entity)
                .set_index(target_dense_index as u64);
        }
        self.sparse.set_bucket_index(entity, target);
    }

    fn private_collect_regroup_pages(
        &mut self,
        emit_page: &mut dyn FnMut(usize, u32),
        emit_group: &mut dyn FnMut(&[TypeId]),
    ) {
        for group in self.groups.iter() {
            emit_group(group);
        }

        if self.sparse.pending_placement_pages.is_empty() {
            return;
        }

        let mut page_position = 0;

        while page_position < self.sparse.pending_placement_pages.len() {
            let page_index = self.sparse.pending_placement_pages[page_position];
            let mask = core::mem::take(
                self.sparse
                    .pending_placement_masks
                    .get_mut(page_index)
                    .unwrap(),
            );

            if mask != 0 {
                emit_page(page_index, mask);
            }

            page_position += 1;
        }

        self.sparse.pending_placement_pages.clear();
    }
}

impl<T: Component> SparseSet<T> {
    /// Returns `true` if `entity` owns a component in this storage.
    #[inline]
    pub fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }
    /// Returns the length of the storage.
    #[inline]
    pub fn len(&self) -> usize {
        self.unclassified_bucket.dense.len()
    }
    /// Returns true if the storage's length is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.unclassified_bucket.dense.is_empty()
    }
}

impl<T: Component> SparseSet<T> {
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    #[inline]
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        self.sparse.get(entity).and_then(|sparse_entity| {
            if entity.gen() == sparse_entity.gen() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        })
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    ///
    /// # Safety
    ///
    /// `entity` has to own a component of this type.  
    /// The index is only valid until a modification occurs in the storage.
    #[inline]
    pub unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        self.sparse.get_unchecked(entity).uindex()
    }
    /// Returns the `EntityId` at a given `index`.
    #[inline]
    pub fn id_at(&self, index: usize) -> Option<EntityId> {
        self.unclassified_bucket.dense.get(index).copied()
    }

    /// Sets the on insertion callback.
    pub fn on_insertion(&mut self, f: impl FnMut(EntityId, &T) + Send + Sync + 'static) {
        self.on_insertion = Some(Box::new(f));
    }

    /// Remove the on insertion callback.
    #[allow(clippy::type_complexity)]
    pub fn take_on_insertion(
        &mut self,
    ) -> Option<Box<dyn FnMut(EntityId, &T) + Send + Sync + 'static>> {
        self.on_insertion.take()
    }

    /// Sets the on removal and deletion callback.
    pub fn on_removal(&mut self, f: impl FnMut(EntityId, &T) + Send + Sync + 'static) {
        self.on_removal = Some(Box::new(f));
    }

    /// Remove the on removal and deletion callback.
    #[allow(clippy::type_complexity)]
    pub fn take_on_removal(
        &mut self,
    ) -> Option<Box<dyn FnMut(EntityId, &T) + Send + Sync + 'static>> {
        self.on_removal.take()
    }

    #[inline]
    pub(crate) fn private_get(&self, entity: EntityId) -> Option<&T> {
        self.index_of(entity)
            .map(|index| unsafe { self.unclassified_bucket.data.get_unchecked(index) })
    }
}

/// [`SparseSet::insert`]'s return value.
#[must_use]
pub enum InsertionResult<T> {
    /// No component were present at this index.
    Inserted,
    /// The component was inserted.\
    /// A component from the same entity was present.
    ComponentOverride(T),
    /// A component from an entity with a smaller generation was present.
    OtherComponentOverride,
    /// A component from an entity with a larger generation was present.
    NotInserted,
}

impl<T> InsertionResult<T> {
    pub(crate) fn was_inserted(&self) -> bool {
        match self {
            InsertionResult::Inserted
            | InsertionResult::ComponentOverride(_)
            | InsertionResult::OtherComponentOverride => true,
            InsertionResult::NotInserted => false,
        }
    }

    #[track_caller]
    pub(crate) fn assert_inserted(&self) {
        assert!(self.was_inserted());
    }
}

impl<T: Component> SparseSet<T> {
    /// Inserts `value` in the `SparseSet`.
    ///
    /// # Tracking
    ///
    /// In case `entity` had a component of this type, the new component will be considered `modified`.  
    /// In all other cases it'll be considered `inserted`.
    #[track_caller]
    pub fn insert(
        &mut self,
        entity: EntityId,
        value: T,
        current: TrackingTimestamp,
    ) -> InsertionResult<T> {
        self.sparse.allocate_at(entity);

        // at this point there can't be nothing at the sparse index
        let sparse_entity = unsafe { self.sparse.get_mut_unchecked(entity) };

        let old_component;
        let mut should_set_pending_placement = false;

        if sparse_entity.is_dead() {
            if let Some(on_insertion) = &mut self.on_insertion {
                on_insertion(entity, &value);
            }

            *sparse_entity = EntityId::new_from_index_and_gen(
                self.unclassified_bucket.dense.len() as u64,
                entity.gen(),
            );

            if self.is_tracking_insertion {
                self.unclassified_bucket.insertion_data.push(current);
            }
            if self.is_tracking_modification {
                self.unclassified_bucket
                    .modification_data
                    .push(TrackingTimestamp::origin());
            }

            self.unclassified_bucket.dense.push(entity);
            self.unclassified_bucket.data.push(value);

            old_component = InsertionResult::Inserted;
            should_set_pending_placement = true;
        } else if entity.gen() == sparse_entity.gen() {
            if let Some(on_insertion) = &mut self.on_insertion {
                on_insertion(entity, &value);
            }

            let old_data = unsafe {
                core::mem::replace(
                    self.unclassified_bucket
                        .data
                        .get_unchecked_mut(sparse_entity.uindex()),
                    value,
                )
            };

            old_component = InsertionResult::ComponentOverride(old_data);

            sparse_entity.copy_gen(entity);

            let dense_entity = unsafe {
                self.unclassified_bucket
                    .dense
                    .get_unchecked_mut(sparse_entity.uindex())
            };

            if self.is_tracking_modification {
                unsafe {
                    *self
                        .unclassified_bucket
                        .modification_data
                        .get_unchecked_mut(sparse_entity.uindex()) = current;
                }
            }

            dense_entity.copy_index_gen(entity);
        } else if entity.gen() > sparse_entity.gen() {
            if let Some(on_insertion) = &mut self.on_insertion {
                on_insertion(entity, &value);
            }

            let _ = unsafe {
                core::mem::replace(
                    self.unclassified_bucket
                        .data
                        .get_unchecked_mut(sparse_entity.uindex()),
                    value,
                )
            };

            old_component = InsertionResult::OtherComponentOverride;
            should_set_pending_placement = true;

            sparse_entity.copy_gen(entity);

            let dense_entity = unsafe {
                self.unclassified_bucket
                    .dense
                    .get_unchecked_mut(sparse_entity.uindex())
            };

            if self.is_tracking_insertion {
                unsafe {
                    *self
                        .unclassified_bucket
                        .insertion_data
                        .get_unchecked_mut(sparse_entity.uindex()) = current;
                }
            }

            dense_entity.copy_index_gen(entity);
        } else {
            old_component = InsertionResult::NotInserted;
        }

        if should_set_pending_placement && !self.groups.is_empty() {
            self.sparse.set_pending_placement(entity);
        }

        old_component
    }
}

impl<T: Component> SparseSet<T> {
    /// Same as `delete` but checks tracking at runtime.
    #[inline]
    pub(crate) fn dyn_delete(&mut self, entity: EntityId, current: TrackingTimestamp) -> bool {
        if let Some(component) = self.actual_remove(entity) {
            if self.is_tracking_deletion() {
                self.deletion_data.push((entity, current, component));
            }

            true
        } else {
            false
        }
    }

    /// Same as `remove` but checks tracking at runtime.
    #[inline]
    pub(crate) fn dyn_remove(&mut self, entity: EntityId, current: TrackingTimestamp) -> Option<T> {
        let component = self.actual_remove(entity);

        if component.is_some() && self.is_tracking_removal() {
            self.removal_data.push((entity, current));
        }

        component
    }

    #[inline]
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<T> {
        let sparse_entity = self.sparse.get(entity)?;

        if entity.gen() >= sparse_entity.gen() {
            let source_bucket_index = self.sparse.bucket_index(entity);
            let source_group_index = source_bucket_index.group_index();
            let dense_index = sparse_entity.uindex();
            let removed_entity = {
                let source_bucket = if let Some(group_index) = source_group_index {
                    self.group_buckets
                        .get(group_index)
                        .expect("A component's bucket index must reference an existing group.")
                } else {
                    &self.unclassified_bucket
                };

                unsafe { *source_bucket.dense.get_unchecked(dense_index) }
            };

            unsafe {
                *self.sparse.get_mut_unchecked(entity) = EntityId::dead();
            }
            self.sparse
                .set_bucket_index(entity, BucketIndex::UNCLASSIFIED);

            let is_tracking_insertion = self.is_tracking_insertion();
            let is_tracking_modification = self.is_tracking_modification();
            let (component, replacement) = {
                let source_bucket = if let Some(group_index) = source_group_index {
                    self.group_buckets
                        .get_mut(group_index)
                        .expect("A component's bucket index must reference an existing group.")
                } else {
                    &mut self.unclassified_bucket
                };

                source_bucket.dense.swap_remove(dense_index);
                if is_tracking_insertion {
                    source_bucket.insertion_data.swap_remove(dense_index);
                }
                if is_tracking_modification {
                    source_bucket.modification_data.swap_remove(dense_index);
                }
                let component = source_bucket.data.swap_remove(dense_index);
                let replacement = source_bucket.dense.get(dense_index).copied();

                (component, replacement)
            };

            // The SparseSet could now be empty or the removed component could have been the last one
            if let Some(replacement) = replacement {
                unsafe {
                    self.sparse
                        .get_mut_unchecked(replacement)
                        .copy_index(sparse_entity);
                }
            }

            if let Some(group_index) = source_group_index {
                self.sparse
                    .removed_from_groups
                    .push((removed_entity, group_index));
            }

            if entity.gen() == sparse_entity.gen() {
                if let Some(on_remove) = &mut self.on_removal {
                    on_remove(entity, &component);
                }

                Some(component)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl<T: Component> SparseSet<T> {
    /// Removes the *inserted* flag on all components of this storage.
    pub(crate) fn private_clear_all_inserted(&mut self, current: TrackingTimestamp) {
        self.last_insert = current;
    }
    /// Removes the *modified* flag on all components of this storage.
    pub(crate) fn private_clear_all_modified(&mut self, current: TrackingTimestamp) {
        self.last_modified = current;
    }
    /// Removes the *inserted* and *modified* flags on all components of this storage.
    pub(crate) fn private_clear_all_inserted_and_modified(&mut self, current: TrackingTimestamp) {
        self.last_insert = current;
        self.last_modified = current;
    }
    /// Clear all deletion tracking data.
    pub fn clear_all_deleted(&mut self) {
        self.deletion_data.clear();
    }
    /// Clear all deletion tracking data older than some timestamp.
    pub fn clear_all_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));
    }
    /// Clear all removal tracking data.
    pub fn clear_all_removed(&mut self) {
        self.removal_data.clear();
    }
    /// Clear all removal tracking data older than some timestamp.
    pub fn clear_all_removed_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.removal_data
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
    /// Clear all deletion and removal tracking data.
    pub fn clear_all_removed_and_deleted(&mut self) {
        self.removal_data.clear();
    }
    /// Clear all deletion and removal tracking data older than some timestamp.
    pub fn clear_all_removed_and_deleted_older_than_timestamp(
        &mut self,
        timestamp: TrackingTimestamp,
    ) {
        self.deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));
        self.removal_data
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
}

impl<T: Component> SparseSet<T> {
    /// Make this storage track insertions.
    #[allow(clippy::manual_repeat_n, reason = "Too recent version")]
    pub fn track_insertion(&mut self) -> &mut SparseSet<T> {
        if self.is_tracking_insertion() {
            return self;
        }

        self.is_tracking_insertion = true;

        self.unclassified_bucket.insertion_data.extend(
            core::iter::repeat(TrackingTimestamp::new(0))
                .take(self.unclassified_bucket.dense.len()),
        );

        self
    }
    /// Make this storage track modification.
    #[allow(clippy::manual_repeat_n, reason = "Too recent version")]
    pub fn track_modification(&mut self) -> &mut SparseSet<T> {
        if self.is_tracking_modification() {
            return self;
        }

        self.is_tracking_modification = true;

        self.unclassified_bucket.modification_data.extend(
            core::iter::repeat(TrackingTimestamp::new(0))
                .take(self.unclassified_bucket.dense.len()),
        );

        self
    }
    /// Make this storage track deletions.
    pub fn track_deletion(&mut self) -> &mut SparseSet<T> {
        self.is_tracking_deletion = true;
        self
    }
    /// Make this storage track removals.
    pub fn track_removal(&mut self) -> &mut SparseSet<T> {
        self.is_tracking_removal = true;
        self
    }
    /// Make this storage track insertions, modifications, deletions and removals.
    pub fn track_all(&mut self) {
        self.track_insertion()
            .track_modification()
            .track_deletion()
            .track_removal();
    }
    /// Returns `true` if the storage tracks insertion.
    pub fn is_tracking_insertion(&self) -> bool {
        self.is_tracking_insertion
    }
    /// Returns `true` if the storage tracks modification.
    pub fn is_tracking_modification(&self) -> bool {
        self.is_tracking_modification
    }
    /// Returns `true` if the storage tracks deletion.
    pub fn is_tracking_deletion(&self) -> bool {
        self.is_tracking_deletion
    }
    /// Returns `true` if the storage tracks removal.
    pub fn is_tracking_removal(&self) -> bool {
        self.is_tracking_removal
    }
    /// Returns `true` if the storage tracks insertion, deletion or removal.
    pub fn is_tracking_any(&self) -> bool {
        self.is_tracking_insertion()
            || self.is_tracking_modification()
            || self.is_tracking_deletion()
            || self.is_tracking_removal()
    }
    pub(crate) fn check_tracking<Track: Tracking>(&self) -> Result<(), error::GetStorage> {
        if (Track::track_insertion() && !self.is_tracking_insertion())
            || (Track::track_modification() && !self.is_tracking_modification())
            || (Track::track_deletion() && !self.is_tracking_deletion())
            || (Track::track_removal() && !self.is_tracking_removal())
        {
            return Err(error::GetStorage::TrackingNotEnabled {
                name: Some(type_name::<SparseSet<T>>()),
                id: StorageId::of::<SparseSet<T>>(),
                tracking: Track::name(),
            });
        }

        Ok(())
    }
    pub(crate) fn enable_tracking<Track: Tracking>(&mut self) {
        if Track::track_insertion() {
            self.track_insertion();
        }
        if Track::track_modification() {
            self.track_modification();
        }
        if Track::track_deletion() {
            self.track_deletion();
        }
        if Track::track_removal() {
            self.track_removal();
        }
    }
}

impl<T: Component> SparseSet<T> {
    /// Reserves memory for at least `additional` components. Adding components can still allocate though.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.unclassified_bucket.dense.reserve(additional);
        self.unclassified_bucket.data.reserve(additional);
    }
    /// Sorts the `SparseSet` with a comparator function, but may not preserve the order of equal elements.
    pub fn sort_unstable_by<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        let mut transform: Vec<usize> = (0..self.unclassified_bucket.dense.len()).collect();

        transform.sort_unstable_by(|&i, &j| {
            // SAFE dense and data have the same length
            compare(
                unsafe { self.unclassified_bucket.data.get_unchecked(i) },
                unsafe { self.unclassified_bucket.data.get_unchecked(j) },
            )
        });

        let mut pos;
        for i in 0..transform.len() {
            // SAFE we're in bound
            pos = unsafe { *transform.get_unchecked(i) };
            while pos < i {
                // SAFE we're in bound
                pos = unsafe { *transform.get_unchecked(pos) };
            }
            self.unclassified_bucket.dense.swap(i, pos);
            self.unclassified_bucket.data.swap(i, pos);
            if self.is_tracking_insertion {
                self.unclassified_bucket.insertion_data.swap(i, pos);
            }
            if self.is_tracking_modification {
                self.unclassified_bucket.modification_data.swap(i, pos);
            }
        }

        for (i, id) in self.unclassified_bucket.dense.iter().enumerate() {
            unsafe {
                self.sparse.get_mut_unchecked(*id).set_index(i as u64);
            }
        }
    }

    /// Applies the given function `f` to the entities `a` and `b`.\
    /// The two entities shouldn't point to the same component.  
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    pub(crate) fn private_apply<R, F: FnOnce(&mut T, &T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
        current: TrackingTimestamp,
    ) -> R {
        let a_index = self.index_of(a).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                a
            )
        });
        let b_index = self.index_of(b).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                b
            )
        });

        if a_index != b_index {
            if self.is_tracking_modification {
                self.unclassified_bucket.modification_data[a_index] = current;
            }

            let a = unsafe { &mut *self.unclassified_bucket.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &*self.unclassified_bucket.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }

    /// Applies the given function `f` to the entities `a` and `b`.\
    /// The two entities shouldn't point to the same component.  
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    pub(crate) fn private_apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
        current: TrackingTimestamp,
    ) -> R {
        let a_index = self.index_of(a).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                a
            )
        });
        let b_index = self.index_of(b).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                b
            )
        });

        if a_index != b_index {
            if self.is_tracking_modification {
                self.unclassified_bucket.modification_data[a_index] = current;
                self.unclassified_bucket.modification_data[b_index] = current;
            }

            let a = unsafe { &mut *self.unclassified_bucket.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &mut *self.unclassified_bucket.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }

    /// Deletes all components in this storage.
    pub(crate) fn private_clear(&mut self, current: TrackingTimestamp) {
        for &id in &self.unclassified_bucket.dense {
            unsafe {
                *self.sparse.get_mut_unchecked(id) = EntityId::dead();
            }
        }

        self.unclassified_bucket.insertion_data.clear();
        self.unclassified_bucket.modification_data.clear();

        let is_tracking_deletion = self.is_tracking_deletion();

        let dense = self.unclassified_bucket.dense.drain(..);
        let data = self.unclassified_bucket.data.drain(..);

        if is_tracking_deletion {
            let iter = dense
                .zip(data)
                .map(|(entity, component)| (entity, current, component));
            self.deletion_data.extend(iter);
        }
    }

    /// Creates a draining iterator that empties the storage and yields the removed items.
    pub(crate) fn private_drain(&mut self, current: TrackingTimestamp) -> SparseSetDrain<'_, T> {
        if self.is_tracking_removal {
            self.removal_data.extend(
                self.unclassified_bucket
                    .dense
                    .iter()
                    .map(|&entity| (entity, current)),
            );
        }

        for id in &self.unclassified_bucket.dense {
            // SAFE ids from sparse_set.dense are always valid
            unsafe {
                *self.sparse.get_mut_unchecked(*id) = EntityId::dead();
            }
        }

        self.unclassified_bucket.insertion_data.clear();
        self.unclassified_bucket.modification_data.clear();

        let dense_ptr = self.unclassified_bucket.dense.as_ptr();
        let dense_len = self.unclassified_bucket.dense.len();

        unsafe {
            self.unclassified_bucket.dense.set_len(0);
        }

        SparseSetDrain {
            dense_ptr,
            dense_len,
            data: self.unclassified_bucket.data.drain(..),
        }
    }

    pub(crate) fn private_retain<F: FnMut(EntityId, &T) -> bool>(
        &mut self,
        current: TrackingTimestamp,
        mut f: F,
    ) {
        let mut removed = 0;
        for i in 0..self.len() {
            let i = i - removed;

            let eid = unsafe { *self.unclassified_bucket.dense.get_unchecked(i) };
            let component = unsafe { self.unclassified_bucket.data.get_unchecked(i) };

            if !f(eid, component) {
                self.dyn_delete(eid, current);
                removed += 1;
            }
        }
    }

    pub(crate) fn private_retain_mut<F: FnMut(EntityId, Mut<'_, T>) -> bool>(
        &mut self,
        current: TrackingTimestamp,
        mut f: F,
    ) {
        let mut removed = 0;
        for i in 0..self.len() {
            let i = i - removed;

            let eid = unsafe { *self.unclassified_bucket.dense.get_unchecked(i) };
            let component = Mut {
                flag: self.unclassified_bucket.modification_data.get_mut(i),
                current,
                data: unsafe { self.unclassified_bucket.data.get_unchecked_mut(i) },
            };

            if !f(eid, component) {
                self.dyn_delete(eid, current);
                removed += 1;
            }
        }
    }
}

impl<T: Ord + Component> SparseSet<T> {
    /// Sorts the `SparseSet`, but may not preserve the order of equal elements.
    pub fn sort_unstable(&mut self) {
        self.sort_unstable_by(Ord::cmp)
    }
}

impl<T: Clone + Component> SparseSet<T> {
    /// Registers the function to clone this component.
    #[inline]
    pub fn register_clone(&mut self) {
        self.clone = Some(T::clone)
    }
}

impl<T: Component + Send + Sync> Storage for SparseSet<T> {
    #[inline]
    fn delete(&mut self, entity: EntityId, current: TrackingTimestamp) {
        self.dyn_delete(entity, current);
    }
    #[inline]
    fn clear(&mut self, current: TrackingTimestamp) {
        self.private_clear(current);
    }
    fn sparse_array(&self) -> Option<&SparseArray> {
        Some(&self.sparse)
    }
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(self.private_memory_usage())
    }
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
    fn clear_all_inserted(&mut self, current: TrackingTimestamp) {
        self.last_insert = current;
    }
    fn clear_all_modified(&mut self, current: TrackingTimestamp) {
        self.last_modified = current;
    }
    fn clear_all_removed_and_deleted(&mut self) {
        self.deletion_data.clear();
        self.removal_data.clear();
    }
    fn clear_all_removed_and_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));

        self.removal_data
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
    #[inline]
    fn move_component_from(
        &mut self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        current: TrackingTimestamp,
        other_current: TrackingTimestamp,
    ) {
        if let Some(component) = self.dyn_remove(from, current) {
            let other_sparse_set = other_all_storages.exclusive_storage_or_insert_mut(
                StorageId::of::<SparseSet<T>>(),
                SparseSet::<T>::new,
            );

            let _ = other_sparse_set.insert(to, component, other_current);
        }
    }

    fn try_clone(&self, other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        self.clone.map(|clone| {
            let mut sparse_set = SparseSet::<T>::new();

            sparse_set.sparse = self.sparse.clone();
            sparse_set.unclassified_bucket.dense = self.unclassified_bucket.dense.clone();
            sparse_set.unclassified_bucket.data =
                self.unclassified_bucket.data.iter().map(clone).collect();

            if sparse_set.is_tracking_insertion {
                sparse_set
                    .unclassified_bucket
                    .insertion_data
                    .resize(self.unclassified_bucket.dense.len(), other_current);
            }
            if sparse_set.is_tracking_modification {
                sparse_set.unclassified_bucket.modification_data.resize(
                    self.unclassified_bucket.dense.len(),
                    TrackingTimestamp::origin(),
                );
            }

            SBoxBuilder::new(sparse_set)
        })
    }

    fn clone_component_to(
        &self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        other_current: TrackingTimestamp,
    ) {
        if let Some(clone) = &self.clone {
            if let Some(component) = self.private_get(from) {
                let other_sparse_set = other_all_storages.exclusive_storage_or_insert_mut(
                    StorageId::of::<SparseSet<T>>(),
                    SparseSet::<T>::new,
                );

                let _ = other_sparse_set.insert(to, (clone)(component), other_current);
            }
        }
    }

    fn collect_regroup_pages(
        &mut self,
        emit_page: &mut dyn FnMut(usize, u32),
        emit_group: &mut dyn FnMut(&[TypeId]),
    ) {
        self.private_collect_regroup_pages(emit_page, emit_group);
    }

    fn entity_group(&self, entity: EntityId) -> Option<&[TypeId]> {
        if !self.sparse.contains(entity) {
            return None;
        }

        self.sparse
            .bucket_index(entity)
            .group_index()
            .map(|group_index| &self.groups[group_index])
    }

    fn move_to_group(&mut self, entity: EntityId, group: &[TypeId]) {
        self.move_to_group_batch(core::slice::from_ref(&entity), group);
    }

    fn move_to_group_batch(&mut self, entities: &[EntityId], group: &[TypeId]) {
        let target_bucket_index = if group.is_empty() {
            BucketIndex::UNCLASSIFIED
        } else {
            let group_index = self
                .groups
                .iter()
                .position(|local_group| local_group == group);
            let group_index = group_index.unwrap_or_else(|| {
                let group_index = self.groups.add(group);
                self.group_buckets.push(ComponentBucket::new());
                group_index
            });

            BucketIndex::from_group_index(group_index)
        };

        for &entity in entities {
            self.move_entity_to_bucket(entity, target_bucket_index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Component, Group, View, ViewMut, World};
    use alloc::vec;
    use std::println;
    use std::sync::{Arc, Mutex};

    #[derive(PartialEq, Eq, Debug)]
    struct STR(&'static str);

    impl Component for STR {
        type Tracking = crate::track::Untracked;
    }

    #[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
    struct I32(i32);

    impl Component for I32 {
        type Tracking = crate::track::Untracked;
    }

    #[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
    struct TrackedI32(i32);

    impl Component for TrackedI32 {
        type Tracking = crate::track::All;
    }

    struct A;
    struct B;
    struct C;
    struct D;

    impl Component for A {
        type Tracking = crate::track::Untracked;
    }

    impl Component for B {
        type Tracking = crate::track::Untracked;
    }

    impl Component for C {
        type Tracking = crate::track::Untracked;
    }

    impl Component for D {
        type Tracking = crate::track::Untracked;
    }

    fn sorted_group(mut storages: Vec<TypeId>) -> Vec<TypeId> {
        storages.sort_unstable();
        storages
    }

    fn storage_id<T: Component>() -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }

    fn current_group<T: Component>(sparse_set: &SparseSet<T>, entity: EntityId) -> &[TypeId] {
        let group_index = sparse_set
            .sparse
            .bucket_index(entity)
            .group_index()
            .expect("the entity should occupy a group");

        &sparse_set.groups[group_index]
    }

    #[test]
    fn insert() {
        let mut array = SparseSet::new();

        array
            .insert(
                EntityId::new_from_parts(0, 0),
                STR("0"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        assert_eq!(
            array.unclassified_bucket.dense,
            &[EntityId::new_from_parts(0, 0)]
        );
        assert_eq!(array.unclassified_bucket.data, &[STR("0")]);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(0, 0)),
            Some(&STR("0"))
        );

        array
            .insert(
                EntityId::new_from_parts(1, 0),
                STR("1"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        assert_eq!(
            array.unclassified_bucket.dense,
            &[
                EntityId::new_from_parts(0, 0),
                EntityId::new_from_parts(1, 0)
            ]
        );
        assert_eq!(array.unclassified_bucket.data, &[STR("0"), STR("1")]);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(0, 0)),
            Some(&STR("0"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(1, 0)),
            Some(&STR("1"))
        );

        array
            .insert(
                EntityId::new_from_parts(5, 0),
                STR("5"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        assert_eq!(
            array.unclassified_bucket.dense,
            &[
                EntityId::new_from_parts(0, 0),
                EntityId::new_from_parts(1, 0),
                EntityId::new_from_parts(5, 0)
            ]
        );
        assert_eq!(
            array.unclassified_bucket.data,
            &[STR("0"), STR("1"), STR("5")]
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0)),
            Some(&STR("5"))
        );

        assert_eq!(array.private_get(EntityId::new_from_parts(4, 0)), None);
    }

    #[test]
    fn insertion_marks_pending_placement_only_when_needed() {
        let mut sparse_set = SparseSet::new();
        let existing = EntityId::new_from_parts(0, 0);

        sparse_set
            .insert(existing, I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        assert!(sparse_set.sparse.pending_placement_pages().is_empty());
        assert_eq!(sparse_set.sparse.pending_placement_mask(0), 0);

        sparse_set.add_group(&[]);

        let result = sparse_set.insert(existing, I32(1), TrackingTimestamp::new(1));
        assert!(matches!(result, InsertionResult::ComponentOverride(I32(0))));
        assert!(sparse_set.sparse.pending_placement_pages().is_empty());

        let inserted = EntityId::new_from_parts(31, 0);
        sparse_set
            .insert(inserted, I32(31), TrackingTimestamp::new(2))
            .assert_inserted();
        assert_eq!(sparse_set.sparse.pending_placement_pages(), &[0]);
        assert_eq!(sparse_set.sparse.pending_placement_mask(0), 1 << 31);

        let newer_generation = EntityId::new_from_parts(0, 1);
        let result = sparse_set.insert(newer_generation, I32(2), TrackingTimestamp::new(3));
        assert!(matches!(result, InsertionResult::OtherComponentOverride));
        assert_eq!(
            sparse_set.sparse.pending_placement_mask(0),
            (1 << 0) | (1 << 31)
        );

        let older_generation = EntityId::new_from_parts(0, 0);
        let result = sparse_set.insert(older_generation, I32(3), TrackingTimestamp::new(4));
        assert!(matches!(result, InsertionResult::NotInserted));
        assert_eq!(
            sparse_set.sparse.pending_placement_mask(0),
            (1 << 0) | (1 << 31)
        );
    }

    #[test]
    fn pending_placement_is_entity_indexed_and_monotonic() {
        let mut sparse_set = SparseSet::new();
        sparse_set.add_group(&[]);

        let first = EntityId::new(0);
        let second = EntityId::new(32);
        sparse_set
            .insert(first, I32(2), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(second, I32(1), TrackingTimestamp::new(0))
            .assert_inserted();

        let pages = sparse_set.sparse.pending_placement_pages().to_vec();
        let masks = [
            sparse_set.sparse.pending_placement_mask(0),
            sparse_set.sparse.pending_placement_mask(1),
        ];

        sparse_set.sort_unstable();
        assert_eq!(sparse_set.sparse.pending_placement_pages(), pages);
        assert_eq!(sparse_set.sparse.pending_placement_mask(0), masks[0]);
        assert_eq!(sparse_set.sparse.pending_placement_mask(1), masks[1]);

        assert_eq!(
            sparse_set.dyn_remove(first, TrackingTimestamp::new(1)),
            Some(I32(2))
        );
        assert_eq!(sparse_set.sparse.pending_placement_pages(), pages);
        assert_eq!(sparse_set.sparse.pending_placement_mask(0), masks[0]);

        sparse_set.private_clear(TrackingTimestamp::new(2));
        assert_eq!(sparse_set.sparse.pending_placement_pages(), pages);
        assert_eq!(sparse_set.sparse.pending_placement_mask(1), masks[1]);

        sparse_set
            .insert(EntityId::new(31), I32(3), TrackingTimestamp::new(3))
            .assert_inserted();
        let mask = sparse_set.sparse.pending_placement_mask(0);
        drop(sparse_set.private_drain(TrackingTimestamp::new(4)));
        assert_eq!(sparse_set.sparse.pending_placement_mask(0), mask);
        assert_eq!(sparse_set.sparse.pending_placement_pages(), pages);
    }

    #[test]
    fn collected_pending_page_can_be_appended_again_once() {
        let mut sparse_set = SparseSet::new();
        sparse_set.add_group(&[]);
        let entity = EntityId::new(64);
        sparse_set
            .insert(entity, I32(1), TrackingTimestamp::new(0))
            .assert_inserted();

        let mut emitted = Vec::new();
        sparse_set.private_collect_regroup_pages(
            &mut |page_index, mask| emitted.push((page_index, mask)),
            &mut |_| {},
        );

        assert_eq!(emitted, [(2, 1)]);
        assert!(sparse_set.sparse.pending_placement_pages().is_empty());
        assert_eq!(sparse_set.sparse.pending_placement_mask(2), 0);

        sparse_set.sparse.set_pending_placement(entity);
        sparse_set.sparse.set_pending_placement(entity);

        assert_eq!(sparse_set.sparse.pending_placement_pages(), &[2]);
        assert_eq!(sparse_set.sparse.pending_placement_mask(2), 1);
    }

    #[cfg(feature = "thread_local")]
    #[test]
    fn thread_local_wrappers_delegate_regroup_page_collection() {
        use crate::borrow::{NonSend, NonSendSync, NonSync};

        fn pending_sparse_set() -> SparseSet<I32> {
            let mut sparse_set = SparseSet::new();
            sparse_set.add_group(&[]);
            sparse_set
                .insert(EntityId::new(64), I32(1), TrackingTimestamp::new(0))
                .assert_inserted();
            sparse_set
        }

        fn assert_delegates(storage: &mut dyn Storage) {
            let mut pages = Vec::new();
            let mut groups = Vec::new();
            storage.collect_regroup_pages(
                &mut |page_index, mask| pages.push((page_index, mask)),
                &mut |group| groups.push(group.to_vec()),
            );

            assert_eq!(pages, [(2, 1)]);
            assert_eq!(groups, [Vec::<TypeId>::new()]);
        }

        assert_delegates(&mut NonSend(pending_sparse_set()));
        assert_delegates(&mut NonSync(pending_sparse_set()));
        assert_delegates(&mut NonSendSync(pending_sparse_set()));
    }

    #[test]
    fn remove() {
        let mut array = SparseSet::new();
        array
            .insert(
                EntityId::new_from_parts(0, 0),
                STR("0"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        array
            .insert(
                EntityId::new_from_parts(5, 0),
                STR("5"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        array
            .insert(
                EntityId::new_from_parts(10, 0),
                STR("10"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();

        assert_eq!(
            array.dyn_remove(EntityId::new_from_parts(0, 0), TrackingTimestamp::new(0)),
            Some(STR("0")),
        );
        assert_eq!(
            array.unclassified_bucket.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0)
            ]
        );
        assert_eq!(array.unclassified_bucket.data, &[STR("10"), STR("5")]);
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0)),
            Some(&STR("10"))
        );

        array
            .insert(
                EntityId::new_from_parts(3, 0),
                STR("3"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        array
            .insert(
                EntityId::new_from_parts(100, 0),
                STR("100"),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
        assert_eq!(
            array.unclassified_bucket.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0),
                EntityId::new_from_parts(3, 0),
                EntityId::new_from_parts(100, 0)
            ]
        );
        assert_eq!(
            array.unclassified_bucket.data,
            &[STR("10"), STR("5"), STR("3"), STR("100")]
        );
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(3, 0)),
            Some(&STR("3"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0)),
            Some(&STR("10"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(100, 0)),
            Some(&STR("100"))
        );

        assert_eq!(
            array.dyn_remove(EntityId::new_from_parts(3, 0), TrackingTimestamp::new(0)),
            Some(STR("3")),
        );
        assert_eq!(
            array.unclassified_bucket.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0),
                EntityId::new_from_parts(100, 0)
            ]
        );
        assert_eq!(
            array.unclassified_bucket.data,
            &[STR("10"), STR("5"), STR("100")]
        );
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
        assert_eq!(array.private_get(EntityId::new_from_parts(3, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0)),
            Some(&STR("10"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(100, 0)),
            Some(&STR("100"))
        );

        assert_eq!(
            array.dyn_remove(EntityId::new_from_parts(100, 0), TrackingTimestamp::new(0)),
            Some(STR("100"))
        );
        assert_eq!(
            array.unclassified_bucket.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0)
            ]
        );
        assert_eq!(array.unclassified_bucket.data, &[STR("10"), STR("5")]);
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
        assert_eq!(array.private_get(EntityId::new_from_parts(3, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0)),
            Some(&STR("10"))
        );
        assert_eq!(array.private_get(EntityId::new_from_parts(100, 0)), None);
    }

    #[test]
    fn remove_grouped_component_returns_and_queues_exact_entity() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, I32>, ViewMut<'_, STR>)>()
                .unwrap();
            views.create_group();
        }

        let entity = world.add_entity((I32(10), STR("grouped")));
        world.regroup();

        assert_eq!(world.remove::<(I32,)>(entity), (Some(I32(10)),));

        let view = world.borrow::<View<'_, I32>>().unwrap();
        assert_eq!(view.sparse_set.sparse.removed_from_groups, [(entity, 0)]);
        assert!(view
            .sparse_set
            .sparse
            .bucket_index(entity)
            .is_unclassified());
    }

    #[test]
    fn delete_grouped_component_queues_exact_entity_and_keeps_tracking_data() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, TrackedI32>, ViewMut<'_, STR>)>()
                .unwrap();
            views.create_group();
        }

        let removed = world.add_entity((TrackedI32(10), STR("removed")));
        let deleted = world.add_entity((TrackedI32(20), STR("deleted")));
        world.regroup();

        assert_eq!(
            world.remove::<(TrackedI32,)>(removed),
            (Some(TrackedI32(10)),)
        );
        world.delete_component::<(TrackedI32,)>(deleted);

        let view = world.borrow::<View<'_, TrackedI32>>().unwrap();
        assert_eq!(
            view.sparse_set.sparse.removed_from_groups,
            [(removed, 0), (deleted, 0)]
        );
        assert_eq!(view.sparse_set.removal_data.len(), 1);
        assert_eq!(view.sparse_set.removal_data[0].0, removed);
        assert_eq!(view.sparse_set.deletion_data.len(), 1);
        assert_eq!(view.sparse_set.deletion_data[0].0, deleted);
        assert_eq!(view.sparse_set.deletion_data[0].2, TrackedI32(20));
    }

    #[test]
    fn unclassified_remove_does_not_queue_regroup_record() {
        let mut sparse_set = SparseSet::<I32>::new();
        let entity = EntityId::new(0);

        sparse_set
            .insert(entity, I32(10), TrackingTimestamp::new(0))
            .assert_inserted();

        assert_eq!(
            sparse_set.dyn_remove(entity, TrackingTimestamp::new(1)),
            Some(I32(10))
        );
        assert!(sparse_set.sparse.removed_from_groups.is_empty());
    }

    #[test]
    fn grouped_remove_repairs_swapped_sparse_index_and_tracking_vectors() {
        let mut sparse_set = SparseSet::<TrackedI32>::new();
        let group = [TypeId::of::<SparseSet<TrackedI32>>()];
        sparse_set.add_group(&group);

        let first = EntityId::new(0);
        let second = EntityId::new(1);
        sparse_set
            .insert(first, TrackedI32(10), TrackingTimestamp::new(1))
            .assert_inserted();
        sparse_set
            .insert(second, TrackedI32(20), TrackingTimestamp::new(2))
            .assert_inserted();
        Storage::move_to_group(&mut sparse_set, first, &group);
        Storage::move_to_group(&mut sparse_set, second, &group);

        let insertion = sparse_set.group_buckets[0].insertion_data[1];
        let modification = sparse_set.group_buckets[0].modification_data[1];

        assert_eq!(
            sparse_set.dyn_remove(first, TrackingTimestamp::new(3)),
            Some(TrackedI32(10))
        );
        assert_eq!(sparse_set.group_buckets[0].dense, [second]);
        assert_eq!(sparse_set.group_buckets[0].data, [TrackedI32(20)]);
        assert_eq!(sparse_set.group_buckets[0].insertion_data.len(), 1);
        assert_eq!(sparse_set.group_buckets[0].modification_data.len(), 1);
        assert_eq!(
            sparse_set.group_buckets[0].insertion_data[0].get(),
            insertion.get()
        );
        assert_eq!(
            sparse_set.group_buckets[0].modification_data[0].get(),
            modification.get()
        );
        assert_eq!(sparse_set.sparse.get(second).unwrap().uindex(), 0);
    }

    #[test]
    fn grouped_remove_resets_bucket_metadata_for_later_insertion() {
        let mut sparse_set = SparseSet::<I32>::new();
        let group = [TypeId::of::<SparseSet<I32>>()];
        sparse_set.add_group(&group);

        let old_entity = EntityId::new_from_parts(0, 0);
        sparse_set
            .insert(old_entity, I32(10), TrackingTimestamp::new(0))
            .assert_inserted();
        Storage::move_to_group(&mut sparse_set, old_entity, &group);

        assert_eq!(
            sparse_set.dyn_remove(old_entity, TrackingTimestamp::new(1)),
            Some(I32(10))
        );
        assert!(sparse_set.sparse.bucket_index(old_entity).is_unclassified());

        let new_entity = EntityId::new_from_parts(0, 1);
        sparse_set
            .insert(new_entity, I32(20), TrackingTimestamp::new(2))
            .assert_inserted();
        assert!(sparse_set.sparse.bucket_index(new_entity).is_unclassified());
        assert_eq!(sparse_set.unclassified_bucket.dense, [new_entity]);
    }

    #[test]
    fn stale_grouped_remove_queues_stored_generation_without_callback_or_tracking() {
        let mut sparse_set = SparseSet::<TrackedI32>::new();
        let group = [TypeId::of::<SparseSet<TrackedI32>>()];
        sparse_set.add_group(&group);

        let callbacks = Arc::new(Mutex::new(Vec::new()));
        let callback_log = Arc::clone(&callbacks);
        sparse_set.on_removal(move |entity, component| {
            callback_log.lock().unwrap().push((entity, component.0));
        });

        let stored = EntityId::new_from_parts(0, 2);
        let newer = EntityId::new_from_parts(0, 3);
        sparse_set
            .insert(stored, TrackedI32(10), TrackingTimestamp::new(0))
            .assert_inserted();
        Storage::move_to_group(&mut sparse_set, stored, &group);

        assert_eq!(
            sparse_set.dyn_remove(newer, TrackingTimestamp::new(1)),
            None
        );
        assert_eq!(sparse_set.sparse.removed_from_groups, [(stored, 0)]);
        assert!(callbacks.lock().unwrap().is_empty());
        assert!(sparse_set.removal_data.is_empty());

        let exact = EntityId::new_from_parts(1, 4);
        sparse_set
            .insert(exact, TrackedI32(20), TrackingTimestamp::new(2))
            .assert_inserted();
        Storage::move_to_group(&mut sparse_set, exact, &group);

        assert_eq!(
            sparse_set.dyn_remove(exact, TrackingTimestamp::new(3)),
            Some(TrackedI32(20))
        );
        assert_eq!(*callbacks.lock().unwrap(), [(exact, 20)]);
    }

    #[test]
    fn clear() {
        let mut sparse_set = SparseSet::new();
        sparse_set.track_all();

        sparse_set
            .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
            .assert_inserted();

        sparse_set.private_clear(TrackingTimestamp::new(0));

        assert_eq!(sparse_set.len(), 0);
        assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
        assert_eq!(sparse_set.private_get(EntityId::new(1)), None);
        assert_eq!(sparse_set.unclassified_bucket.insertion_data.len(), 0);
        assert_eq!(sparse_set.unclassified_bucket.modification_data.len(), 0);
        assert_eq!(sparse_set.deletion_data.len(), 2);
        assert_eq!(sparse_set.removal_data.len(), 0);
    }

    #[test]
    fn drain() {
        let mut sparse_set = SparseSet::new();
        sparse_set.track_all();

        sparse_set
            .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
            .assert_inserted();

        let mut drain = sparse_set.private_drain(TrackingTimestamp::new(0));

        assert_eq!(drain.next(), Some(I32(0)));
        assert_eq!(drain.next(), Some(I32(1)));
        assert_eq!(drain.next(), None);

        drop(drain);

        assert_eq!(sparse_set.len(), 0);
        assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
        assert_eq!(sparse_set.private_get(EntityId::new(1)), None);
        assert_eq!(sparse_set.unclassified_bucket.insertion_data.len(), 0);
        assert_eq!(sparse_set.unclassified_bucket.modification_data.len(), 0);
        assert_eq!(sparse_set.deletion_data.len(), 0);
        assert_eq!(sparse_set.removal_data.len(), 2);
    }

    #[test]
    fn drain_with_id() {
        let mut sparse_set = SparseSet::new();

        sparse_set
            .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
            .assert_inserted();

        let mut drain = sparse_set
            .private_drain(TrackingTimestamp::new(0))
            .with_id();

        assert_eq!(drain.next(), Some((EntityId::new(0), I32(0))));
        assert_eq!(drain.next(), Some((EntityId::new(1), I32(1))));
        assert_eq!(drain.next(), None);

        drop(drain);

        assert_eq!(sparse_set.len(), 0);
        assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
    }

    #[test]
    fn drain_empty() {
        let mut sparse_set = SparseSet::<I32>::new();

        assert_eq!(
            sparse_set.private_drain(TrackingTimestamp::new(0)).next(),
            None
        );
        assert_eq!(
            sparse_set
                .private_drain(TrackingTimestamp::new(0))
                .with_id()
                .next(),
            None
        );

        assert_eq!(sparse_set.len(), 0);
    }

    #[test]
    fn unstable_sort() {
        let mut array = SparseSet::new();

        for i in (0..100).rev() {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(100 - i);
            array
                .insert(entity_id, I32(i as i32), TrackingTimestamp::new(0))
                .assert_inserted();
        }

        array.sort_unstable();

        for window in array.unclassified_bucket.data.windows(2) {
            assert!(window[0] < window[1]);
        }
        for i in 0..100 {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(100 - i);
            assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
        }
    }

    #[test]
    fn partially_sorted_unstable_sort() {
        let mut array = SparseSet::new();

        for i in 0..20 {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(i);
            array
                .insert(entity_id, I32(i as i32), TrackingTimestamp::new(0))
                .assert_inserted();
        }
        for i in (20..100).rev() {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(100 - i + 20);
            array
                .insert(entity_id, I32(i as i32), TrackingTimestamp::new(0))
                .assert_inserted();
        }

        array.sort_unstable();

        for window in array.unclassified_bucket.data.windows(2) {
            assert!(window[0] < window[1]);
        }
        for i in 0..20 {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(i);
            assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
        }
        for i in 20..100 {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(100 - i + 20);
            assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
        }
    }

    #[test]
    fn unstable_sort_preserves_inserted_tracking() {
        let mut array = SparseSet::new();
        let tracked = EntityId::new_from_parts(0, 0);
        let other = EntityId::new_from_parts(1, 0);
        let another = EntityId::new_from_parts(2, 0);
        let last = TrackingTimestamp::new(1);
        let current = TrackingTimestamp::new(2);

        array
            .insert(tracked, TrackedI32(3), current)
            .assert_inserted();
        array.insert(other, TrackedI32(1), last).assert_inserted();
        array.insert(another, TrackedI32(2), last).assert_inserted();

        assert!(<crate::track::All as Tracking>::is_inserted(
            &array, tracked, last, current
        ));
        assert!(!<crate::track::All as Tracking>::is_inserted(
            &array, other, last, current
        ));

        array.sort_unstable();

        assert!(<crate::track::All as Tracking>::is_inserted(
            &array, tracked, last, current
        ));
        assert!(!<crate::track::All as Tracking>::is_inserted(
            &array, other, last, current
        ));
        assert!(!<crate::track::All as Tracking>::is_inserted(
            &array, another, last, current
        ));
    }

    #[test]
    fn unstable_sort_preserves_modified_tracking() {
        let mut array = SparseSet::new();
        let tracked = EntityId::new_from_parts(0, 0);
        let other = EntityId::new_from_parts(1, 0);
        let another = EntityId::new_from_parts(2, 0);
        let last = TrackingTimestamp::new(1);
        let current = TrackingTimestamp::new(2);

        array
            .insert(tracked, TrackedI32(3), TrackingTimestamp::origin())
            .assert_inserted();
        array
            .insert(other, TrackedI32(1), TrackingTimestamp::origin())
            .assert_inserted();
        array
            .insert(another, TrackedI32(2), TrackingTimestamp::origin())
            .assert_inserted();

        let _ = array.insert(tracked, TrackedI32(3), current);

        assert!(<crate::track::All as Tracking>::is_modified(
            &array, tracked, last, current
        ));
        assert!(!<crate::track::All as Tracking>::is_modified(
            &array, other, last, current
        ));

        array.sort_unstable();

        assert!(<crate::track::All as Tracking>::is_modified(
            &array, tracked, last, current
        ));
        assert!(!<crate::track::All as Tracking>::is_modified(
            &array, other, last, current
        ));
        assert!(!<crate::track::All as Tracking>::is_modified(
            &array, another, last, current
        ));
    }

    #[test]
    fn debug() {
        let mut sparse_set = SparseSet::new();

        sparse_set
            .insert(EntityId::new(0), STR("0"), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(5), STR("5"), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(10), STR("10"), TrackingTimestamp::new(0))
            .assert_inserted();

        println!("{:#?}", sparse_set);
    }

    #[test]
    fn multiple_enable_tracking() {
        let mut sparse_set = SparseSet::new();

        sparse_set
            .insert(
                EntityId::new_from_parts(0, 0),
                I32(0),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();

        sparse_set.track_all();
        sparse_set.track_all();
        sparse_set.track_all();

        assert_eq!(sparse_set.unclassified_bucket.insertion_data.len(), 1);
        assert_eq!(sparse_set.unclassified_bucket.modification_data.len(), 1);
    }

    fn consume_pending_without_regrouping<T: Component>(sparse_set: &mut SparseSet<T>) {
        sparse_set.sparse.pending_placement_masks.fill(0);
        sparse_set.sparse.pending_placement_pages.clear();
    }

    #[test]
    fn overlapping_groups_inserted_together_form_union() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, C>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, B, C));
        world.regroup();

        let expected = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
        ]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, entity), expected);
        assert_eq!(current_group(views.1.sparse_set, entity), expected);
        assert_eq!(current_group(views.2.sparse_set, entity), expected);
    }

    #[test]
    fn deleted_pending_entity_is_discarded_and_pending_masks_are_consumed() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, B));
        assert!(world.delete_entity(entity));
        world.regroup();

        let views = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        assert!(!views.0.contains(entity));
        assert!(!views.1.contains(entity));
        assert!(views
            .0
            .sparse_set
            .sparse
            .pending_placement_pages()
            .is_empty());
        assert!(views
            .1
            .sparse_set
            .sparse
            .pending_placement_pages()
            .is_empty());
        assert_eq!(views.0.sparse_set.sparse.pending_placement_mask(0), 0);
        assert_eq!(views.1.sparse_set.sparse.pending_placement_mask(0), 0);
    }

    #[test]
    fn regroup_uses_the_current_generation_for_a_recycled_pending_index() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }

        let previous = world.add_entity((A, B));
        assert!(world.delete_entity(previous));
        let current = world.add_entity((A, B));
        assert_eq!(current.index(), previous.index());
        assert_ne!(current.gen(), previous.gen());

        world.regroup();

        let expected = sorted_group(vec![storage_id::<A>(), storage_id::<B>()]);
        let views = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        assert_eq!(current_group(views.0.sparse_set, current), expected);
        assert_eq!(current_group(views.1.sparse_set, current), expected);
    }

    #[test]
    fn existing_group_expands_when_only_new_component_is_pending() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, C>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, C));
        world.regroup();
        world.add_component(entity, (B,));
        world.regroup();

        let expected = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
        ]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, entity), expected);
        assert_eq!(current_group(views.1.sparse_set, entity), expected);
        assert_eq!(current_group(views.2.sparse_set, entity), expected);
    }

    #[test]
    fn existing_encompassing_group_is_reused_and_same_target_is_noop() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
                .unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, B, C));
        world.regroup();

        let expected = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
        ]);

        {
            let view = world.borrow::<ViewMut<'_, A>>().unwrap();
            assert_eq!(view.sparse_set.groups.iter().count(), 1);
            assert_eq!(view.sparse_set.group_buckets[0].dense, [entity]);

            Storage::move_to_group(view.sparse_set, entity, &expected);

            assert_eq!(view.sparse_set.groups.iter().count(), 1);
            assert_eq!(view.sparse_set.group_buckets[0].dense, [entity]);
        }

        world.regroup();

        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();
        assert_eq!(views.0.sparse_set.groups.iter().count(), 1);
        assert_eq!(views.1.sparse_set.groups.iter().count(), 1);
        assert_eq!(views.2.sparse_set.groups.iter().count(), 1);
    }

    #[test]
    fn chained_overlaps_merge_transitively() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, B>, ViewMut<'_, C>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, C>, ViewMut<'_, D>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, B, C, D));
        world.regroup();

        let expected = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
            storage_id::<D>(),
        ]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, entity), expected);
        assert_eq!(current_group(views.1.sparse_set, entity), expected);
        assert_eq!(current_group(views.2.sparse_set, entity), expected);
        assert_eq!(current_group(views.3.sparse_set, entity), expected);
    }

    #[test]
    fn disjoint_matches_remain_separate() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, C>, ViewMut<'_, D>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, B, C, D));
        world.regroup();

        let ab = sorted_group(vec![storage_id::<A>(), storage_id::<B>()]);
        let cd = sorted_group(vec![storage_id::<C>(), storage_id::<D>()]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, entity), ab);
        assert_eq!(current_group(views.1.sparse_set, entity), ab);
        assert_eq!(current_group(views.2.sparse_set, entity), cd);
        assert_eq!(current_group(views.3.sparse_set, entity), cd);
    }

    #[test]
    fn current_bucket_group_bridges_collected_candidates() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, B>, ViewMut<'_, C>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, C>, ViewMut<'_, D>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((B, C));
        world.regroup();
        world.add_component(entity, (A, D));
        world.regroup();

        let expected = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
            storage_id::<D>(),
        ]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, entity), expected);
        assert_eq!(current_group(views.1.sparse_set, entity), expected);
        assert_eq!(current_group(views.2.sparse_set, entity), expected);
        assert_eq!(current_group(views.3.sparse_set, entity), expected);
    }

    #[test]
    fn dynamically_created_group_is_reused_by_later_entities() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, B>, ViewMut<'_, C>)>().unwrap();
            views.create_group();
        }

        let first = world.add_entity((A, B, C));
        world.regroup();

        let group_counts = {
            let views = world
                .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
                .unwrap();
            (
                views.0.sparse_set.groups.iter().count(),
                views.1.sparse_set.groups.iter().count(),
                views.2.sparse_set.groups.iter().count(),
            )
        };

        let second = world.add_entity((A, B, C));
        world.regroup();

        let expected = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
        ]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, first), expected);
        assert_eq!(current_group(views.0.sparse_set, second), expected);
        assert_eq!(views.0.sparse_set.groups.iter().count(), group_counts.0);
        assert_eq!(views.1.sparse_set.groups.iter().count(), group_counts.1);
        assert_eq!(views.2.sparse_set.groups.iter().count(), group_counts.2);
    }

    #[test]
    fn regroup_batch_reuses_one_group_for_all_entities() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }

        let entities: Vec<_> = (0..16).map(|_| world.add_entity((A, B))).collect();
        world.regroup();

        let views = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        assert_eq!(views.0.sparse_set.groups.iter().count(), 1);
        assert_eq!(views.1.sparse_set.groups.iter().count(), 1);
        assert_eq!(views.0.sparse_set.group_buckets[0].dense, entities);
        assert_eq!(views.1.sparse_set.group_buckets[0].dense, entities);
    }

    #[test]
    fn movement_from_existing_group_repairs_swapped_sparse_indices() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, C>)>().unwrap();
            views.create_group();
        }
        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }

        let first = world.add_entity((A, C));
        let second = world.add_entity((A, C));
        world.regroup();

        let (moving, remaining) = {
            let view = world.borrow::<View<'_, A>>().unwrap();
            let dense = &view.sparse_set.group_buckets[0].dense;
            assert_eq!(dense.len(), 2);
            (dense[0], dense[1])
        };
        assert!([first, second].contains(&moving));

        world.add_component(moving, (B,));
        world.regroup();

        let ac = sorted_group(vec![storage_id::<A>(), storage_id::<C>()]);
        let abc = sorted_group(vec![
            storage_id::<A>(),
            storage_id::<B>(),
            storage_id::<C>(),
        ]);
        let views = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();

        assert_eq!(current_group(views.0.sparse_set, moving), abc);
        assert_eq!(current_group(views.1.sparse_set, moving), abc);
        assert_eq!(current_group(views.2.sparse_set, moving), abc);
        assert_eq!(current_group(views.0.sparse_set, remaining), ac);
        assert_eq!(current_group(views.2.sparse_set, remaining), ac);
        assert_eq!(
            views.0.sparse_set.sparse.get(remaining).unwrap().uindex(),
            0
        );
        assert_eq!(
            views.2.sparse_set.sparse.get(remaining).unwrap().uindex(),
            0
        );
        assert_eq!(views.0.sparse_set.group_buckets[0].dense, [remaining]);
        assert_eq!(views.2.sparse_set.group_buckets[0].dense, [remaining]);
    }

    #[test]
    fn empty_group_movement_returns_to_unclassified_without_creating_a_group() {
        let mut world = World::new();

        {
            let mut views = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
            views.create_group();
        }

        let entity = world.add_entity((A, B));
        world.regroup();

        let view = world.borrow::<ViewMut<'_, A>>().unwrap();
        assert_eq!(view.sparse_set.groups.iter().count(), 1);
        assert_eq!(view.sparse_set.group_buckets[0].dense, [entity]);

        Storage::move_to_group(view.sparse_set, entity, &[]);

        assert_eq!(view.sparse_set.groups.iter().count(), 1);
        assert_eq!(view.sparse_set.group_buckets.len(), 1);
        assert!(view.sparse_set.group_buckets[0].dense.is_empty());
        assert_eq!(view.sparse_set.unclassified_bucket.dense, [entity]);
        assert!(view
            .sparse_set
            .sparse
            .bucket_index(entity)
            .is_unclassified());
    }

    #[test]
    fn empty_group_movement_preserves_component_tracking() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, TrackedI32>, ViewMut<'_, STR>)>()
                .unwrap();
            views.create_group();
        }

        let entity = world.add_entity((TrackedI32(10), STR("grouped")));
        world.regroup();

        let (insertion, modification) = {
            let view = world.borrow::<View<'_, TrackedI32>>().unwrap();
            assert_eq!(view.sparse_set.group_buckets[0].data, [TrackedI32(10)]);
            (
                view.sparse_set.group_buckets[0].insertion_data[0],
                view.sparse_set.group_buckets[0].modification_data[0],
            )
        };

        let view = world.borrow::<ViewMut<'_, TrackedI32>>().unwrap();
        Storage::move_to_group(view.sparse_set, entity, &[]);

        assert_eq!(view.sparse_set.unclassified_bucket.dense, [entity]);
        assert_eq!(view.sparse_set.unclassified_bucket.data, [TrackedI32(10)]);
        assert_eq!(
            view.sparse_set.unclassified_bucket.insertion_data[0].get(),
            insertion.get()
        );
        assert_eq!(
            view.sparse_set.unclassified_bucket.modification_data[0].get(),
            modification.get()
        );
    }

    #[test]
    fn empty_group_movement_repairs_the_source_bucket_sparse_index() {
        let mut sparse_set = SparseSet::<I32>::new();
        let group = [TypeId::of::<A>()];
        sparse_set.add_group(&group);

        let first = EntityId::new(0);
        let second = EntityId::new(1);
        sparse_set
            .insert(first, I32(10), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(second, I32(20), TrackingTimestamp::new(0))
            .assert_inserted();
        Storage::move_to_group(&mut sparse_set, first, &group);
        Storage::move_to_group(&mut sparse_set, second, &group);

        Storage::move_to_group(&mut sparse_set, first, &[]);

        assert_eq!(sparse_set.group_buckets[0].dense, [second]);
        assert_eq!(sparse_set.sparse.get(second).unwrap().uindex(), 0);
        assert_eq!(sparse_set.unclassified_bucket.dense, [first]);
        assert_eq!(sparse_set.unclassified_bucket.data, [I32(10)]);
    }

    /// Checks that a 3 storage group is correctly populated after a regroup.
    #[test]
    fn regroup_three_components() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, STR>, ViewMut<'_, I32>, ViewMut<'_, TrackedI32>)>()
                .unwrap();
            views.create_group();
        }

        let entity = world.add_entity((STR("a"), I32(1), TrackedI32(2)));

        {
            let views = world
                .borrow::<(ViewMut<'_, STR>, ViewMut<'_, I32>, ViewMut<'_, TrackedI32>)>()
                .unwrap();
            consume_pending_without_regrouping(views.1.sparse_set);
            consume_pending_without_regrouping(views.2.sparse_set);
        }

        world.regroup();

        let views = world
            .borrow::<(View<'_, STR>, View<'_, I32>, View<'_, TrackedI32>)>()
            .unwrap();

        for sparse in [
            &views.0.sparse_set.sparse,
            &views.1.sparse_set.sparse,
            &views.2.sparse_set.sparse,
        ] {
            assert_eq!(sparse.bucket_index(entity).group_index(), Some(0));
            assert!(sparse.pending_placement_pages().is_empty());
        }
        assert_eq!(views.0.sparse_set.group_buckets[0].dense, [entity]);
        assert_eq!(views.1.sparse_set.group_buckets[0].dense, [entity]);
        assert_eq!(views.2.sparse_set.group_buckets[0].dense, [entity]);
    }

    /// Checks that groups placed at different indices still work.
    #[test]
    fn regroup_different_local_indices() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, STR>, ViewMut<'_, TrackedI32>)>()
                .unwrap();
            views.create_group();
        }
        {
            let mut views = world
                .borrow::<(ViewMut<'_, STR>, ViewMut<'_, I32>)>()
                .unwrap();
            views.create_group();
        }

        let entity = world.add_entity((STR("a"), I32(1)));

        {
            let view = world.borrow::<ViewMut<'_, I32>>().unwrap();
            consume_pending_without_regrouping(view.sparse_set);
        }

        world.regroup();

        let views = world.borrow::<(View<'_, STR>, View<'_, I32>)>().unwrap();
        assert_eq!(
            views.0.sparse_set.sparse.bucket_index(entity).group_index(),
            Some(1)
        );
        assert_eq!(
            views.1.sparse_set.sparse.bucket_index(entity).group_index(),
            Some(0)
        );
        assert_eq!(views.0.sparse_set.group_buckets[1].dense, [entity]);
        assert_eq!(views.1.sparse_set.group_buckets[0].dense, [entity]);
    }

    /// Checks that a 3 storages group can be populated in two steps.
    ///
    /// The first step inserts 2 components then the last component is inserted later.
    #[test]
    fn missing_component_consumes_pending_then_final_component_places_cohort() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, STR>, ViewMut<'_, I32>, ViewMut<'_, TrackedI32>)>()
                .unwrap();
            views.create_group();
        }

        let entity = world.add_entity((STR("a"), I32(1)));
        world.regroup();

        {
            let views = world.borrow::<(View<'_, STR>, View<'_, I32>)>().unwrap();
            assert!(views
                .0
                .sparse_set
                .sparse
                .pending_placement_pages()
                .is_empty());
            assert!(views
                .1
                .sparse_set
                .sparse
                .pending_placement_pages()
                .is_empty());
            assert!(views
                .0
                .sparse_set
                .sparse
                .bucket_index(entity)
                .is_unclassified());
            assert!(views
                .1
                .sparse_set
                .sparse
                .bucket_index(entity)
                .is_unclassified());
        }

        world.add_component(entity, (TrackedI32(2),));
        world.regroup();

        let views = world
            .borrow::<(View<'_, STR>, View<'_, I32>, View<'_, TrackedI32>)>()
            .unwrap();
        assert_eq!(
            views.0.sparse_set.sparse.bucket_index(entity).group_index(),
            Some(0)
        );
        assert_eq!(
            views.1.sparse_set.sparse.bucket_index(entity).group_index(),
            Some(0)
        );
        assert_eq!(
            views.2.sparse_set.sparse.bucket_index(entity).group_index(),
            Some(0)
        );
    }

    /// Checks that regroup transfer tracking info.
    #[test]
    fn regroup_preserves_tracking() {
        let mut world = World::new();

        {
            let mut views = world
                .borrow::<(ViewMut<'_, TrackedI32>, ViewMut<'_, STR>)>()
                .unwrap();
            views.create_group();
        }

        let grouped = world.add_entity((TrackedI32(10), STR("grouped")));
        let remaining = world.add_entity(TrackedI32(20));

        world.get::<&mut TrackedI32>(grouped).unwrap().0 += 1;

        let (grouped_insert, grouped_modified, remaining_insert, remaining_modified) = {
            let view = world.borrow::<View<'_, TrackedI32>>().unwrap();
            (
                view.sparse_set.unclassified_bucket.insertion_data[0].get(),
                view.sparse_set.unclassified_bucket.modification_data[0].get(),
                view.sparse_set.unclassified_bucket.insertion_data[1].get(),
                view.sparse_set.unclassified_bucket.modification_data[1].get(),
            )
        };
        assert_ne!(grouped_insert, 0);
        assert_ne!(grouped_modified, 0);

        world.regroup();

        let view = world.borrow::<View<'_, TrackedI32>>().unwrap();
        assert_eq!(view.sparse_set.group_buckets[0].dense, [grouped]);
        assert_eq!(view.sparse_set.group_buckets[0].data, [TrackedI32(11)]);
        assert_eq!(
            view.sparse_set.group_buckets[0].insertion_data[0].get(),
            grouped_insert
        );
        assert_eq!(
            view.sparse_set.group_buckets[0].modification_data[0].get(),
            grouped_modified
        );
        assert_eq!(view.sparse_set.unclassified_bucket.dense, [remaining]);
        assert_eq!(view.sparse_set.unclassified_bucket.data, [TrackedI32(20)]);
        assert_eq!(
            view.sparse_set.unclassified_bucket.insertion_data[0].get(),
            remaining_insert
        );
        assert_eq!(
            view.sparse_set.unclassified_bucket.modification_data[0].get(),
            remaining_modified
        );
        assert_eq!(view.sparse_set.sparse.get(remaining).unwrap().uindex(), 0);
    }
}
