#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BucketIndex(u16);

#[allow(dead_code)]
impl BucketIndex {
    pub(crate) const UNCLASSIFIED: Self = Self(0);

    #[inline]
    pub(crate) fn from_group_index(group_index: usize) -> Self {
        let bucket_index = group_index.checked_add(1).unwrap();
        assert!(bucket_index <= u16::MAX as usize);
        Self(bucket_index as u16)
    }

    #[inline]
    pub(crate) fn group_index(self) -> Option<usize> {
        self.0.checked_sub(1).map(usize::from)
    }

    #[inline]
    pub(crate) fn is_unclassified(self) -> bool {
        self == Self::UNCLASSIFIED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_indices_map_groups_to_physical_buckets() {
        assert!(BucketIndex::UNCLASSIFIED.is_unclassified());
        assert_eq!(BucketIndex::UNCLASSIFIED.group_index(), None);

        for group_index in [0, 1, 127, u16::MAX as usize - 1] {
            let bucket = BucketIndex::from_group_index(group_index);
            assert!(!bucket.is_unclassified());
            assert_eq!(bucket.group_index(), Some(group_index));
        }
    }
}
