use crate::storage::EntityId;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::hint::unreachable_unchecked;

pub struct SparseArray<T>(Vec<Option<Box<T>>>);

impl<T> SparseArray<T> {
    #[inline]
    pub(super) fn new() -> Self {
        SparseArray(Vec::new())
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    pub(super) fn as_mut_ptr(&mut self) -> *mut Option<Box<T>> {
        self.0.as_mut_ptr()
    }
}

impl SparseArray<[EntityId; crate::sparse_set::BUCKET_SIZE]> {
    #[inline]
    pub(super) fn allocate_at(&mut self, entity: EntityId) {
        if entity.bucket() >= self.0.len() {
            self.0.resize(entity.bucket() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.bucket()
            let bucket = self.0.get_unchecked_mut(entity.bucket());

            if bucket.is_none() {
                *bucket = Some(Box::new([EntityId::dead(); crate::sparse_set::BUCKET_SIZE]));
            }
        }
    }
    pub(crate) fn bulk_allocate(&mut self, start: EntityId, end: EntityId) {
        if end.bucket() >= self.0.len() {
            self.0.resize(end.bucket() + 1, None);
        }
        for bucket_index in start.bucket()..end.bucket() + 1 {
            let bucket = unsafe { self.0.get_unchecked_mut(bucket_index) };

            if bucket.is_none() {
                *bucket = Some(Box::new([EntityId::dead(); crate::sparse_set::BUCKET_SIZE]));
            }
        }
    }
    #[inline]
    pub(super) fn get(&self, entity: EntityId) -> Option<EntityId> {
        self.0
            .get(entity.bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.bucket_index()) })
    }
    #[inline]
    pub(super) unsafe fn get_unchecked(&self, entity: EntityId) -> EntityId {
        match self.0.get_unchecked(entity.bucket()) {
            Some(bucket) => *bucket.get_unchecked(entity.bucket_index()),
            None => unreachable_unchecked(),
        }
    }
    #[inline]
    pub(crate) unsafe fn get_mut_unchecked(&mut self, entity: EntityId) -> &mut EntityId {
        match self.0.get_unchecked_mut(entity.bucket()) {
            Some(bucket) => bucket.get_unchecked_mut(entity.bucket_index()),
            None => unreachable_unchecked(),
        }
    }
    #[inline]
    pub(crate) unsafe fn get_at_unchecked(&self, index: usize) -> EntityId {
        match self.0.get_unchecked(index / crate::sparse_set::BUCKET_SIZE) {
            Some(bucket) => EntityId::new_from_parts(
                index as u64,
                bucket
                    .get_unchecked(index % crate::sparse_set::BUCKET_SIZE)
                    .index() as u16,
                0,
            ),
            None => unreachable_unchecked(),
        }
    }
}

impl SparseArray<[EntityId; crate::sparse_set::metadata::BUCKET_SIZE]> {
    #[inline]
    pub(crate) fn allocate_at(&mut self, entity: EntityId) {
        if entity.shared_bucket() >= self.0.len() {
            self.0.resize(entity.shared_bucket() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.bucket()
            if self.0.get_unchecked(entity.shared_bucket()).is_none() {
                *self.0.get_unchecked_mut(entity.shared_bucket()) = Some(Box::new(
                    [EntityId::dead(); crate::sparse_set::metadata::BUCKET_SIZE],
                ));
            }
        }
    }
    #[inline]
    pub(super) fn shared_index(&self, entity: EntityId) -> Option<EntityId> {
        self.0
            .get(entity.shared_bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.shared_bucket_index()) })
    }
    #[inline]
    pub(super) unsafe fn set_sparse_index_unchecked(&mut self, shared: EntityId, owned: EntityId) {
        self.allocate_at(shared);

        match self.0.get_unchecked_mut(shared.shared_bucket()) {
            Some(bucket) => *bucket.get_unchecked_mut(shared.shared_bucket_index()) = owned,
            None => unreachable_unchecked(),
        }
    }
    #[inline]
    pub(crate) fn is_valid(&self, index: usize) -> bool {
        if let Some(bucket) = self.0.get(index / crate::sparse_set::metadata::BUCKET_SIZE) {
            bucket
                .as_ref()
                .map(|bucket| unsafe {
                    *bucket.get_unchecked(index % crate::sparse_set::metadata::BUCKET_SIZE)
                })
                .filter(EntityId::is_dead)
                .is_none()
        } else {
            false
        }
    }
}
