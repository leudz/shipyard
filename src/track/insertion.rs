use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{Insertion, InsertionConst};
use crate::tracking::{is_track_within_bounds, InsertionTracking, Track, Tracking};

impl Sealed for Track<Insertion> {}

impl Tracking for Track<Insertion> {
    fn as_const() -> u32 {
        InsertionConst
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
}

impl InsertionTracking for Track<Insertion> {}
