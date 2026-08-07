use crate::entity_id::EntityId;
use crate::sparse_set::BUCKET_SIZE;
use crate::sparse_set::{bucket_index::BucketIndex, group_page::GroupPage};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hint::unreachable_unchecked;
use core::mem::size_of;

/// Internal part of a [`SparseSet`].
///
/// [`SparseSet`]: crate::sparse_set::SparseSet
#[derive(Clone)]
pub struct SparseArray {
    ids: Vec<Option<Box<[EntityId; BUCKET_SIZE]>>>,
    group_pages: Vec<Option<Box<GroupPage>>>,
    pending_placement_masks: Vec<u32>,
    pending_placement_pages: Vec<usize>,
}

impl SparseArray {
    #[inline]
    pub(super) fn new() -> Self {
        SparseArray {
            ids: Vec::new(),
            group_pages: Vec::new(),
            pending_placement_masks: Vec::new(),
            pending_placement_pages: Vec::new(),
        }
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }
    #[inline]
    pub(super) fn as_ptr(&self) -> *const Option<Box<[EntityId; BUCKET_SIZE]>> {
        self.ids.as_ptr()
    }
    #[inline]
    pub(super) fn as_mut_ptr(&mut self) -> *mut Option<Box<[EntityId; BUCKET_SIZE]>> {
        self.ids.as_mut_ptr()
    }
    pub(super) fn used_memory(&self) -> usize {
        self.ids.len() * size_of::<Option<Box<[EntityId; BUCKET_SIZE]>>>()
            + self.ids.iter().fold(0, |count, array| {
                if array.is_some() {
                    count + size_of::<[EntityId; BUCKET_SIZE]>()
                } else {
                    count
                }
            })
            + self.group_pages.len() * size_of::<Option<Box<GroupPage>>>()
            + self.group_pages.iter().fold(0, |count, page| {
                if page.is_some() {
                    count + size_of::<GroupPage>()
                } else {
                    count
                }
            })
            + self.pending_placement_masks.len() * size_of::<u32>()
            + self.pending_placement_pages.len() * size_of::<usize>()
    }
    pub(super) fn reserved_memory(&self) -> usize {
        self.ids.capacity() * size_of::<Option<Box<[EntityId; BUCKET_SIZE]>>>()
            + self.ids.iter().fold(0, |count, array| {
                if array.is_some() {
                    count + size_of::<[EntityId; BUCKET_SIZE]>()
                } else {
                    count
                }
            })
            + self.group_pages.capacity() * size_of::<Option<Box<GroupPage>>>()
            + self.group_pages.iter().fold(0, |count, page| {
                if page.is_some() {
                    count + size_of::<GroupPage>()
                } else {
                    count
                }
            })
            + self.pending_placement_masks.capacity() * size_of::<u32>()
            + self.pending_placement_pages.capacity() * size_of::<usize>()
    }
}

