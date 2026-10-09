mod add_component;
mod bulk_add_entity;
mod clone;
mod delete;
mod drain;
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
#[doc(hidden)]
pub use groups::{GroupRegion, Groups};
pub use memory_usage::{SparseSetMemory, SparseSetMemoryUsage};
pub use remove::TupleRemove;
pub use sparse_array::SparseArray;
#[doc(hidden)]
pub use window::RawEntityIdAccess;

pub(crate) use groups::GroupRange;
pub(crate) use sparse_array::BUCKET_SIZE;
pub(crate) use window::{FullRawWindow, FullRawWindowMut};

use crate::all_storages::AllStorages;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::error;
use crate::memory_usage::StorageMemoryUsage;
use crate::r#mut::Mut;
use crate::storage::{SBoxBuilder, Storage, StorageId};
use crate::tracking::{Tracking, TrackingTimestamp};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use clone::SparseSetCloneFunctions;
use core::any::type_name;
use core::{
    cmp::{Ord, Ordering},
    fmt,
};

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
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) insertion_data: Vec<TrackingTimestamp>,
    pub(crate) modification_data: Vec<TrackingTimestamp>,
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
    clone: Option<SparseSetCloneFunctions<T>>,
    pub(crate) groups: Groups,
}

impl<T: fmt::Debug + Component> fmt::Debug for SparseSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.dense.iter().zip(&self.data))
            .finish()
    }
}

impl<T: Component> SparseSet<T> {
    #[inline]
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: SparseArray::new(),
            dense: Vec::new(),
            data: Vec::new(),
            insertion_data: Vec::new(),
            modification_data: Vec::new(),
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
            groups: Groups::default(),
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
        &self.data
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
        self.dense.len()
    }
    /// Returns true if the storage's length is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }
}

impl<T: Component> SparseSet<T> {
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.\
    /// This index is only valid for this storage and until a modification happens.
    #[inline]
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        if let Some(sparse_entity) = self.sparse.get(entity) {
            if entity.gen() == sparse_entity.gen() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        } else {
            None
        }
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
        self.dense.get(index).copied()
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
        if let Some(index) = self.index_of(entity) {
            Some(unsafe { self.data.get_unchecked(index) })
        } else {
            None
        }
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

