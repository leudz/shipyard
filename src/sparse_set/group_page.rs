use crate::sparse_set::bucket_index::BucketIndex;
use crate::sparse_set::BUCKET_SIZE;

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct GroupPage {
    bucket_indices: [BucketIndex; BUCKET_SIZE],
    pending_placement_mask: u32,
}

#[allow(dead_code)]
impl GroupPage {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            bucket_indices: [BucketIndex::UNCLASSIFIED; BUCKET_SIZE],
            pending_placement_mask: 0,
        }
    }

    #[inline]
    pub(super) fn bucket_index(&self, slot: usize) -> BucketIndex {
        debug_assert!(slot < BUCKET_SIZE);
        self.bucket_indices[slot]
    }

    #[inline]
    pub(super) fn set_bucket_index(&mut self, slot: usize, bucket: BucketIndex) {
        debug_assert!(slot < BUCKET_SIZE);
        self.bucket_indices[slot] = bucket;
    }

    #[inline]
    pub(super) fn set_pending_placement(&mut self, slot: usize) -> bool {
        debug_assert!(slot < BUCKET_SIZE);
        let was_empty = self.pending_placement_mask == 0;
        self.pending_placement_mask |= 1 << slot;
        was_empty
    }

    #[inline]
    pub(super) fn is_pending_placement(&self, slot: usize) -> bool {
        debug_assert!(slot < BUCKET_SIZE);
        self.pending_placement_mask & (1 << slot) != 0
    }

    #[inline]
    pub(super) fn pending_placement_mask(&self) -> u32 {
        self.pending_placement_mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_page_tracks_bucket_indices_and_pending_slots() {
        let mut page = GroupPage::new();

        for slot in 0..BUCKET_SIZE {
            assert_eq!(page.bucket_index(slot), BucketIndex::UNCLASSIFIED);
            assert!(!page.is_pending_placement(slot));
        }
        assert_eq!(page.pending_placement_mask(), 0);

        page.set_bucket_index(7, BucketIndex::from_group_index(2));
        assert_eq!(page.bucket_index(7).group_index(), Some(2));

        assert!(page.set_pending_placement(1));
        assert!(!page.set_pending_placement(1));
        assert!(!page.set_pending_placement(7));
        assert!(!page.set_pending_placement(31));
        assert_eq!(
            page.pending_placement_mask(),
            (1 << 1) | (1 << 7) | (1 << 31)
        );
    }
}
