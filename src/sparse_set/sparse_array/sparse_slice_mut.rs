use super::SparseSlice;
use crate::storage::EntityId;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

pub(crate) struct SparseSliceMut<'a, T>(pub(super) &'a mut [Option<Box<T>>]);

impl<'a, T> SparseSliceMut<'a, T> {
    pub(in crate::sparse_set) fn as_non_mut(&self) -> SparseSlice<'_, T> {
        SparseSlice(&self.0)
    }
    pub(in crate::sparse_set) fn len(&self) -> usize {
        self.0.len()
    }
    pub(in crate::sparse_set) fn as_mut_ptr(&mut self) -> *mut Option<Box<T>> {
        self.0.as_mut_ptr()
    }
    pub(in crate::sparse_set) fn reborrow(&mut self) -> SparseSliceMut<'_, T> {
        SparseSliceMut(self.0)
    }
}

impl<'a> SparseSliceMut<'a, [usize; crate::sparse_set::BUCKET_SIZE]> {
    pub(in crate::sparse_set) unsafe fn sparse_index_unchecked(&self, entity: EntityId) -> usize {
        *self
            .0
            .get_unchecked(entity.bucket())
            .as_ref()
            .unwrap_or_else(|| core::hint::unreachable_unchecked())
            .get_unchecked(entity.bucket_index())
    }
    pub(in crate::sparse_set) unsafe fn set_sparse_index_unchecked(
        &mut self,
        entity: EntityId,
        index: usize,
    ) {
        match self.0.get_unchecked_mut(entity.bucket()) {
            Some(bucket) => *bucket.get_unchecked_mut(entity.bucket_index()) = index,
            None => core::hint::unreachable_unchecked(),
        }
    }
}
