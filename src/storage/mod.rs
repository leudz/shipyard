mod sbox;
mod storage_id;

pub use sbox::SBoxBuilder;
pub use storage_id::StorageId;

pub(crate) use sbox::SBox;

use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
use crate::memory_usage::StorageMemoryUsage;
use crate::sparse_set::SparseArray;
use crate::tracking::TrackingTimestamp;
use alloc::borrow::Cow;
use core::any::{Any, TypeId};

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
    /// Consumes pending regroup inputs, emitting page masks and group definitions.
    #[doc(hidden)]
    #[inline]
    #[allow(unused_variables)]
    fn collect_regroup_pages(
        &mut self,
        emit_page: &mut dyn FnMut(usize, u32),
        emit_group: &mut dyn FnMut(&[TypeId]),
    ) {
    }
    /// Consumes pending grouped removals, emitting their former group signatures.
    #[doc(hidden)]
    #[inline]
    #[allow(unused_variables)]
    fn collect_regroup_removals(&mut self, emit: &mut dyn FnMut(EntityId, &[TypeId])) {}
    /// Returns the group currently occupied by `entity`.
    #[doc(hidden)]
    #[inline]
    #[allow(unused_variables)]
    fn entity_group(&self, entity: EntityId) -> Option<&[TypeId]> {
        None
    }
    /// Moves `entity` into `group`, creating the local group when necessary.
    #[doc(hidden)]
    #[inline]
    #[allow(unused_variables)]
    fn move_to_group(&mut self, entity: EntityId, group: &[TypeId]) {}
    /// Moves a batch of entities into `group`.
    #[doc(hidden)]
    #[inline]
    #[allow(unused_variables)]
    fn move_to_group_batch(&mut self, entities: &[EntityId], group: &[TypeId]) {
        for &entity in entities {
            self.move_to_group(entity, group);
        }
    }
}
