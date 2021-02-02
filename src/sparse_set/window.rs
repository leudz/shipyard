use super::SparseSet;
use crate::EntityId;
use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ptr;

pub struct FullRawWindowMut<'a, T> {
    sparse: *mut *mut EntityId,
    sparse_len: usize,
    pub(crate) dense: *mut EntityId,
    dense_len: usize,
    pub(crate) data: *mut T,
    pub(crate) is_tracking_modification: bool,
    _phantom: PhantomData<&'a mut T>,
}

unsafe impl<T: Send> Send for FullRawWindowMut<'_, T> {}

impl<'w, T> FullRawWindowMut<'w, T> {
    #[inline]
    pub(crate) fn new(sparse_set: &mut SparseSet<T>) -> Self {
        let sparse_len = sparse_set.sparse.len();
        let sparse: *mut Option<Box<[EntityId; super::BUCKET_SIZE]>> =
            sparse_set.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;
        let is_tracking_modification = sparse_set.is_tracking_modification();

        FullRawWindowMut {
            sparse,
            sparse_len,
            dense: sparse_set.dense.as_mut_ptr(),
            dense_len: sparse_set.dense.len(),
            data: sparse_set.data.as_mut_ptr(),
            is_tracking_modification,
            _phantom: PhantomData,
        }
    }
    #[inline]
    pub(crate) fn index_of(&self, entity: EntityId) -> Option<usize> {
        self.sparse_index(entity).and_then(|sparse_entity| {
            if entity.gen() == sparse_entity.gen() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        })
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    #[inline]
    pub(crate) unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        if let Some(index) = self.index_of(entity) {
            index
        } else {
            unreachable!()
        }
    }
    #[inline]
    fn sparse_index(&self, entity: EntityId) -> Option<EntityId> {
        if entity.bucket() < self.sparse_len {
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };

            if !bucket.is_null() {
                Some(unsafe { ptr::read(bucket.add(entity.bucket_index())) })
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl<T> Clone for FullRawWindowMut<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        FullRawWindowMut {
            sparse: self.sparse,
            sparse_len: self.sparse_len,
            dense: self.dense,
            dense_len: self.dense_len,
            data: self.data,
            is_tracking_modification: self.is_tracking_modification,
            _phantom: PhantomData,
        }
    }
}
