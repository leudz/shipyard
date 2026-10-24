use crate::entity_id::EntityId;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hint::unreachable_unchecked;
use core::mem::size_of;

#[cfg(feature = "memory_constrained")]
pub(crate) const PAGE_LEN: usize = 256 / size_of::<EntityId>();
#[cfg(not(feature = "memory_constrained"))]
pub(crate) const PAGE_LEN: usize = 4096 / size_of::<EntityId>();

/// Internal part of a [`SparseSet`].
///
/// [`SparseSet`]: crate::sparse_set::SparseSet
#[derive(Clone)]
pub struct SparseArray(Vec<Option<Box<Page>>>);

#[cfg_attr(feature = "memory_constrained", repr(C, align(256)))]
#[cfg_attr(not(feature = "memory_constrained"), repr(C, align(4096)))]
#[derive(Clone)]
pub(crate) struct Page([EntityId; PAGE_LEN]);

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
                    count + size_of::<[EntityId; PAGE_LEN]>()
                } else {
                    count
                }
            })
    }
    pub(super) fn reserved_memory(&self) -> usize {
        self.0.capacity() * size_of::<Option<Box<[EntityId; PAGE_LEN]>>>()
            + self.0.iter().fold(0, |count, array| {
                if array.is_some() {
                    count + size_of::<[EntityId; PAGE_LEN]>()
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

        if entity.page_index() >= self.0.len() {
            self.0.resize(entity.page_index() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.page_index()
            let page = self.0.get_unchecked_mut(entity.page_index());

            if page.is_none() {
                *page = Some(Box::new(Page([EntityId::dead(); PAGE_LEN])));
            }
        }
    }
    pub(crate) fn bulk_allocate(&mut self, start: EntityId, end: EntityId) {
        if end.page_index() >= self.0.len() {
            self.0.resize(end.page_index() + 1, None);
        }
        for page_index in start.page_index()..end.page_index() + 1 {
            let page = unsafe { self.0.get_unchecked_mut(page_index) };

            if page.is_none() {
                *page = Some(Box::new(Page([EntityId::dead(); PAGE_LEN])));
            }
        }
    }
    #[inline]
    pub(crate) fn get(&self, entity: EntityId) -> Option<EntityId> {
        let Some(Some(page)) = self.0.get(entity.page_index()) else {
            return None;
        };

        Some(unsafe { *page.0.get_unchecked(entity.page_offset()) })
    }
    #[inline]
    pub(super) unsafe fn get_unchecked(&self, entity: EntityId) -> EntityId {
        // SAFE: Our caller guarantees `entity`'s page is allocated.
        unsafe {
            match self.0.get_unchecked(entity.page_index()) {
                Some(page) => *page.0.get_unchecked(entity.page_offset()),
                None => unreachable_unchecked(),
            }
        }
    }
    #[inline]
    pub(crate) unsafe fn get_mut_unchecked(&mut self, entity: EntityId) -> &mut EntityId {
        // SAFE: Our caller guarantees `entity`'s page is allocated.
        unsafe {
            match self.0.get_unchecked_mut(entity.page_index()) {
                Some(page) => page.0.get_unchecked_mut(entity.page_offset()),
                None => unreachable_unchecked(),
            }
        }
    }
    #[inline]
    #[allow(missing_docs)]
    pub fn contains(&self, entity: EntityId) -> bool {
        if let Some(sparse_entity) = self.get(entity) {
            sparse_entity.generation() == entity.generation()
        } else {
            false
        }
    }
}
