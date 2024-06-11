use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::InsertionAndModification;
use crate::tracking::{InsertionTracking, ModificationTracking, Tracking, TrackingTimestamp};

impl Sealed for InsertionAndModification {}

impl Tracking for InsertionAndModification {
    const VALUE: u32 = 0b0011;

    fn name() -> &'static str {
        "Insertion and Modification"
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
}

impl InsertionTracking for InsertionAndModification {}
impl ModificationTracking for InsertionAndModification {}
