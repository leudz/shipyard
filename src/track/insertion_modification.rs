use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{Insertion, Modification};
use crate::tracking::{
    is_track_within_bounds, InsertionTracking, ModificationTracking, Track, Tracking,
};

impl Sealed for Track<{ Insertion + Modification }> {}

impl Tracking for Track<{ Insertion + Modification }> {
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

impl InsertionTracking for Track<{ Insertion + Modification }> {}
impl ModificationTracking for Track<{ Insertion + Modification }> {}