        if sparse_entity.is_dead() {
            if let Some(on_insertion) = &mut self.on_insertion {
                on_insertion(entity, &value);
            }

            *sparse_entity =
                EntityId::new_from_index_and_gen(self.dense.len() as u64, entity.gen());

            if self.is_tracking_insertion {
                self.insertion_data.push(current);
            }
            if self.is_tracking_modification {
                self.modification_data.push(TrackingTimestamp::origin());
            }

            self.dense.push(entity);
            self.data.push(value);

            old_component = InsertionResult::Inserted;
        } else if entity.gen() == sparse_entity.gen() {
            if let Some(on_insertion) = &mut self.on_insertion {
                on_insertion(entity, &value);
            }

            let old_data = unsafe {
                core::mem::replace(self.data.get_unchecked_mut(sparse_entity.uindex()), value)
            };

            old_component = InsertionResult::ComponentOverride(old_data);

            sparse_entity.copy_gen(entity);

            let dense_entity = unsafe { self.dense.get_unchecked_mut(sparse_entity.uindex()) };

            if self.is_tracking_modification {
                unsafe {
                    *self
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
                core::mem::replace(self.data.get_unchecked_mut(sparse_entity.uindex()), value)
            };

            old_component = InsertionResult::OtherComponentOverride;

            sparse_entity.copy_gen(entity);

            let dense_entity = unsafe { self.dense.get_unchecked_mut(sparse_entity.uindex()) };

            if self.is_tracking_insertion {
                unsafe {
                    *self
                        .insertion_data
                        .get_unchecked_mut(sparse_entity.uindex()) = current;
                }
            }

            dense_entity.copy_index_gen(entity);

            // A different entity now sits in this slot, so a range holding it isn't aligned anymore.
            self.reset_groups();
        } else {
            old_component = InsertionResult::NotInserted;
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
            self.reset_groups();

            unsafe {
                *self.sparse.get_mut_unchecked(entity) = EntityId::dead();
            }

            self.dense.swap_remove(sparse_entity.uindex());
            if self.is_tracking_insertion() {
                self.insertion_data.swap_remove(sparse_entity.uindex());
            }
            if self.is_tracking_modification() {
                self.modification_data.swap_remove(sparse_entity.uindex());
            }
            let component = self.data.swap_remove(sparse_entity.uindex());

            // The SparseSet could now be empty or the removed component could have been the last one
            if sparse_entity.uindex() < self.dense.len() {
                unsafe {
                    let last = *self.dense.get_unchecked(sparse_entity.uindex());
                    self.sparse
                        .get_mut_unchecked(last)
                        .copy_index(sparse_entity);
                }
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

    /// Makes the whole storage unevaluated if it has any group range.
    ///
    /// Called before any mutation that changes the layout of the storage, see [`Groups::reset`].
    #[inline]
    fn reset_groups(&mut self) {
        // Most storages have no group, we keep their cost to this single branch.
        if !self.groups.has_ranges() {
            return;
        }

        self.groups.reset();

        #[cfg(debug_assertions)]
        self.groups.assert_invariants(self.dense.len());
    }
}

impl<T: Component> SparseSet<T> {
    /// Swaps the components at physical indices `i` and `j`, along with their entities and timestamps.
    ///
    /// Both entities' sparse entries are updated to their new index.
    fn swap_rows(&mut self, i: usize, j: usize) {
        if i == j {
            return;
        }

        self.dense.swap(i, j);
        self.data.swap(i, j);
        if self.is_tracking_insertion {
            self.insertion_data.swap(i, j);
        }
        if self.is_tracking_modification {
            self.modification_data.swap(i, j);
        }

        let entity_i = self.dense[i];
        let entity_j = self.dense[j];

        // SAFE: both entities were just read from `dense`, so they own a component in this storage
        // and their sparse page is allocated.
        unsafe {
            self.sparse.get_mut_unchecked(entity_i).set_index(i as u64);
            self.sparse.get_mut_unchecked(entity_j).set_index(j as u64);
        }
    }

    /// Moves `entity`'s component to `destination`, wherever it currently is in this storage.
    ///
    /// A move into a range appends the component at the range's end.\
    /// Timestamps move with the component. A move isn't a modification and doesn't call any callback.
    ///
    /// ### Panics
    ///
    /// - `entity` doesn't have a component in this storage.
    /// - `destination` is a group without a range in this storage.
    #[track_caller]
    pub(crate) fn private_move_to(&mut self, entity: EntityId, destination: GroupRegion<'_>) {
        let index = self
            .index_of(entity)
            .expect("only components present in the storage can be moved");

        if let GroupRegion::Range(group) = destination {
            assert!(
                self.groups.range(group).is_some(),
                "the destination group has no range in this storage"
            );
        }

        // The ungrouped region sits between the range and the unevaluated region,
        // so every move is made of steps into or out of it.
        // With a single range per storage, the range always borders the ungrouped region,
        // and the step helpers below rely on that.
        match (self.groups.region_of(index), destination) {
            (GroupRegion::Range(source), GroupRegion::Range(group)) => {
                if !Arc::ptr_eq(source, group) {
                    unreachable!(
                        "moving a component between two group ranges requires several ranges per storage, which isn't supported yet"
                    );
                }
            }
            (GroupRegion::Range(_), GroupRegion::Ungrouped) => {
                self.range_to_ungrouped(index);
            }
            (GroupRegion::Range(_), GroupRegion::Unevaluated) => {
                let index = self.range_to_ungrouped(index);
                self.ungrouped_to_unevaluated(index);
            }
            (GroupRegion::Ungrouped, GroupRegion::Range(_)) => {
                self.ungrouped_to_range(index);
            }
            (GroupRegion::Ungrouped, GroupRegion::Unevaluated) => {
                self.ungrouped_to_unevaluated(index);
            }
            (GroupRegion::Unevaluated, GroupRegion::Range(_)) => {
                let index = self.unevaluated_to_ungrouped(index);
                self.ungrouped_to_range(index);
            }
            (GroupRegion::Unevaluated, GroupRegion::Ungrouped) => {
                self.unevaluated_to_ungrouped(index);
            }
            (GroupRegion::Ungrouped, GroupRegion::Ungrouped)
            | (GroupRegion::Unevaluated, GroupRegion::Unevaluated) => {}
        }

        #[cfg(debug_assertions)]
        self.groups.assert_invariants(self.dense.len());
    }

    /// Moves the component at `index`, inside the last range, to the ungrouped region.\
    /// Returns its new index.
    fn range_to_ungrouped(&mut self, index: usize) -> usize {
        // We swap it with the range's last component, then shrink the range to leave it out.
        let new_index = self.groups.grouped_end() - 1;
        self.swap_rows(index, new_index);

        let range = self
            .groups
            .ranges
            .last_mut()
            .expect("the component is inside a range");
        debug_assert!(range.start <= index);

        range.len -= 1;
        // Only positions from `index` onward changed, so at most the ones before it are still trusted.
        range.trust = range.trust.min(index - range.start);

        new_index
    }

    /// Appends the ungrouped component at `index` to the last range.
    fn ungrouped_to_range(&mut self, index: usize) {
        // We swap it with the first ungrouped component, then grow the range over it.
        let new_index = self.groups.grouped_end();
        self.swap_rows(index, new_index);

        let range = self
            .groups
            .ranges
            .last_mut()
            .expect("the destination range exists");

        // Every member appends the same entity at the same logical end,
        // so a fully trusted range stays fully trusted.
        if range.trust == range.len {
            range.trust += 1;
        }
        range.len += 1;
    }

    /// Moves the ungrouped component at `index` to the unevaluated region.
    fn ungrouped_to_unevaluated(&mut self, index: usize) {
        // We swap it with the last ungrouped component, then grow the unevaluated region over it.
        let new_index = self.groups.unevaluated_start - 1;
        self.swap_rows(index, new_index);

        self.groups.unevaluated_start = new_index;
    }

    /// Moves the unevaluated component at `index` to the ungrouped region.
    /// Returns its new index.
    fn unevaluated_to_ungrouped(&mut self, index: usize) -> usize {
        // We swap it with the first unevaluated component, then grow the ungrouped region over it.
        let new_index = self.groups.unevaluated_start;
        self.swap_rows(index, new_index);

        self.groups.unevaluated_start += 1;

        new_index
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

        self.insertion_data
            .extend(core::iter::repeat(TrackingTimestamp::new(0)).take(self.dense.len()));

        self
    }
    /// Make this storage track modification.
    #[allow(clippy::manual_repeat_n, reason = "Too recent version")]
    pub fn track_modification(&mut self) -> &mut SparseSet<T> {
        if self.is_tracking_modification() {
            return self;
        }

        self.is_tracking_modification = true;

        self.modification_data
            .extend(core::iter::repeat(TrackingTimestamp::new(0)).take(self.dense.len()));

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
        self.dense.reserve(additional);
        self.data.reserve(additional);
    }
    /// Sorts the `SparseSet` with a comparator function, but may not preserve the order of equal elements.
    pub fn sort_unstable_by<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        self.reset_groups();

        let mut transform: Vec<usize> = (0..self.dense.len()).collect();

        transform.sort_unstable_by(|&i, &j| {
            // SAFE dense and data have the same length
            compare(unsafe { self.data.get_unchecked(i) }, unsafe {
                self.data.get_unchecked(j)
            })
        });

        let mut pos;
        for i in 0..transform.len() {
            // SAFE we're in bound
            pos = unsafe { *transform.get_unchecked(i) };
            while pos < i {
                // SAFE we're in bound
                pos = unsafe { *transform.get_unchecked(pos) };
            }
            self.dense.swap(i, pos);
            self.data.swap(i, pos);
            if self.is_tracking_insertion {
                self.insertion_data.swap(i, pos);
            }
            if self.is_tracking_modification {
                self.modification_data.swap(i, pos);
            }
        }

        for (i, id) in self.dense.iter().enumerate() {
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
                self.modification_data[a_index] = current;
            }

            let a = unsafe { &mut *self.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &*self.data.as_mut_ptr().add(b_index) };

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
                self.modification_data[a_index] = current;
                self.modification_data[b_index] = current;
            }

            let a = unsafe { &mut *self.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &mut *self.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }

    /// Deletes all components in this storage.
    pub(crate) fn private_clear(&mut self, current: TrackingTimestamp) {
        self.reset_groups();

        for &id in &self.dense {
            unsafe {
                *self.sparse.get_mut_unchecked(id) = EntityId::dead();
            }
        }

        self.insertion_data.clear();
        self.modification_data.clear();

        let is_tracking_deletion = self.is_tracking_deletion();

        let dense = self.dense.drain(..);
        let data = self.data.drain(..);

        if is_tracking_deletion {
            let iter = dense
                .zip(data)
                .map(|(entity, component)| (entity, current, component));
            self.deletion_data.extend(iter);
        }
    }

    /// Creates a draining iterator that empties the storage and yields the removed items.
    pub(crate) fn private_drain(&mut self, current: TrackingTimestamp) -> SparseSetDrain<'_, T> {
        self.reset_groups();

        if self.is_tracking_removal {
            self.removal_data
                .extend(self.dense.iter().map(|&entity| (entity, current)));
        }

        for id in &self.dense {
            // SAFE ids from sparse_set.dense are always valid
            unsafe {
                *self.sparse.get_mut_unchecked(*id) = EntityId::dead();
            }
        }

        self.insertion_data.clear();
        self.modification_data.clear();

        let dense_ptr = self.dense.as_ptr();
        let dense_len = self.dense.len();

        unsafe {
            self.dense.set_len(0);
        }

        SparseSetDrain {
            dense_ptr,
            dense_len,
            data: self.data.drain(..),
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

            let eid = unsafe { *self.dense.get_unchecked(i) };
            let component = unsafe { self.data.get_unchecked(i) };

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

            let eid = unsafe { *self.dense.get_unchecked(i) };
            let component = Mut {
                flag: self.modification_data.get_mut(i),
                current,
                data: unsafe { self.data.get_unchecked_mut(i) },
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
    /// Registers the functions to clone this component and the full storage.
    #[inline]
    pub fn register_clone(&mut self) {
        self.clone = Some(SparseSetCloneFunctions::new());
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
        self.clone.as_ref().map(|clone| {
            let mut sparse_set = SparseSet::<T>::new();

            sparse_set.sparse = self.sparse.clone();
            sparse_set.dense = self.dense.clone();
            sparse_set.data = (clone.clone_storage)(&self.data);

            if sparse_set.is_tracking_insertion {
                sparse_set
                    .insertion_data
                    .resize(self.dense.len(), other_current);
            }
            if sparse_set.is_tracking_modification {
                sparse_set
                    .modification_data
                    .resize(self.dense.len(), TrackingTimestamp::origin());
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

                let _ =
                    other_sparse_set.insert(to, (clone.clone_component)(component), other_current);
            }
        }
    }

    fn groups(&self) -> Option<&Groups> {
        Some(&self.groups)
    }

    fn groups_mut(&mut self) -> Option<&mut Groups> {
        Some(&mut self.groups)
    }

    fn dense(&self) -> Option<&[EntityId]> {
        Some(&self.dense)
    }

    #[track_caller]
    fn move_to(&mut self, entity: EntityId, destination: GroupRegion<'_>) {
        self.private_move_to(entity, destination);
    }
}

#[cfg(test)]
mod tests {
    use super::groups::GroupRange;
    use super::*;
    use crate::Component;
    use alloc::sync::Arc;
    use alloc::vec;
    use std::println;

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
        assert_eq!(array.dense, &[EntityId::new_from_parts(0, 0)]);
        assert_eq!(array.data, &[STR("0")]);
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
            array.dense,
            &[
                EntityId::new_from_parts(0, 0),
                EntityId::new_from_parts(1, 0)
            ]
        );
        assert_eq!(array.data, &[STR("0"), STR("1")]);
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
            array.dense,
            &[
                EntityId::new_from_parts(0, 0),
                EntityId::new_from_parts(1, 0),
                EntityId::new_from_parts(5, 0)
            ]
        );
        assert_eq!(array.data, &[STR("0"), STR("1"), STR("5")]);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0)),
            Some(&STR("5"))
        );

