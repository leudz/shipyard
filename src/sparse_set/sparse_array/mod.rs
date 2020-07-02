mod sparse_slice;
mod sparse_slice_mut;

pub(crate) use sparse_slice::SparseSlice;
pub(crate) use sparse_slice_mut::SparseSliceMut;

use crate::storage::EntityId;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub(crate) struct SparseArray<T>(pub(super) Vec<Option<Box<T>>>);

impl<T> SparseArray<T> {
    pub(super) fn as_slice(&self) -> SparseSlice<'_, T> {
        SparseSlice(&*self.0)
    }
    pub(super) fn as_slice_mut(&mut self) -> SparseSliceMut<'_, T> {
        SparseSliceMut(&mut *self.0)
    }
}

impl SparseArray<[usize; crate::sparse_set::BUCKET_SIZE]> {
    pub(super) fn sparse_index(&self, entity: EntityId) -> Option<usize> {
        // SAFE bucket_index always returns a valid bucket index
        self.0
            .get(entity.bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.bucket_index()) })
    }
    pub(super) unsafe fn set_sparse_index_unchecked(&mut self, entity: EntityId, index: usize) {
        match self.0.get_unchecked_mut(entity.bucket()) {
            Some(bucket) => *bucket.get_unchecked_mut(entity.bucket_index()) = index,
            None => core::hint::unreachable_unchecked(),
        }
    }
}

impl SparseArray<[EntityId; crate::sparse_set::metadata::BUCKET_SIZE]> {
    pub(crate) fn allocate_at(&mut self, entity: EntityId) {
        if entity.bucket() >= self.0.len() {
            self.0.resize(entity.bucket() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.bucket()
            if self.0.get_unchecked(entity.bucket()).is_none() {
                *self.0.get_unchecked_mut(entity.bucket()) = Some(Box::new(
                    [EntityId::dead(); crate::sparse_set::metadata::BUCKET_SIZE],
                ));
            }
        }
    }
    pub(super) fn shared_index(&self, entity: EntityId) -> Option<EntityId> {
        self.0
            .get(entity.bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.bucket_index()) })
    }
    pub(super) unsafe fn set_sparse_index_unchecked(&mut self, shared: EntityId, owned: EntityId) {
        self.allocate_at(shared);

        match self.0.get_unchecked_mut(shared.bucket()) {
            Some(bucket) => *bucket.get_unchecked_mut(shared.bucket_index()) = owned,
            None => core::hint::unreachable_unchecked(),
        }
    }
}
