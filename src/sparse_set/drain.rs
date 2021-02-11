use crate::entity_id::EntityId;
use crate::iter::WithId;
use alloc::vec::Drain;

/// A draining iterator for [`SparseSet<T>`].
///
/// [`SparseSet<T>`]: crate::sparse_set::SparseSet
pub struct SparseSetDrain<'a, T> {
    pub(super) dense_ptr: *const EntityId,
    pub(super) dense_len: usize,
    pub(super) data: Drain<'a, T>,
}

impl<T> SparseSetDrain<'_, T> {
    /// Makes the iterator return which entity owns each component as well.
    pub fn with_id(self) -> WithId<Self> {
        WithId(self)
    }
}

impl<'a, T> Iterator for SparseSetDrain<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.data.next()
    }
}

impl<'a, T> Iterator for WithId<SparseSetDrain<'a, T>> {
    type Item = (EntityId, T);

    fn next(&mut self) -> Option<Self::Item> {
        let element = self.0.data.next()?;

        // SAFE this index is valid memory
        let id = unsafe {
            self.0
                .dense_ptr
                .add(self.0.dense_len - 1 - self.0.data.len())
                .read()
        };

        Some((id, element))
    }
}
