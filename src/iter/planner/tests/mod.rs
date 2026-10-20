mod grouping_cover_sets;
mod grouping_selection_arithmetic;

use super::{Child, GroupedCandidate, NodeMetadata, Walk, WindowMetadata};
use crate::iter::EntityCursor;
use crate::sparse_set::GroupRange;
use crate::storage::StorageId;
use alloc::vec::Vec;

/// A storage's size and group ranges, owned by a test for its window children to borrow.
struct Storage {
    id: StorageId,
    /// Number of components in the storage.
    len: usize,
    /// Physical index where the unevaluated region starts.
    unevaluated_start: usize,
    ranges: Vec<GroupRange>,
}

impl Storage {
    /// Returns a window child over this storage, able to captain and to sail.
    fn child(&self) -> Child<'_> {
        Child {
            can_captain: true,
            can_sailor: true,
            sail_time: self.len,
            candidates: Some(self.len),
            entities: EntityCursor::empty(),
            metadata: NodeMetadata::Window(WindowMetadata {
                storage_id: self.id,
                len: self.len,
                unevaluated_start: self.unevaluated_start,
                ranges: &self.ranges,
            }),
        }
    }
}

/// Returns a window child over each of `storages`, in order.
fn children<const N: usize>(storages: &[Storage; N]) -> [Child<'_>; N] {
    storages.each_ref().map(Storage::child)
}

/// Returns `candidate`'s walks, in emission order.
fn walk_list(candidate: &GroupedCandidate) -> Vec<Walk> {
    candidate.walks().collect()
}
