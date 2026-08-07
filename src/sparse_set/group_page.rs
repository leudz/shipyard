use crate::sparse_set::bucket_index::BucketIndex;
use crate::sparse_set::BUCKET_SIZE;

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct GroupPage {
    bucket_indices: [BucketIndex; BUCKET_SIZE],
}

#[allow(dead_code)]
impl GroupPage {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            bucket_indices: [BucketIndex::UNCLASSIFIED; BUCKET_SIZE],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_page_tracks_bucket_indices() {
        let mut page = GroupPage::new();

        for slot in 0..BUCKET_SIZE {
            assert_eq!(page.bucket_index(slot), BucketIndex::UNCLASSIFIED);
        }

        page.set_bucket_index(7, BucketIndex::from_group_index(2));
        assert_eq!(page.bucket_index(7).group_index(), Some(2));
    }
}
