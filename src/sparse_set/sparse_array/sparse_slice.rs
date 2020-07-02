use crate::storage::EntityId;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

#[derive(Clone, Copy)]
pub(crate) struct SparseSlice<'a, T>(pub(super) &'a [Option<Box<T>>]);

impl<'a> SparseSlice<'a, [usize; crate::sparse_set::BUCKET_SIZE]> {
    pub(in crate::sparse_set) fn sparse_index(&self, entity: EntityId) -> Option<usize> {
        // SAFE bucket_index always returns a valid bucket index
        self.0
            .get(entity.bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.bucket_index()) })
    }
}
