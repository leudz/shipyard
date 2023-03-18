use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{InsertionAndModification, InsertionConst, ModificationConst};
use crate::tracking::{
    is_track_within_bounds, InsertionTracking, ModificationTracking, Track, Tracking,
};

impl Sealed for Track<InsertionAndModification> {}

impl Tracking for Track<InsertionAndModification> {
    fn as_const() -> u32 {
        InsertionConst + ModificationConst
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

impl InsertionTracking for Track<InsertionAndModification> {}
impl ModificationTracking for Track<InsertionAndModification> {}
