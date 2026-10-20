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
use core::ops::Range;
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

            let index = sparse_entity.uindex();
            self.replace_older_generation(entity, index, value, current);

            old_component = InsertionResult::OtherComponentOverride;
        } else {
            old_component = InsertionResult::NotInserted;
        }

        old_component
    }

    /// Replaces the component at `index`, owned by an older generation of `entity`, with `value`.
    ///
    /// The new component counts as inserted. The old one is dropped, it isn't a removal
    /// so there's no removal callback or tracking.
    fn replace_older_generation(
        &mut self,
        entity: EntityId,
        index: usize,
        value: T,
        current: TrackingTimestamp,
    ) {
        // The other members of the old entity's ranges don't hold the new entity,
        // so its component can't stay in them.
        let index = self.extract_to_unevaluated(index);

        // SAFE: The extraction returns an index of the storage, so it's in bounds of every vector.
        // `entity`'s sparse entry was allocated by `insert`, the extraction kept it pointing at `index`.
        let old_value = unsafe {
            self.sparse.get_mut_unchecked(entity).copy_gen(entity);
            self.dense.get_unchecked_mut(index).copy_index_gen(entity);

            if self.is_tracking_insertion {
                *self.insertion_data.get_unchecked_mut(index) = current;
            }
            // The new entity's component was never modified, it doesn't inherit the old one's timestamp.
            if self.is_tracking_modification {
                *self.modification_data.get_unchecked_mut(index) = TrackingTimestamp::origin();
            }

            core::mem::replace(self.data.get_unchecked_mut(index), value)
        };

        #[cfg(debug_assertions)]
        self.groups.assert_invariants(self.dense.len());

        // The storage is consistent by now, in case dropping the old value panics.
        drop(old_value);
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
            // In a grouped storage, the component first moves to the unevaluated region.
            // The swap-remove below then only moves the storage's last component, which is unevaluated too.
            // Its sparse entry is still live here, so the moves keep it up to date.
            let index = self.extract_to_unevaluated(sparse_entity.uindex());

            // SAFE: `sparse.get` found an entry for `entity`, so its sparse page is allocated.
            unsafe {
                *self.sparse.get_mut_unchecked(entity) = EntityId::dead();
            }

            self.dense.swap_remove(index);
            if self.is_tracking_insertion() {
                self.insertion_data.swap_remove(index);
            }
            if self.is_tracking_modification() {
                self.modification_data.swap_remove(index);
            }
            let component = self.data.swap_remove(index);

            // The SparseSet could now be empty or the removed component could have been the last one
            if index < self.dense.len() {
                // SAFE: `index` is in bounds, and `last` was read from `dense`
                // so it owns a component and its sparse page is allocated.
                unsafe {
                    let last = *self.dense.get_unchecked(index);
                    self.sparse.get_mut_unchecked(last).set_index(index as u64);
                }
            }

            #[cfg(debug_assertions)]
            self.groups.assert_invariants(self.dense.len());

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
    /// Called by clear and drain, before they empty the whole storage, see [`Groups::reset`].
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
    /// A storage without any range is implicitly ungrouped, moving to `Ungrouped` does nothing.
    ///
    /// ### Panics
    ///
    /// - `entity` doesn't have a component in this storage.
    /// - `destination` is a group without a range in this storage.
    /// - `destination` is `Unevaluated` and this storage has no range.
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

        // Without range every component is already ungrouped, and there's no unevaluated region.
        // A range destination can't get here, finding its range would have panicked.
        if !self.groups.has_ranges() {
            assert!(
                destination == LocalRegion::Ungrouped,
                "a storage without group ranges has no unevaluated region"
            );

            return;
        }

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

    /// Moves the component at `index` to the unevaluated region, wherever it is.\
    /// Returns its new index.
    ///
    /// This prepares a component to be removed or replaced. Unlike a regroup move, it leaves its
    /// range with a physical swap-remove, so its cost doesn't depend on the range's length.
    /// That range only keeps trusting the logical positions the move didn't change.
    /// Every other range keeps its logical order and trust.\
    /// Timestamps move with the component. A move isn't a modification and doesn't call any callback.
    #[inline]
    fn extract_to_unevaluated(&mut self, index: usize) -> usize {
        // Most storages have no group, we keep their cost to this single branch.
        if !self.groups.has_ranges() || index >= self.groups.unevaluated_start {
            return index;
        }

        let index = match self.groups.range_index_of(index) {
            Some(range_index) => self.extract_from_range(range_index, index),
            None => index,
        };
        self.ungrouped_to_unevaluated(index);

        self.groups.unevaluated_start
    }

    /// Moves the component at `index`, inside range `range_index`, to the ungrouped region
    /// with a physical swap-remove.\
    /// Returns its new index.
    ///
    /// Unlike [`take_from_range`](Self::take_from_range), its cost doesn't depend on the range's length.
    /// Other members can't reproduce the resulting order though, so the range's trust gets lowered further.
    fn extract_from_range(&mut self, range_index: usize, index: usize) -> usize {
        let ring = self.groups.ranges[range_index].ring();
        let logical = ring.logical_of(index);
        let wrap_offset = ring.wrap_offset();
        // Physical offsets `[s, len)` hold logical `[0, len - s)`, so the last slot holds `len - 1 - s`.
        let physical_last_logical = ring.len - 1 - wrap_offset;

        // The physical last component fills the hole, ours sits right after the shrunk range.
        self.swap_rows(index, ring.end() - 1);

        // Only the positions before `logical` and `physical_last_logical` are unchanged.
        // Our hole at `logical` now holds another component, and dropping the last slot
        // moves every logical position after `physical_last_logical` back by one.\
        // Logical 0 keeps its slot, unless it was the dropped one. Then the ring starts at its first slot.
        let new_len = ring.len - 1;
        let new_wrap_offset = if wrap_offset < new_len {
            wrap_offset
        } else {
            0
        };

        let range = &mut self.groups.ranges[range_index];
        range.trust = range.trust.min(logical).min(physical_last_logical);
        range.resize(new_len, new_wrap_offset);

        // The later ranges move left over our component and leave it as the first ungrouped component.
        self.move_ranges_left(range_index + 1);

        self.groups.grouped_end()
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
    /// Sorts the ungrouped components with a comparator function, but may not preserve the order of equal elements.
    ///
    /// In a storage that isn't part of any group, every component is sorted.\
    /// In a grouped storage, only the components regroup found to belong to no group are sorted.
    /// Group ranges and components added since the last regroup keep their position.
    ///
    /// ### Panics
    ///
    /// If `compare` panics, the components being sorted can be left in any order.
    /// The storage stays valid: every entity keeps its component and timestamps,
    /// and the components that weren't being sorted don't move.
    pub fn sort_unstable_by<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        let sortable = self.groups.ungrouped_range(self.dense.len());
        if sortable.len() < 2 {
            return;
        }

        // We only sort the entity ids. Components, timestamps and the sparse array stay where they
        // are until `repair` moves them to follow their entity, when it's dropped.
        // That happens on return or while unwinding from `compare`.
        let repair = SortRepair {
            sortable: sortable.clone(),
            dense: &mut self.dense,
            sparse: &mut self.sparse,
            data: &mut self.data,
            insertion_data: self
                .is_tracking_insertion
                .then_some(&mut self.insertion_data[..]),
            modification_data: self
                .is_tracking_modification
                .then_some(&mut self.modification_data[..]),
        };

        // The sparse array still points each entity at its component's original row while the ids move.
        // Even if `compare` panics, the slice ends with all its ids, in some order.
        repair.dense[sortable].sort_unstable_by(|&a, &b| {
            // SAFE: `a` and `b` are ids of the sorted slice, which only ever holds the ids it started with.
            // These come from `dense` so they own a component and their sparse page is allocated.
            // Their sparse entries hold the row of their component, smaller than `data`'s length.
            // Neither `sparse` nor `data` change until `repair` is dropped, so these rows stay valid.
            let (a_component, b_component) = unsafe {
                let a_index = repair.sparse.get_unchecked(a).uindex();
                let b_index = repair.sparse.get_unchecked(b).uindex();

                (
                    repair.data.get_unchecked(a_index),
                    repair.data.get_unchecked(b_index),
                )
            };

            compare(a_component, b_component)
        });
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
        // A removal only moves components at its index or after it, even in a grouped storage.
        // So the visited components stay before `i`, and the component that takes `i` hasn't been visited yet.
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
        // Same traversal as `private_retain`, removals never move a visited component.
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
    /// Sorts the ungrouped components, but may not preserve the order of equal elements.
    ///
    /// See [`sort_unstable_by`](Self::sort_unstable_by) for which components are sorted
    /// and what happens on panic.
    pub fn sort_unstable(&mut self) {
        self.sort_unstable_by(Ord::cmp)
    }
}

/// Moves the rows of a [`SparseSet`] to follow its sorted entity ids, when dropped.
///
/// Sorting only reorders `dense[sortable]`. Until we're dropped, every sorted entity's
/// sparse entry still holds the original row of its component and timestamps.
struct SortRepair<'a, T> {
    /// Physical indices of the sorted rows.
    sortable: Range<usize>,
    dense: &'a mut [EntityId],
    sparse: &'a mut SparseArray,
    data: &'a mut [T],
    /// `None` when the storage doesn't track insertion.
    insertion_data: Option<&'a mut [TrackingTimestamp]>,
    /// `None` when the storage doesn't track modification.
    modification_data: Option<&'a mut [TrackingTimestamp]>,
}

