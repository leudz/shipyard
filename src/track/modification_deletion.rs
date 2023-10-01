use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{DeletionConst, ModificationAndDeletion, ModificationConst};
use crate::tracking::{
    map_deletion_data, DeletionTracking, ModificationTracking, RemovalOrDeletionTracking, Track,
    Tracking, TrackingTimestamp,
};

impl Sealed for Track<ModificationAndDeletion> {}

impl Tracking for Track<ModificationAndDeletion> {
    fn as_const() -> u32 {
        ModificationConst + DeletionConst
    }

    fn is_modified<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: TrackingTimestamp,
        current: TrackingTimestamp,
    ) -> bool {
        if let Some(dense) = sparse_set.index_of(entity) {
            sparse_set.modification_data[dense].is_within(last, current)
        } else {
            false
        }
    }

    fn is_deleted<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: TrackingTimestamp,
        current: TrackingTimestamp,
    ) -> bool {
        sparse_set
            .deletion_data
            .iter()
            .any(|(id, timestamp, _)| *id == entity && timestamp.is_within(last, current))
    }
}

impl ModificationTracking for Track<ModificationAndDeletion> {}
impl DeletionTracking for Track<ModificationAndDeletion> {}
impl RemovalOrDeletionTracking for Track<ModificationAndDeletion> {
    #[allow(trivial_casts)]
    fn removed_or_deleted<T: Component>(
        sparse_set: &SparseSet<T>,
    ) -> core::iter::Chain<
        core::iter::Map<
            core::slice::Iter<'_, (EntityId, TrackingTimestamp, T)>,
            for<'r> fn(&'r (EntityId, TrackingTimestamp, T)) -> (EntityId, TrackingTimestamp),
        >,
        core::iter::Copied<core::slice::Iter<'_, (EntityId, TrackingTimestamp)>>,
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
        sparse_set
            .deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));
    }
}