        assert_eq!(array.private_get(EntityId::new_from_parts(4, 0)), None);
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
            array.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5")]);
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
            array.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0),
                EntityId::new_from_parts(3, 0),
                EntityId::new_from_parts(100, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5"), STR("3"), STR("100")]);
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
            array.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0),
                EntityId::new_from_parts(100, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5"), STR("100")]);
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
            array.dense,
            &[
                EntityId::new_from_parts(10, 0),
                EntityId::new_from_parts(5, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5")]);
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
        assert_eq!(sparse_set.insertion_data.len(), 0);
        assert_eq!(sparse_set.modification_data.len(), 0);
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
        assert_eq!(sparse_set.insertion_data.len(), 0);
        assert_eq!(sparse_set.modification_data.len(), 0);
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

        for window in array.data.windows(2) {
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

        for window in array.data.windows(2) {
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

        assert_eq!(sparse_set.insertion_data.len(), 1);
        assert_eq!(sparse_set.modification_data.len(), 1);
    }

    /// Builds a storage with entities 0, 1 and 2, all placed in a single fully trusted group range.
    fn grouped_sparse_set() -> SparseSet<I32> {
        let mut sparse_set = SparseSet::new();
        for index in 0..3 {
            sparse_set
                .insert(
                    EntityId::new_from_parts(index, 0),
                    I32(index as i32),
                    TrackingTimestamp::new(0),
                )
                .assert_inserted();
        }

        sparse_set.groups.ranges.push(GroupRange {
            group: Arc::from([StorageId::Custom(0), StorageId::Custom(1)]),
            start: 0,
            len: 3,
            shift: 0,
            trust: 3,
        });
        sparse_set.groups.unevaluated_start = 3;

        sparse_set
    }

    fn assert_reset(groups: &Groups) {
        assert_eq!(groups.ranges.len(), 1);
        assert_eq!(groups.ranges[0].start, 0);
        assert_eq!(groups.ranges[0].len, 0);
        assert_eq!(groups.ranges[0].trust, 0);
        assert_eq!(groups.unevaluated_start, 0);
    }

    fn assert_untouched(groups: &Groups) {
        assert_eq!(groups.ranges.len(), 1);
        assert_eq!(groups.ranges[0].start, 0);
        assert_eq!(groups.ranges[0].len, 3);
        assert_eq!(groups.ranges[0].trust, 3);
        assert_eq!(groups.unevaluated_start, 3);
    }

    /// Asserts that removing a component from a grouped storage resets its groups.
    #[test]
    fn groups_remove() {
        let mut sparse_set = grouped_sparse_set();

        let removed =
            sparse_set.dyn_remove(EntityId::new_from_parts(1, 0), TrackingTimestamp::new(0));

        assert_eq!(removed, Some(I32(1)));
        assert_reset(&sparse_set.groups);
    }

    /// Asserts that deleting a component from a grouped storage resets its groups.
    #[test]
    fn groups_delete() {
        let mut sparse_set = grouped_sparse_set();

        let deleted =
            sparse_set.dyn_delete(EntityId::new_from_parts(1, 0), TrackingTimestamp::new(0));

        assert!(deleted);
        assert_reset(&sparse_set.groups);
    }

    /// Asserts that `retain` removing a single component from a grouped storage resets its groups.
    #[test]
    fn groups_retain() {
        let mut sparse_set = grouped_sparse_set();

        sparse_set.private_retain(TrackingTimestamp::new(0), |_, component| component.0 != 1);

        assert_eq!(sparse_set.len(), 2);
        assert_reset(&sparse_set.groups);
    }

    /// Asserts that inserting over the slot of an entity with a smaller generation
    /// resets the storage's groups, since a different entity now sits in the range.
    #[test]
    fn groups_other_override() {
        let mut sparse_set = grouped_sparse_set();

        let result = sparse_set.insert(
            EntityId::new_from_parts(1, 1),
            I32(10),
            TrackingTimestamp::new(0),
        );

        assert!(matches!(result, InsertionResult::OtherComponentOverride));
        assert_reset(&sparse_set.groups);
    }

    /// Asserts that sorting a grouped storage resets its groups.
    #[test]
    fn groups_sort() {
        let mut sparse_set = grouped_sparse_set();

        sparse_set.sort_unstable();

        assert_reset(&sparse_set.groups);
    }

    /// Asserts that clearing a grouped storage resets its groups.
    #[test]
    fn groups_clear() {
        let mut sparse_set = grouped_sparse_set();

        sparse_set.private_clear(TrackingTimestamp::new(0));

        assert!(sparse_set.is_empty());
        assert_reset(&sparse_set.groups);
    }

    /// Asserts that draining a grouped storage resets its groups.
    #[test]
    fn groups_drain() {
        let mut sparse_set = grouped_sparse_set();

        sparse_set.private_drain(TrackingTimestamp::new(0));

        assert!(sparse_set.is_empty());
        assert_reset(&sparse_set.groups);
    }

    /// Asserts that inserting a new entity in a grouped storage leaves its groups untouched,
    /// the component lands in the unevaluated region.
    #[test]
    fn groups_insert_new() {
        let mut sparse_set = grouped_sparse_set();

        sparse_set
            .insert(
                EntityId::new_from_parts(3, 0),
                I32(3),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();

        assert_eq!(sparse_set.len(), 4);
        assert_untouched(&sparse_set.groups);
    }

    /// Asserts that overwriting an entity's component in a grouped storage leaves its groups untouched.
    #[test]
    fn groups_insert_same() {
        let mut sparse_set = grouped_sparse_set();

        let result = sparse_set.insert(
            EntityId::new_from_parts(1, 0),
            I32(10),
            TrackingTimestamp::new(0),
        );

        assert!(matches!(result, InsertionResult::ComponentOverride(I32(1))));
        assert_untouched(&sparse_set.groups);
    }

    /// Asserts that a `SparseSet` exposes its grouping metadata and entities through `Storage`,
    /// with `dense` listing the inserted entities in order.
    #[test]
    fn storage_groups() {
        let mut sparse_set = SparseSet::new();
        sparse_set
            .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(5), I32(5), TrackingTimestamp::new(0))
            .assert_inserted();

        assert!(Storage::groups(&sparse_set).is_some());
        assert!(Storage::groups_mut(&mut sparse_set).is_some());
        assert_eq!(
            Storage::dense(&sparse_set),
            Some(&[EntityId::new(0), EntityId::new(5)][..])
        );
    }

    /// Asserts that a `NonSend` `SparseSet` exposes its grouping metadata and entities through `Storage`,
    /// with `dense` listing the inserted entities in order.
    #[cfg(feature = "thread_local")]
    #[test]
    fn storage_groups_non_send() {
        let mut sparse_set = crate::borrow::NonSend(SparseSet::new());
        sparse_set
            .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(5), I32(5), TrackingTimestamp::new(0))
            .assert_inserted();

        assert!(Storage::groups(&sparse_set).is_some());
        assert!(Storage::groups_mut(&mut sparse_set).is_some());
        assert_eq!(
            Storage::dense(&sparse_set),
            Some(&[EntityId::new(0), EntityId::new(5)][..])
        );
    }

    /// Asserts that swapping two rows of a tracked storage keeps every entity's component,
    /// insertion and modification state, and points the sparse array at the new indices.
    #[test]
    fn swap_rows() {
        let mut sparse_set = SparseSet::new();
        let e0 = EntityId::new(0);
        let e1 = EntityId::new(1);
        let e2 = EntityId::new(2);
        let last = TrackingTimestamp::new(1);
        let current = TrackingTimestamp::new(2);

        sparse_set
            .insert(e0, TrackedI32(0), current)
            .assert_inserted();
        sparse_set.insert(e1, TrackedI32(1), last).assert_inserted();
        sparse_set.insert(e2, TrackedI32(2), last).assert_inserted();
        let _ = sparse_set.insert(e2, TrackedI32(20), current);

        sparse_set.swap_rows(0, 2);

        assert_eq!(sparse_set.index_of(e0), Some(2));
        assert_eq!(sparse_set.index_of(e1), Some(1));
        assert_eq!(sparse_set.index_of(e2), Some(0));
        assert_eq!(sparse_set.private_get(e0), Some(&TrackedI32(0)));
        assert_eq!(sparse_set.private_get(e2), Some(&TrackedI32(20)));
        assert!(<crate::track::All as Tracking>::is_inserted(
            &sparse_set,
            e0,
            last,
            current
        ));
        assert!(!<crate::track::All as Tracking>::is_inserted(
            &sparse_set,
            e2,
            last,
            current
        ));
        assert!(!<crate::track::All as Tracking>::is_modified(
            &sparse_set,
            e0,
            last,
            current
        ));
        assert!(<crate::track::All as Tracking>::is_modified(
            &sparse_set,
            e2,
            last,
            current
        ));
    }

    /// Asserts that swapping a row with itself changes nothing.
    #[test]
    fn swap_rows_same() {
        let mut sparse_set = SparseSet::new();
        sparse_set
            .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
            .assert_inserted();
        sparse_set
            .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
            .assert_inserted();

        sparse_set.swap_rows(1, 1);

        assert_eq!(sparse_set.dense, [EntityId::new(0), EntityId::new(1)]);
        assert_eq!(sparse_set.index_of(EntityId::new(1)), Some(1));
    }

    /// Builds a storage holding entities `0..n` in order, each with `I32(index)`.
    /// The first `range_len` are in a fully trusted range, the next `ungrouped_len` are ungrouped
    /// and the last `unevaluated_len` are unevaluated.
    fn layout(
        range_len: usize,
        ungrouped_len: usize,
        unevaluated_len: usize,
    ) -> (SparseSet<I32>, Arc<[StorageId]>) {
        let mut sparse_set = SparseSet::new();
        for index in 0..range_len + ungrouped_len + unevaluated_len {
            sparse_set
                .insert(
                    EntityId::new(index as u64),
                    I32(i32::try_from(index).unwrap()),
                    TrackingTimestamp::new(0),
                )
                .assert_inserted();
        }

        let group: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(1)]);
        sparse_set.groups.ranges.push(GroupRange {
            group: group.clone(),
            start: 0,
            len: range_len,
            shift: 0,
            trust: range_len,
        });
        sparse_set.groups.unevaluated_start = range_len + ungrouped_len;

        (sparse_set, group)
    }

    /// Returns the sorted entity indices of the range, the ungrouped region and the unevaluated region.
    fn regions(sparse_set: &SparseSet<I32>) -> [Vec<u64>; 3] {
        let grouped_end = sparse_set.groups.grouped_end();
        let unevaluated_start = sparse_set.groups.unevaluated_start;

        let sorted = |entities: &[EntityId]| {
            let mut indices: Vec<u64> = entities.iter().map(|entity| entity.index()).collect();
            indices.sort_unstable();
            indices
        };

        [
            sorted(&sparse_set.dense[..grouped_end]),
            sorted(&sparse_set.dense[grouped_end..unevaluated_start]),
            sorted(&sparse_set.dense[unevaluated_start..]),
        ]
    }

    /// Panics if an entity of a storage built by `layout` lost its component.
    fn assert_reachable(sparse_set: &SparseSet<I32>) {
        for index in 0..sparse_set.len() {
            assert_eq!(
                sparse_set.private_get(EntityId::new(index as u64)),
                Some(&I32(i32::try_from(index).unwrap()))
            );
        }
    }

    /// Asserts that moving an unevaluated component to the range appends it,
    /// keeps the range fully trusted and pushes an ungrouped component into the grown ungrouped region.
    #[test]
    fn move_unevaluated_to_range() {
        let (mut sparse_set, group) = layout(2, 2, 3);

        sparse_set.private_move_to(EntityId::new(5), GroupRegion::Range(&group));

        assert_eq!(sparse_set.groups.ranges[0].len, 3);
        assert_eq!(sparse_set.groups.ranges[0].trust, 3);
        assert_eq!(sparse_set.groups.unevaluated_start, 5);
        assert_eq!(sparse_set.dense[2], EntityId::new(5));
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 1, 5], vec![2, 3], vec![4, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving an unevaluated component to the range works without any ungrouped component.
    #[test]
    fn move_unevaluated_to_range_no_ungrouped() {
        let (mut sparse_set, group) = layout(2, 0, 3);

        sparse_set.private_move_to(EntityId::new(3), GroupRegion::Range(&group));

        assert_eq!(sparse_set.groups.ranges[0].len, 3);
        assert_eq!(sparse_set.groups.ranges[0].trust, 3);
        assert_eq!(sparse_set.groups.unevaluated_start, 3);
        assert_eq!(regions(&sparse_set), [vec![0, 1, 3], vec![], vec![2, 4]]);
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving an ungrouped component to the range appends it and keeps the range fully trusted.
    #[test]
    fn move_ungrouped_to_range() {
        let (mut sparse_set, group) = layout(2, 2, 3);

        sparse_set.private_move_to(EntityId::new(3), GroupRegion::Range(&group));

        assert_eq!(sparse_set.groups.ranges[0].len, 3);
        assert_eq!(sparse_set.groups.ranges[0].trust, 3);
        assert_eq!(sparse_set.groups.unevaluated_start, 4);
        assert_eq!(sparse_set.dense[2], EntityId::new(3));
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 1, 3], vec![2], vec![4, 5, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that appending to a damaged range leaves its trust unchanged.
    #[test]
    fn move_to_damaged_range() {
        let (mut sparse_set, group) = layout(3, 2, 2);
        sparse_set.groups.ranges[0].trust = 1;

        sparse_set.private_move_to(EntityId::new(3), GroupRegion::Range(&group));

        assert_eq!(sparse_set.groups.ranges[0].len, 4);
        assert_eq!(sparse_set.groups.ranges[0].trust, 1);
    }

    /// Asserts that moving an unevaluated component to the ungrouped region grows it by one.
    #[test]
    fn move_unevaluated_to_ungrouped() {
        let (mut sparse_set, _) = layout(2, 2, 3);

        sparse_set.private_move_to(EntityId::new(5), GroupRegion::Ungrouped);

        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.unevaluated_start, 5);
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 1], vec![2, 3, 5], vec![4, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving a component from the middle of the range to the unevaluated region
    /// shrinks the range and drops its trust to the component's old position.
    #[test]
    fn move_range_to_unevaluated() {
        let (mut sparse_set, _) = layout(3, 2, 2);

        sparse_set.private_move_to(EntityId::new(1), GroupRegion::Unevaluated);

        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.ranges[0].trust, 1);
        assert_eq!(sparse_set.groups.unevaluated_start, 4);
        assert_eq!(sparse_set.dense[..2], [EntityId::new(0), EntityId::new(2)]);
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 2], vec![3, 4], vec![1, 5, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving a component from the range to the unevaluated region works without any ungrouped component.
    #[test]
    fn move_range_to_unevaluated_no_ungrouped() {
        let (mut sparse_set, _) = layout(3, 0, 2);

        sparse_set.private_move_to(EntityId::new(0), GroupRegion::Unevaluated);

        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.ranges[0].trust, 0);
        assert_eq!(sparse_set.groups.unevaluated_start, 2);
        assert_eq!(regions(&sparse_set), [vec![1, 2], vec![], vec![0, 3, 4]]);
        assert_reachable(&sparse_set);
    }

    /// Asserts that evicting the range's last component keeps the rest of the range fully trusted.
    #[test]
    fn move_range_last_to_unevaluated() {
        let (mut sparse_set, _) = layout(3, 2, 2);

        sparse_set.private_move_to(EntityId::new(2), GroupRegion::Unevaluated);

        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.ranges[0].trust, 2);
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 1], vec![3, 4], vec![2, 5, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving a component from the range to the ungrouped region shrinks the range
    /// and drops its trust to the component's old position.
    #[test]
    fn move_range_to_ungrouped() {
        let (mut sparse_set, _) = layout(3, 2, 2);

        sparse_set.private_move_to(EntityId::new(1), GroupRegion::Ungrouped);

        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.ranges[0].trust, 1);
        assert_eq!(sparse_set.groups.unevaluated_start, 5);
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 2], vec![1, 3, 4], vec![5, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving an ungrouped component to the unevaluated region shrinks the ungrouped region by one.
    #[test]
    fn move_ungrouped_to_unevaluated() {
        let (mut sparse_set, _) = layout(2, 2, 3);

        sparse_set.private_move_to(EntityId::new(2), GroupRegion::Unevaluated);

        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.unevaluated_start, 3);
        assert_eq!(
            regions(&sparse_set),
            [vec![0, 1], vec![3], vec![2, 4, 5, 6]]
        );
        assert_reachable(&sparse_set);
    }

    /// Asserts that moving components to the region they're already in doesn't move anything.
    #[test]
    fn move_in_place() {
        let (mut sparse_set, group) = layout(2, 2, 3);
        let dense = sparse_set.dense.clone();

        sparse_set.private_move_to(EntityId::new(0), GroupRegion::Range(&group));
        sparse_set.private_move_to(EntityId::new(2), GroupRegion::Ungrouped);
        sparse_set.private_move_to(EntityId::new(4), GroupRegion::Unevaluated);

        assert_eq!(sparse_set.dense, dense);
        assert_eq!(sparse_set.groups.ranges[0].len, 2);
        assert_eq!(sparse_set.groups.ranges[0].trust, 2);
        assert_eq!(sparse_set.groups.unevaluated_start, 4);
    }

    /// Asserts that moving to a group without a range in the storage panics,
    /// even when an equal group exists, since groups are compared by pointer.
    #[test]
    #[should_panic(expected = "the destination group has no range in this storage")]
    fn move_to_other_group() {
        let (mut sparse_set, _) = layout(2, 2, 3);
        let other_group: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(1)]);

        sparse_set.private_move_to(EntityId::new(4), GroupRegion::Range(&other_group));
    }

    /// Asserts that moving an entity without a component in the storage panics.
    #[test]
    #[should_panic(expected = "only components present in the storage can be moved")]
    fn move_missing() {
        let (mut sparse_set, _) = layout(2, 2, 3);

        sparse_set.private_move_to(EntityId::new(10), GroupRegion::Ungrouped);
    }

    /// Asserts that `Storage::move_to` panics on storages that don't support grouping.
    #[test]
    #[should_panic(expected = "grouping only supports SparseSet storages")]
    fn move_unsupported() {
        let mut entities = crate::entities::Entities::new();

        Storage::move_to(&mut entities, EntityId::new(0), GroupRegion::Ungrouped);
    }

    /// Asserts that `Storage::move_to` on a `NonSend` `SparseSet` moves the component.
    #[cfg(feature = "thread_local")]
    #[test]
    fn move_non_send() {
        let (sparse_set, _) = layout(2, 2, 3);
        let mut sparse_set = crate::borrow::NonSend(sparse_set);

        Storage::move_to(&mut sparse_set, EntityId::new(5), GroupRegion::Ungrouped);

        assert_eq!(
            regions(&sparse_set),
            [vec![0, 1], vec![2, 3, 5], vec![4, 6]]
        );
    }
}