impl SparseArray {
    #[inline]
    #[track_caller]
    pub(super) fn allocate_at(&mut self, entity: EntityId) {
        if entity.is_dead() {
            panic!("Tried to add a component with a dead entity.");
        }

        if entity.bucket() >= self.ids.len() {
            self.ids.resize(entity.bucket() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.bucket()
            let bucket = self.ids.get_unchecked_mut(entity.bucket());

            if bucket.is_none() {
                *bucket = Some(Box::new([EntityId::dead(); BUCKET_SIZE]));
            }
        }
    }
    pub(crate) fn bulk_allocate(&mut self, start: EntityId, end: EntityId) {
        if end.bucket() >= self.ids.len() {
            self.ids.resize(end.bucket() + 1, None);
        }
        for bucket_index in start.bucket()..end.bucket() + 1 {
            let bucket = unsafe { self.ids.get_unchecked_mut(bucket_index) };

            if bucket.is_none() {
                *bucket = Some(Box::new([EntityId::dead(); BUCKET_SIZE]));
            }
        }
    }
    #[inline]
    pub(crate) fn get(&self, entity: EntityId) -> Option<EntityId> {
        self.ids
            .get(entity.bucket())?
            .as_ref()
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.bucket_index()) })
    }
    #[inline]
    pub(super) unsafe fn get_unchecked(&self, entity: EntityId) -> EntityId {
        match self.ids.get_unchecked(entity.bucket()) {
            Some(bucket) => *bucket.get_unchecked(entity.bucket_index()),
            None => unreachable_unchecked(),
        }
    }
    #[inline]
    pub(crate) unsafe fn get_mut_unchecked(&mut self, entity: EntityId) -> &mut EntityId {
        match self.ids.get_unchecked_mut(entity.bucket()) {
            Some(bucket) => bucket.get_unchecked_mut(entity.bucket_index()),
            None => unreachable_unchecked(),
        }
    }

    #[inline]
    fn group_page_mut_or_insert(&mut self, page_index: usize) -> &mut GroupPage {
        if page_index >= self.group_pages.len() {
            self.group_pages.resize(page_index + 1, None);
        }

        self.group_pages[page_index]
            .get_or_insert_with(|| Box::new(GroupPage::new()))
            .as_mut()
    }

    #[inline]
    fn insert_pending_placement_page(&mut self, page_index: usize) {
        let can_push = match self.pending_placement_pages.last() {
            Some(&last) => last < page_index,
            None => true,
        };

        if can_push {
            self.pending_placement_pages.push(page_index);
            return;
        }

        match self.pending_placement_pages.binary_search(&page_index) {
            Ok(_) => {}
            Err(position) => self.pending_placement_pages.insert(position, page_index),
        }
    }

    #[inline]
    pub(crate) fn set_pending_placement(&mut self, entity: EntityId) {
        debug_assert!(matches!(
            self.get(entity),
            Some(sparse_entity)
                if !sparse_entity.is_dead() && sparse_entity.gen() == entity.gen()
        ));

        if entity.bucket() >= self.pending_placement_masks.len() {
            self.pending_placement_masks.resize(entity.bucket() + 1, 0);
        }

        let mask = unsafe {
            // SAFE we just allocated at least entity.bucket()
            self.pending_placement_masks
                .get_unchecked_mut(entity.bucket())
        };
        let became_pending_page = *mask == 0;
        *mask |= 1 << entity.bucket_index();

        if became_pending_page {
            self.insert_pending_placement_page(entity.bucket());
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn pending_placement_pages(&self) -> &[usize] {
        &self.pending_placement_pages
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn pending_placement_mask(&self, page_index: usize) -> u32 {
        self.pending_placement_masks
            .get(page_index)
            .copied()
            .unwrap_or(0)
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn bucket_index(&self, entity: EntityId) -> BucketIndex {
        self.group_pages
            .get(entity.bucket())
            .and_then(Option::as_deref)
            .map_or(BucketIndex::UNCLASSIFIED, |page| {
                page.bucket_index(entity.bucket_index())
            })
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn set_bucket_index(&mut self, entity: EntityId, bucket: BucketIndex) {
        self.group_page_mut_or_insert(entity.bucket())
            .set_bucket_index(entity.bucket_index(), bucket);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_live(array: &mut SparseArray, entity: EntityId) {
        array.allocate_at(entity);
        unsafe {
            *array.get_mut_unchecked(entity) = EntityId::new_from_index_and_gen(0, entity.gen());
        }
    }

    #[test]
    fn pending_masks_are_lazily_allocated_and_pages_are_sorted_unique() {
        let mut array = SparseArray::new();
        let entities = [95, 64, 32, 31, 33, 0].map(EntityId::new);

        for entity in entities {
            insert_live(&mut array, entity);
        }

        assert!(array.pending_placement_masks.is_empty());
        assert!(array.group_pages.is_empty());

        for entity in entities {
            array.set_pending_placement(entity);
        }

        array.set_pending_placement(EntityId::new(33));
        array.set_pending_placement(EntityId::new(31));

        assert_eq!(array.pending_placement_pages(), &[0, 1, 2]);
        assert_eq!(array.pending_placement_mask(0), (1 << 0) | (1 << 31));
        assert_eq!(array.pending_placement_mask(1), (1 << 0) | (1 << 1));
        assert_eq!(array.pending_placement_mask(2), (1 << 0) | (1 << 31));
        assert_eq!(array.pending_placement_masks.len(), 3);
        assert!(array.group_pages.is_empty());
    }

    #[test]
    fn pending_placement_preserves_unclassified_bucket_and_clone_state() {
        let mut array = SparseArray::new();
        let entity = EntityId::new(42);
        insert_live(&mut array, entity);

        array.set_pending_placement(entity);

        assert!(array.bucket_index(entity).is_unclassified());
        assert!(array.group_pages.is_empty());

        let clone = array.clone();
        assert_eq!(clone.pending_placement_pages(), &[1]);
        assert_eq!(clone.pending_placement_mask(1), 1 << 10);
        assert!(clone.bucket_index(entity).is_unclassified());
        assert!(clone.group_pages.is_empty());
    }

    #[test]
    fn bucket_index_allocates_group_page_without_pending_mask() {
        let mut array = SparseArray::new();
        let entity = EntityId::new(42);

        array.set_bucket_index(entity, BucketIndex::from_group_index(4));

        assert_eq!(array.bucket_index(entity).group_index(), Some(4));
        assert!(array.pending_placement_masks.is_empty());
        assert!(array.pending_placement_pages.is_empty());
    }

    #[test]
    fn missing_group_pages_have_unclassified_bucket_and_empty_mask() {
        let array = SparseArray::new();
        let entity = EntityId::new(96);

        assert_eq!(array.bucket_index(entity), BucketIndex::UNCLASSIFIED);
        assert_eq!(array.pending_placement_mask(3), 0);
        assert!(array.pending_placement_pages().is_empty());
    }
}
