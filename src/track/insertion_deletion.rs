use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{Deletion, Insertion};
use crate::tracking::{
    is_track_within_bounds, map_deletion_data, DeletionTracking, InsertionTracking,
    RemovalOrDeletionTracking, Track, Tracking, TrackingTimestamp,
};

impl Sealed for Track<{ Insertion + Deletion }> {}

impl Tracking for Track<{ Insertion + Deletion }> {
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

    fn is_deleted<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        sparse_set.deletion_data.iter().any(|(id, timestamp, _)| {
            *id == entity && is_track_within_bounds(*timestamp, last, current)
        })
    }

    #[inline]
    fn delete<T: Component>(sparse_set: &mut SparseSet<T>, entity: EntityId, current: u32) -> bool {
        if let Some(component) = sparse_set.actual_remove(entity) {
            sparse_set.deletion_data.push((entity, current, component));

            true
        } else {
            false
        }
    }
}

impl InsertionTracking for Track<{ Insertion + Deletion }> {}
impl DeletionTracking for Track<{ Insertion + Deletion }> {}
impl RemovalOrDeletionTracking for Track<{ Insertion + Deletion }> {
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
        sparse_set
            .deletion_data
            .iter()
            .map(map_deletion_data as _)
            .chain([].iter().copied())
    }

    fn clear_all_removed_and_deleted<T: Component>(sparse_set: &mut SparseSet<T>) {
        sparse_set.deletion_data.clear();
    }

    fn clear_all_removed_and_deleted_older_than_timestamp<T: Component>(
        sparse_set: &mut SparseSet<T>,
        timestamp: TrackingTimestamp,
    ) {
        sparse_set.deletion_data.retain(|(_, t, _)| {
            is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t)
        });
    }
}
