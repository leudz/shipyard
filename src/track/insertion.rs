use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::Insertion;
use crate::tracking::{InsertionTracking, Tracking, TrackingTimestamp};

impl Sealed for Insertion {}

impl Tracking for Insertion {
    const VALUE: u32 = 0b0001;

    fn name() -> &'static str {
        "Insertion"
    }

    #[inline]
    fn is_inserted<T: Component>(
        sparse_set: &SparseSet<T>,
        entity: EntityId,
        last: TrackingTimestamp,
        current: TrackingTimestamp,
    ) -> bool {
        if let Some(dense) = sparse_set.index_of(entity) {
            sparse_set.insertion_data[dense].is_within(last, current)
        } else {
            false
        }
    }
}

impl InsertionTracking for Insertion {}
