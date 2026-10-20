mod sbox;
mod storage_id;
#[cfg(test)]
mod tests;

pub use sbox::SBoxBuilder;
pub use storage_id::StorageId;

pub(crate) use sbox::SBox;

use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
use crate::memory_usage::StorageMemoryUsage;
use crate::sparse_set::{GroupRegion, Groups, SparseArray};
use crate::tracking::TrackingTimestamp;
use alloc::borrow::Cow;
use core::any::Any;
use core::ops::Range;

/// A grouped storage's ids, lent by [`Storage::reorder_ids`] to be reordered,
/// next to the parts that stay as they are.
#[doc(hidden)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "regroup doesn't use the two-phase path yet")
)]
pub struct ReorderParts<'a> {
    /// The `EntityId` of every component, in physical order.
    pub(crate) dense: &'a mut [EntityId],
    /// Still holds the rows the components and timestamps are in.
    pub(crate) sparse: &'a SparseArray,
    pub(crate) groups: &'a Groups,
}

pub trait SizedAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> SizedAny for T {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Defines common storage operations.
pub trait Storage: SizedAny {
    /// Casts to `&dyn Any`.
    #[inline]
    fn any(&self) -> &dyn Any {
        SizedAny::as_any(self)
    }
    /// Casts to `&mut dyn Any`.
    #[inline]
    fn any_mut(&mut self) -> &mut dyn Any {
        SizedAny::as_any_mut(self)
    }
    /// Deletes an entity from this storage.
    #[inline]
    #[allow(unused_variables)]
    fn delete(&mut self, entity: EntityId, current: TrackingTimestamp) {}
    /// Deletes all components of this storage.
    #[inline]
    #[allow(unused_variables)]
    fn clear(&mut self, current: TrackingTimestamp) {}
    /// Returns how much memory this storage uses.
    #[inline]
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        None
    }
    /// Returns the storage's name.
    #[inline]
    fn name(&self) -> Cow<'static, str> {
        core::any::type_name::<Self>().into()
    }
    /// Returns a [`SparseSet`]'s internal [`SparseArray`].
    ///
    /// [`SparseSet`]: crate::sparse_set::SparseSet
    /// [`SparseArray`]: crate::sparse_set::SparseArray
    #[inline]
    fn sparse_array(&self) -> Option<&SparseArray> {
        None
    }
    /// Returns `true` if the storage is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        false
    }
    /// Clear all insertion tracking data.
    #[inline]
    #[allow(unused_variables)]
    fn clear_all_inserted(&mut self, current: TrackingTimestamp) {}
    /// Clear all modification tracking data.
    #[inline]
    #[allow(unused_variables)]
    fn clear_all_modified(&mut self, current: TrackingTimestamp) {}
    /// Clear all deletion and removal tracking data.
    fn clear_all_removed_and_deleted(&mut self) {}
    /// Clear all deletion and removal tracking data older than some timestamp.
    #[inline]
    #[allow(unused_variables)]
    fn clear_all_removed_and_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
    }
    /// Moves a component from a `World` to another.
    #[inline]
    #[allow(unused_variables)]
    fn move_component_from(
        &mut self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        current: TrackingTimestamp,
        other_current: TrackingTimestamp,
    ) {
    }

    /// Attempts to clone the entire storage.
    #[inline]
    #[allow(unused_variables)]
    fn try_clone(&self, other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        None
    }

    /// Clones a component from a `World` to another.
    #[inline]
    #[allow(unused_variables)]
    fn clone_component_to(
        &self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        other_current: TrackingTimestamp,
    ) {
    }

    /// Returns the storage's grouping metadata, if it supports grouping.
    #[doc(hidden)]
    #[inline]
    fn groups(&self) -> Option<&Groups> {
        None
    }

    /// Returns the storage's grouping metadata, if it supports grouping.
    #[doc(hidden)]
    #[inline]
    fn groups_mut(&mut self) -> Option<&mut Groups> {
        None
    }

    /// Returns the `EntityId` of every component, in physical order, if the storage supports grouping.
    #[doc(hidden)]
    #[inline]
    fn dense(&self) -> Option<&[EntityId]> {
        None
    }

    /// Moves `entity`'s component to `destination`, wherever it currently is in this storage.
    ///
    /// Regroup only calls it on storages that support grouping.\
    /// A storage without any group range is implicitly ungrouped: moving to
    /// [`GroupRegion::Ungrouped`] does nothing and no component can be moved to
    /// [`GroupRegion::Unevaluated`].
    ///
    /// ### Panics
    ///
    /// - The storage doesn't support grouping.
    /// - `entity` doesn't have a component in this storage.
    /// - `destination` is a group without a range in this storage.
    /// - `destination` is [`GroupRegion::Unevaluated`] and the storage has no group range.
    #[doc(hidden)]
    #[allow(unused_variables)]
    #[track_caller]
    fn move_to(&mut self, entity: EntityId, destination: GroupRegion<'_>) {
        panic!("grouping only supports SparseSet storages");
    }

    /// Returns the `EntityId` of every component in physical order, to reorder them,
    /// along with the storage's sparse array and groups, if the storage supports grouping.
    ///
    /// Nothing follows the ids: each sparse entry keeps the row its entity's component
    /// and timestamps are in, until [`repair_rows`](Storage::repair_rows) moves them.
    ///
    /// ### Safety
    ///
    /// - The ids must only be permuted, none can be added, lost or duplicated.
    /// - Before the storage is used in any other way than through this function,
    ///   [`sparse_array`](Storage::sparse_array), [`dense`](Storage::dense),
    ///   [`groups`](Storage::groups) and [`groups_mut`](Storage::groups_mut),
    ///   every reordered row has to be repaired with [`repair_rows`](Storage::repair_rows).
    #[doc(hidden)]
    #[inline]
    unsafe fn reorder_ids(&mut self) -> Option<ReorderParts<'_>> {
        None
    }

    /// Moves the components and timestamps of `domain` to follow the ids
    /// [`reorder_ids`](Storage::reorder_ids) reordered, pointing their sparse entries at their new row.
    ///
    /// Rows already in place don't move and nothing outside `domain` is touched.
    ///
    /// ### Panics
    ///
    /// - The storage doesn't support grouping.
    ///
    /// ### Safety
    ///
    /// - `domain` must be in bounds of the storage.
    /// - The sparse entry of every id of `dense[domain]` must hold the row of that entity's
    ///   component and timestamps, and these rows must be exactly the rows of `domain`.\
    ///   This holds when, since the last repair, ids were only permuted inside `domain`.
    #[doc(hidden)]
    #[allow(unused_variables)]
    unsafe fn repair_rows(&mut self, domain: Range<usize>) {
        panic!("grouping only supports SparseSet storages");
    }
}
