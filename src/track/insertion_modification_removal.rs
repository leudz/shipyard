use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{
    InsertionAndModificationAndRemoval, InsertionConst, ModificationConst, RemovalConst,
};
use crate::tracking::{
    is_track_within_bounds, map_deletion_data, InsertionTracking, ModificationTracking,
    RemovalOrDeletionTracking, RemovalTracking, Track, Tracking, TrackingTimestamp,
};

impl Sealed for Track<InsertionAndModificationAndRemoval> {}

impl Tracking for Track<InsertionAndModificationAndRemoval> {
    fn as_const() -> u32 {
        InsertionConst + ModificationConst + RemovalConst
    }

    #[inline]
    fn is_inserted<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        if let Some(dense) = sparse_set.index_of(entity) {
            is_track_within_bounds(sparse_set.insertion_data[dense], last, current)
        } else {
            false
        }
    }

    #[inline]
    fn is_modified<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        if let Some(dense) = sparse_set.index_of(entity) {
            is_track_within_bounds(sparse_set.modification_data[dense], last, current)
        } else {
            false
        }
    }

    fn is_removed<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        sparse_set.removal_data.iter().any(|(id, timestamp)| {
            *id == entity && is_track_within_bounds(*timestamp, last, current)
        })
    }
}

impl InsertionTracking for Track<InsertionAndModificationAndRemoval> {}
impl ModificationTracking for Track<InsertionAndModificationAndRemoval> {}
impl RemovalTracking for Track<InsertionAndModificationAndRemoval> {}
impl RemovalOrDeletionTracking for Track<InsertionAndModificationAndRemoval> {
    #[allow(trivial_casts)]
    fn removed_or_deleted<T: Component>(
        sparse_set: &SparseSet<T>,
    ) -> core::iter::Chain<
        core::iter::Map<
            core::slice::Iter<'_, (EntityId, u32, T)>,
            for<'r> fn(&'r (EntityId, u32, T)) -> (EntityId, u32),
        >,
        core::iter::Copied<core::slice::Iter<'_, (EntityId, u32)>>,
    > {
        [].iter()
            .map(map_deletion_data as _)
            .chain(sparse_set.removal_data.iter().copied())
    }

    fn clear_all_removed_and_deleted<T: Component>(sparse_set: &mut SparseSet<T>) {
        sparse_set.removal_data.clear();
    }

    fn clear_all_removed_and_deleted_older_than_timestamp<T: Component>(
        sparse_set: &mut SparseSet<T>,
        timestamp: TrackingTimestamp,
    ) {
        sparse_set
            .removal_data
            .retain(|(_, t)| is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t));
    }
}
