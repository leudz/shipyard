use crate::entity_id::EntityId;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hint::unreachable_unchecked;
use core::mem::size_of;

#[cfg(feature = "memory_constrained")]
pub(crate) const BUCKET_SIZE: usize = 256 / size_of::<EntityId>();
#[cfg(not(feature = "memory_constrained"))]
pub(crate) const BUCKET_SIZE: usize = 4096 / size_of::<EntityId>();

/// Internal part of a [`SparseSet`].
///
/// [`SparseSet`]: crate::sparse_set::SparseSet
#[derive(Clone)]
pub struct SparseArray(Vec<Option<Box<Page>>>);

#[cfg_attr(feature = "memory_constrained", repr(align(256)))]
#[cfg_attr(not(feature = "memory_constrained"), repr(align(4096)))]
#[derive(Clone)]
pub(crate) struct Page([EntityId; BUCKET_SIZE]);

impl SparseArray {
    #[inline]
    pub(super) fn new() -> Self {
        SparseArray(Vec::new())
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    pub(super) fn as_ptr(&self) -> *const Option<Box<Page>> {
        self.0.as_ptr()
    }
    #[inline]
    pub(super) fn as_mut_ptr(&mut self) -> *mut Option<Box<Page>> {
        self.0.as_mut_ptr()
    }
    pub(super) fn used_memory(&self) -> usize {
        self.0.len() * size_of::<Option<Box<EntityId>>>()
            + self.0.iter().fold(0, |count, array| {
                if array.is_some() {
                    count + size_of::<[EntityId; BUCKET_SIZE]>()
                } else {
                    count
                }
            })
    }
    pub(super) fn reserved_memory(&self) -> usize {
        self.0.capacity() * size_of::<Option<Box<[EntityId; BUCKET_SIZE]>>>()
            + self.0.iter().fold(0, |count, array| {
                if array.is_some() {
                    count + size_of::<[EntityId; BUCKET_SIZE]>()
                } else {
                    count
                }
            })
    }
}

impl SparseArray {
    #[inline]
    #[track_caller]
    pub(super) fn allocate_at(&mut self, entity: EntityId) {
        if entity.is_dead() {
            panic!("Tried to add a component with a dead entity.");
        }

        if entity.bucket() >= self.0.len() {
            self.0.resize(entity.bucket() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.bucket()
            let bucket = self.0.get_unchecked_mut(entity.bucket());

            if bucket.is_none() {
                *bucket = Some(Box::new(Page([EntityId::dead(); BUCKET_SIZE])));
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
                *bucket = Some(Box::new(Page([EntityId::dead(); BUCKET_SIZE])));
            }
        }
    }
    #[inline]
    pub(crate) fn get(&self, entity: EntityId) -> Option<EntityId> {
        self.0
            .get(entity.bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.0.get_unchecked(entity.bucket_index()) })
    }
    #[inline]
    pub(super) unsafe fn get_unchecked(&self, entity: EntityId) -> EntityId {
        match self.0.get_unchecked(entity.bucket()) {
            Some(bucket) => *bucket.0.get_unchecked(entity.bucket_index()),
            None => unreachable_unchecked(),
        }
    }
    #[inline]
    pub(crate) unsafe fn get_mut_unchecked(&mut self, entity: EntityId) -> &mut EntityId {
        match self.0.get_unchecked_mut(entity.bucket()) {
            Some(bucket) => bucket.0.get_unchecked_mut(entity.bucket_index()),
            None => unreachable_unchecked(),
        }
    }
    #[inline]
    #[allow(missing_docs)]
    pub fn contains(&self, entity: EntityId) -> bool {
        if let Some(sparse_entity) = self.get(entity) {
            sparse_entity.gen() == entity.gen()
        } else {
            false
        }
    }
}
