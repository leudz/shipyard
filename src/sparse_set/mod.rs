mod add_component;
mod bulk_add_entity;
mod clone;
mod delete;
mod drain;
mod groups;
mod memory_usage;
mod remove;
mod sparse_array;
#[cfg(test)]
mod tests;
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
pub use window::EntityCursor;

pub(crate) use groups::{GroupRange, Ring};
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
            if entity.generation() == sparse_entity.generation() {
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
        // SAFE: Our caller guarantees `entity` owns a component, so its bucket is allocated.
        unsafe { self.sparse.get_unchecked(entity).uindex() }
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
                EntityId::new_from_index_and_gen(self.dense.len() as u64, entity.generation());

            if self.is_tracking_insertion {
                self.insertion_data.push(current);
            }
            if self.is_tracking_modification {
                self.modification_data.push(TrackingTimestamp::origin());
            }

            self.dense.push(entity);
            self.data.push(value);

            old_component = InsertionResult::Inserted;
        } else if entity.generation() == sparse_entity.generation() {
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
        } else if entity.generation() > sparse_entity.generation() {
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

        if entity.generation() >= sparse_entity.generation() {
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

            if entity.generation() == sparse_entity.generation() {
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

/// A region of a grouped storage, with ranges identified by their position in [`Groups::ranges`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum LocalRegion {
    Range(usize),
    Ungrouped,
    Unevaluated,
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

    /// Moves every range from `first` on one slot to the left, keeping their logical order.
    ///
    /// The component in the slot right before `ranges[first]` is carried along and ends
    /// right after the last range.
    fn move_ranges_left(&mut self, first: usize) {
        for range_index in first..self.groups.ranges.len() {
            let ring = self.groups.ranges[range_index].ring();

            // The carried component is right before this range. It swaps with the range's
            // physical last component, which keeps its logical position once the range moved.
            if ring.len != 0 {
                self.swap_rows(ring.start - 1, ring.end() - 1);
            }
            self.groups.ranges[range_index].move_left();
        }
    }

    /// Moves every range from `first` on one slot to the right, keeping their logical order.
    ///
    /// The component in the slot right after the last range is carried along and ends
    /// right before `ranges[first]`.
    fn move_ranges_right(&mut self, first: usize) {
        for range_index in (first..self.groups.ranges.len()).rev() {
            let ring = self.groups.ranges[range_index].ring();

            // The carried component is right after this range. It swaps with the range's
            // physical first component, which keeps its logical position once the range moved.
            if ring.len != 0 {
                self.swap_rows(ring.end(), ring.start);
            }
            self.groups.ranges[range_index].move_right();
        }
    }

    /// Moves `entity`'s component to `destination`, wherever it currently is in this storage.
    ///
    /// A move into a range appends the component at the range's logical end.\
    /// A move out of a range is a logical swap-remove: the range's logical last component
    /// takes its position.\
    /// Every other range keeps its logical order and trust.\
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

        let destination = match destination {
            GroupRegion::Range(group) => LocalRegion::Range(
                self.groups
                    .range_index(group)
                    .expect("the destination group has no range in this storage"),
            ),
            GroupRegion::Ungrouped => LocalRegion::Ungrouped,
            GroupRegion::Unevaluated => LocalRegion::Unevaluated,
        };
        let source = match self.groups.range_index_of(index) {
            Some(range_index) => LocalRegion::Range(range_index),
            None if index < self.groups.unevaluated_start => LocalRegion::Ungrouped,
            None => LocalRegion::Unevaluated,
        };

        if source == destination {
            return;
        }

        // The ungrouped region sits between the ranges and the unevaluated region,
        // so every move goes through it: out of the source, then into the destination.
        // Each step returns the component's new index, hole passing moves it around.
        let index = match source {
            LocalRegion::Range(range_index) => self.range_to_ungrouped(range_index, index),
            LocalRegion::Ungrouped => index,
            LocalRegion::Unevaluated => self.unevaluated_to_ungrouped(index),
        };

        match destination {
            LocalRegion::Range(range_index) => self.ungrouped_to_range(range_index, index),
            LocalRegion::Ungrouped => {}
            LocalRegion::Unevaluated => self.ungrouped_to_unevaluated(index),
        }

        #[cfg(debug_assertions)]
        self.groups.assert_invariants(self.dense.len());
    }

    /// Moves the component at `index`, inside range `range_index`, to the ungrouped region.\
    /// Returns its new index.
    fn range_to_ungrouped(&mut self, range_index: usize, index: usize) -> usize {
        let logical = self.groups.ranges[range_index].ring().logical_of(index);

        // The component leaves its range at the slot right after the shrunk range,
        // then the later ranges move left over it and leave it as the first ungrouped component.
        self.take_from_range(range_index, logical);
        self.move_ranges_left(range_index + 1);

        self.groups.grouped_end()
    }

    /// Takes the component at `logical` out of range `range_index` with a logical swap-remove.
    ///
    /// The component ends right after the shrunk range, in a slot no range covers anymore.\
    /// Trust is lowered conservatively: other members only agree once they did the same.
    fn take_from_range(&mut self, range_index: usize, logical: usize) {
        let ring = self.groups.ranges[range_index].ring();
        let last = ring.len - 1;

        // The logical last component fills the hole, ours takes its place.
        // Members at different rotations all get the same logical result,
        // unlike with a physical swap-remove.
        self.swap_rows(ring.physical_of(logical), ring.physical_of(last));

        // Our component sits at the wrap point now. We move it to the ring's physical last slot
        // from the cheaper side, only that member's start and shift depend on the side.
        let wrap_offset = ring.wrap_offset();
        let unwrapped_len = ring.len - wrap_offset;
        let new_wrap_offset = if wrap_offset == 0 {
            // Unrotated, the logical last component is the physical last one.
            0
        } else if unwrapped_len <= wrap_offset {
            // The unwrapped part slides left over our component.
            for physical in ring.start + wrap_offset - 1..ring.end() - 1 {
                self.swap_rows(physical, physical + 1);
            }

            wrap_offset - 1
        } else {
            // The wrapped part slides right over our component, which lands on the first slot.
            // Then it swaps with the unwrapped part's last component, which becomes the
            // wrapped part's first.
            for physical in (ring.start..ring.start + wrap_offset - 1).rev() {
                self.swap_rows(physical, physical + 1);
            }
            self.swap_rows(ring.start, ring.end() - 1);

            // The unwrapped part is the longer one so it has at least 2 components,
            // the wrap offset stays smaller than the new length.
            wrap_offset
        };

        let range = &mut self.groups.ranges[range_index];
        // Only `logical` and the old last position changed, the positions before `logical` didn't.
        range.trust = range.trust.min(logical);
        range.resize(last, new_wrap_offset);
    }

    /// Appends the ungrouped component at `index` to range `range_index`.
    fn ungrouped_to_range(&mut self, range_index: usize, index: usize) {
        // We swap it with the first ungrouped component, right after the last range.
        // The later ranges then move right over it, which leaves it right after our range.
        let grouped_end = self.groups.grouped_end();
        self.swap_rows(index, grouped_end);
        self.move_ranges_right(range_index + 1);

        self.append_to_range(range_index);
    }

    /// Appends the component right after range `range_index` at the range's logical end.
    fn append_to_range(&mut self, range_index: usize) {
        let ring = self.groups.ranges[range_index].ring();

        // The logical end is at the wrap point. We open a slot there from the cheaper side,
        // only that member's start and shift depend on the side.
        let new_wrap_offset = if ring.len == 0 || ring.wrap_offset() == 0 {
            // The logical end is the physical end, where our component already is.
            0
        } else {
            let wrap_offset = ring.wrap_offset();

            if ring.len - wrap_offset <= wrap_offset {
                // The unwrapped part slides right, our component lands right before it.
                for physical in (ring.start + wrap_offset..ring.end()).rev() {
                    self.swap_rows(physical, physical + 1);
                }

                wrap_offset + 1
            } else {
                // The wrapped part's first component moves to the end, after the unwrapped part's
                // last. Our component takes its slot and slides right to the end of the wrapped part.
                self.swap_rows(ring.start, ring.end());
                for physical in ring.start..ring.start + wrap_offset - 1 {
                    self.swap_rows(physical, physical + 1);
                }

                wrap_offset
            }
        };

        let range = &mut self.groups.ranges[range_index];
        // Regroup appends the same entity at the same logical end of every member,
        // so a fully trusted range stays fully trusted. A damaged one stays damaged.
        if range.trust == range.len {
            range.trust += 1;
        }
        range.resize(ring.len + 1, new_wrap_offset);
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
        if let Some(clone) = &self.clone
            && let Some(component) = self.private_get(from)
        {
            let other_sparse_set = other_all_storages.exclusive_storage_or_insert_mut(
                StorageId::of::<SparseSet<T>>(),
                SparseSet::<T>::new,
            );

            let _ = other_sparse_set.insert(to, (clone.clone_component)(component), other_current);
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
