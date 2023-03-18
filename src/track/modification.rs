use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{Modification, ModificationConst};
use crate::tracking::{is_track_within_bounds, ModificationTracking, Track, Tracking};

impl Sealed for Track<Modification> {}

impl Tracking for Track<Modification> {
    fn as_const() -> u32 {
        ModificationConst
    }

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
}

impl ModificationTracking for Track<Modification> {}
