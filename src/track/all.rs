use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{All, AllConst};
use crate::tracking::{
    is_track_within_bounds, map_deletion_data, DeletionTracking, InsertionTracking,
    ModificationTracking, RemovalOrDeletionTracking, RemovalTracking, Track, Tracking,
    TrackingTimestamp,
};

impl Sealed for Track<All> {}

impl Tracking for Track<All> {
    fn as_const() -> u32 {
        AllConst
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

    #[inline]
    fn remove<T: Component>(
        sparse_set: &mut SparseSet<T>,
        entity: EntityId,
        current: u32,
    ) -> Option<T> {
        let component = sparse_set.actual_remove(entity);

        if component.is_some() {
            sparse_set.removal_data.push((entity, current));
        }

        component
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

impl InsertionTracking for Track<All> {}
impl ModificationTracking for Track<All> {}
impl RemovalTracking for Track<All> {}
impl DeletionTracking for Track<All> {}
impl RemovalOrDeletionTracking for Track<All> {
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
            .chain(sparse_set.removal_data.iter().copied())
    }

    fn clear_all_removed_and_deleted<T: Component>(sparse_set: &mut SparseSet<T>) {
        sparse_set.deletion_data.clear();
        sparse_set.removal_data.clear();
    }

    fn clear_all_removed_and_deleted_older_than_timestamp<T: Component>(
        sparse_set: &mut SparseSet<T>,
        timestamp: TrackingTimestamp,
    ) {
        sparse_set.deletion_data.retain(|(_, t, _)| {
            is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t)
        });

        sparse_set
            .removal_data
            .retain(|(_, t)| is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t));
    }
}