impl<T> Drop for SortRepair<'_, T> {
    fn drop(&mut self) {
        // SAFE: `sortable` is part of the ungrouped region, inside the storage, and `dense`, `data`
        // and the enabled timestamp vectors all have the storage's length.
        // The sort kept the same ids in `dense[sortable]`, they come from `dense` so their
        // sparse page is allocated. Nothing touched the sparse array or the rows since sorting
        // started, so each entry still holds its entity's original row, one of `sortable`'s,
        // and no two entities share a row.
        unsafe {
            repair_rows(
                self.sortable.clone(),
                self.dense,
                self.sparse,
                self.data,
                self.insertion_data.as_deref_mut(),
                self.modification_data.as_deref_mut(),
            );
        }
    }
}

/// Moves the components and timestamps of `domain` to follow the ids of `dense[domain]`,
/// pointing each of their sparse entries at its new row.
///
/// The ids are already in the order we want, `dense` doesn't change. Rows outside `domain`
/// and the sparse entries of ids outside it aren't touched.\
/// Rows already in place don't move. A cycle of `k` rows takes `k - 1` swaps of each row array,
/// a component can move several times along its cycle.
///
/// It never calls user code, allocates or drops a component, so it can't panic.
///
/// ### Safety
///
/// - `domain` must be in bounds of `dense`, `data` and each timestamp slice passed.
/// - Every id of `dense[domain]` must have an allocated sparse page, and its sparse entry
///   must hold the row of that entity's component and timestamps.
/// - These rows must be exactly the rows of `domain`, each one held by a single entry.
unsafe fn repair_rows<T>(
    domain: Range<usize>,
    dense: &[EntityId],
    sparse: &mut SparseArray,
    data: &mut [T],
    mut insertion_data: Option<&mut [TrackingTimestamp]>,
    mut modification_data: Option<&mut [TrackingTimestamp]>,
) {
    // Calling `original(d)` the sparse index of `dense[d]`, the row that should end at `d`:
    // our caller guarantees `original` is a permutation of `domain`.
    //
    // We follow each of its cycles. The component `start` needs is moved there,
    // and the one it displaces is carried forward: after the swap, `current` always holds
    // `start`'s original component. When the cycle comes back to `start`,
    // `current` is the destination that needs it, so it's already in place.
    //
    // We read `next` before overwriting the sparse entry with `current`.
    // The overwrite marks the destination repaired: `original(d) == d` from then on,
    // so when the outer loop reaches a destination of a cycle we already walked,
    // it's a fixed point and nothing moves.
    //
    // `current` and `next` are always rows of `domain`: `current` starts as `start`
    // then takes the values of `next`, which `original` maps inside `domain`.
    for start in domain {
        let mut current = start;

        loop {
            // SAFE: `current` is a row of `domain`, in bounds of `dense`.
            let entity = unsafe { *dense.get_unchecked(current) };

            // SAFE: `entity` is an id of `dense[domain]`, our caller guarantees its page is allocated.
            let sparse_entity = unsafe { sparse.get_mut_unchecked(entity) };
            let next = sparse_entity.uindex();
            // `current` is a row of `domain`, a valid component index.
            sparse_entity.set_index(current as u64);

            if next == start {
                break;
            }

            // SAFE: `current` and `next` are rows of `domain`, in bounds of `data`
            // and of the timestamp slices we got.
            unsafe {
                swap_unchecked(data, current, next);
                if let Some(insertion_data) = &mut insertion_data {
                    swap_unchecked(insertion_data, current, next);
                }
                if let Some(modification_data) = &mut modification_data {
                    swap_unchecked(modification_data, current, next);
                }
            }

            current = next;
        }
    }
}

/// Swaps the elements at indices `a` and `b` of `slice`, without bounds checks.
///
/// ### Safety
///
/// `a` and `b` must be smaller than `slice`'s length.
unsafe fn swap_unchecked<U>(slice: &mut [U], a: usize, b: usize) {
    let ptr = slice.as_mut_ptr();

    // SAFE: Our caller guarantees both indices are in bounds, so both pointers point to elements
    // of `slice`, which we borrow mutably. `ptr::swap` also accepts two equal pointers.
    unsafe { core::ptr::swap(ptr.add(a), ptr.add(b)) }
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
