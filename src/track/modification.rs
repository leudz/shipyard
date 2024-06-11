use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::Modification;
use crate::tracking::{ModificationTracking, Tracking, TrackingTimestamp};

impl Sealed for Modification {}

impl Tracking for Modification {
    const VALUE: u32 = 0b0010;

    fn name() -> &'static str {
        "Modification"
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

impl ModificationTracking for Modification {}
