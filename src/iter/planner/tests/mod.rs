mod grouping_cover_sets;
mod grouping_selection_arithmetic;

use super::{Alternative, Child, GroupedCandidate, NodeMetadata, Route, Walk, WindowMetadata};
use crate::iter::EntityCursor;
use crate::sparse_set::{GroupRange, Groups};
use crate::storage::StorageId;
use alloc::vec::Vec;

/// A storage's size and group metadata, owned by a test for its window children to borrow.
struct Storage {
    groups: Groups,
    /// Number of components in the storage.
    len: usize,
}

impl Storage {
    /// Returns a storage `id` of `len` components, with `ranges` and its unevaluated region
    /// starting at `unevaluated_start`.
    fn new(
        id: StorageId,
        len: usize,
        unevaluated_start: usize,
        ranges: Vec<GroupRange>,
    ) -> Storage {
        Storage {
            groups: Groups {
                storage_id: Some(id),
                ranges,
                unevaluated_start,
            },
            len,
        }
    }

    /// Returns a window child over this storage, able to captain and to sail.
    fn child(&self) -> Child<'_> {
        Child {
            can_captain: true,
            can_sailor: true,
            sail_time: self.len,
            candidates: Some(self.len),
            metadata: NodeMetadata::Window(WindowMetadata {
                groups: &self.groups,
                len: self.len,
                entities: EntityCursor::empty(),
            }),
        }
    }
}

/// Returns a window child over each of `storages`, in order.
fn children<const N: usize>(storages: &[Storage; N]) -> [Child<'_>; N] {
    storages.each_ref().map(Storage::child)
}

/// Returns the [`WindowMetadata`] of each of `children`, by position, `None` for other children.
fn windows<'m>(children: &[Child<'m>]) -> Vec<Option<WindowMetadata<'m>>> {
    children
        .iter()
        .map(|child| match &child.metadata {
            NodeMetadata::Window(window) => Some(*window),
            _ => None,
        })
        .collect()
}

/// Returns the walks of `alternative`, a local alternative `select` reported for `children`.
fn local_candidate(children: &[Child<'_>], alternative: &Alternative) -> GroupedCandidate {
    let Route::Local { required } = alternative.route else {
        panic!("the alternative walks the children's own groups");
    };

    GroupedCandidate::new(&windows(children), required)
}

/// Returns `candidate`'s walks, in emission order.
fn walk_list(candidate: &GroupedCandidate) -> Vec<Walk> {
    candidate.walks().collect()
}
