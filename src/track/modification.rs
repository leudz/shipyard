use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{Modification, ModificationConst};
use crate::tracking::{ModificationTracking, Track, Tracking, TrackingTimestamp};

impl Sealed for Track<Modification> {}

impl Tracking for Track<Modification> {
    fn as_const() -> u32 {
        ModificationConst
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

impl ModificationTracking for Track<Modification> {}
