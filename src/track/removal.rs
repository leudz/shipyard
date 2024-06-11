use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::Removal;
use crate::tracking::{
    map_deletion_data, RemovalOrDeletionTracking, RemovalTracking, Tracking, TrackingTimestamp,
};

impl Sealed for Removal {}

impl Tracking for Removal {
    const VALUE: u32 = 0b1000;

    fn name() -> &'static str {
        "Removal"
    }

    fn is_removed<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: TrackingTimestamp,
        current: TrackingTimestamp,
    ) -> bool {
        sparse_set
            .removal_data
            .iter()
            .any(|(id, timestamp)| *id == entity && timestamp.is_within(last, current))
    }
}

impl RemovalTracking for Removal {}
impl RemovalOrDeletionTracking for Removal {
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
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
}
