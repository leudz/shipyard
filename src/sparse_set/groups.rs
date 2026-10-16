use crate::storage::StorageId;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

/// Grouping metadata of a [`SparseSet`](crate::sparse_set::SparseSet).
///
/// A grouped storage's vectors are split into contiguous regions:
///
/// ```text
/// [ Group range(s) ][ Ungrouped ][ Unevaluated ]
/// ```
///
/// A storage without any range is entirely unevaluated, which is how storages behave without grouping.
#[doc(hidden)]
#[derive(Default)]
pub struct Groups {
    /// This storage's own id.
    pub(crate) storage_id: Option<StorageId>,
    /// In physical order, contiguous from index 0.
    pub(crate) ranges: Vec<GroupRange>,
    /// Physical index where the unevaluated region starts.
    pub(crate) unevaluated_start: usize,
}

/// The part of a storage's vectors that holds a group's components.
pub(crate) struct GroupRange {
    /// The group's sorted member ids. Shared by every member and compared by pointer.
    pub(crate) group: Arc<[StorageId]>,
    pub(crate) start: usize,
    pub(crate) len: usize,
    /// Rotation of the range once ranges become rings. Always 0 for now.
    pub(crate) shift: isize,
    /// Length of the prefix that is unchanged since the last regroup.
    pub(crate) trust: usize,
}

/// A region of a grouped storage, used as the destination of a regroup move.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub enum GroupRegion<'a> {
    /// The range of the group with these sorted member ids.
    Range(&'a Arc<[StorageId]>),
    /// Components that regroup evaluated and found to belong to no group.
    Ungrouped,
    /// Components that regroup hasn't looked at yet.
    Unevaluated,
}

impl GroupRange {
    /// Returns the physical index right after this range.
    pub(crate) fn end(&self) -> usize {
        self.start + self.len
    }
}

impl Groups {
    /// Returns `true` if at least one group range is attached to this storage.
    #[inline]
    pub(crate) fn has_ranges(&self) -> bool {
        !self.ranges.is_empty()
    }

    /// Returns the physical index where the ungrouped region starts.
    pub(crate) fn grouped_end(&self) -> usize {
        self.ranges.last().map_or(0, GroupRange::end)
    }

    /// Returns the physical indices of the ungrouped region.
    #[allow(dead_code, reason = "Used once grouped iteration lands.")]
    pub(crate) fn ungrouped_range(&self) -> Range<usize> {
        self.grouped_end()..self.unevaluated_start
    }

    /// Returns the physical indices of the unevaluated region.
    #[allow(dead_code, reason = "Used once grouped iteration lands.")]
    pub(crate) fn unevaluated_range(&self, dense_len: usize) -> Range<usize> {
        self.unevaluated_start..dense_len
    }

    /// Returns the region holding the component at physical `index`.
    pub(crate) fn region_of(&self, index: usize) -> GroupRegion<'_> {
        if let Some(range) = self
            .ranges
            .iter()
            .find(|range| range.start <= index && index < range.end())
        {
            GroupRegion::Range(&range.group)
        } else if index < self.unevaluated_start {
            GroupRegion::Ungrouped
        } else {
            GroupRegion::Unevaluated
        }
    }

    /// Returns `group`'s range in this storage.
    ///
    /// Groups are compared by pointer, an equal group allocated separately doesn't match.
    pub(crate) fn range(&self, group: &Arc<[StorageId]>) -> Option<&GroupRange> {
        self.ranges
            .iter()
            .find(|range| Arc::ptr_eq(&range.group, group))
    }

    /// Returns `group`'s range in this storage.
    ///
    /// Groups are compared by pointer, an equal group allocated separately doesn't match.
    #[allow(dead_code, reason = "Used once storages can hold several ranges.")]
    pub(crate) fn range_mut(&mut self, group: &Arc<[StorageId]>) -> Option<&mut GroupRange> {
        self.ranges
            .iter_mut()
            .find(|range| Arc::ptr_eq(&range.group, group))
    }

    /// Attaches an empty range for `group` after the last range.
    ///
    /// No component moves. Components that were never grouped stay in the unevaluated region.
    pub(crate) fn attach(&mut self, group: Arc<[StorageId]>, storage_id: StorageId) {
        let start = self.grouped_end();

        self.ranges.push(GroupRange {
            group,
            start,
            len: 0,
            shift: 0,
            trust: 0,
        });
        self.storage_id = Some(storage_id);
    }

    /// Returns the group of a range whose member ids are equal to `ids`.
    ///
    /// This compares by value and is only meant to detect duplicate declarations.\
    /// Everywhere else groups are compared by pointer.
    pub(crate) fn find_by_value(&self, ids: &[StorageId]) -> Option<&Arc<[StorageId]>> {
        self.ranges
            .iter()
            .map(|range| &range.group)
            .find(|group| group[..] == *ids)
    }

    /// Makes the whole storage unevaluated, without moving any component.
    ///
    /// Ranges stay attached, so group definitions survive.
    pub(crate) fn reset(&mut self) {
        // This is a temporary reset rule.
        // Any mutation that changes a grouped storage's layout lands here until
        // the removal design replaces it.
        // Iteration stays correct: our trust is 0 so every group's aligned part is empty,
        // and all our components are found in the unevaluated region.
        for range in &mut self.ranges {
            range.start = 0;
            range.len = 0;
            range.trust = 0;
        }

        self.unevaluated_start = 0;
    }

    /// Panics if the regions don't describe a valid layout for a storage of `dense_len` components.
    #[cfg(debug_assertions)]
    #[track_caller]
    pub(crate) fn assert_invariants(&self, dense_len: usize) {
        let mut previous_end = 0;
        for range in &self.ranges {
            assert_eq!(
                range.start, previous_end,
                "group ranges must tile the start of the storage in order"
            );
            assert!(
                range.trust <= range.len,
                "a range's trust cannot exceed its length"
            );
            assert_eq!(range.shift, 0, "ranges cannot be rotated yet");

            previous_end = range.end();
        }

        // Temporary until ranges become rings and storages can hold several of them.
        assert!(
            self.ranges.len() <= 1,
            "a storage can only belong to one group for now"
        );

        if self.ranges.is_empty() {
            assert_eq!(
                self.unevaluated_start, 0,
                "a storage without group ranges must be fully unevaluated"
            );
        }

        assert!(
            self.grouped_end() <= self.unevaluated_start,
            "the unevaluated region cannot start inside a group range"
        );
        assert!(
            self.unevaluated_start <= dense_len,
            "the unevaluated region cannot start past the end of the storage"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn group(ids: &[u64]) -> Arc<[StorageId]> {
        ids.iter().map(|&id| StorageId::Custom(id)).collect()
    }

    fn range(group: &Arc<[StorageId]>, start: usize, len: usize) -> GroupRange {
        GroupRange {
            group: group.clone(),
            start,
            len,
            shift: 0,
            trust: len,
        }
    }

    /// Asserts that a storage without ranges is entirely unevaluated.
    #[test]
    fn regions_empty() {
        let groups = Groups::default();

        assert!(!groups.has_ranges());
        assert_eq!(groups.grouped_end(), 0);
        assert_eq!(groups.ungrouped_range(), 0..0);
        assert_eq!(groups.unevaluated_range(5), 0..5);
    }

    /// Asserts that with one range of 3 and 2 ungrouped components,
    /// the regions of a storage of 8 components are `[0, 3)`, `[3, 5)` and `[5, 8)`.
    #[test]
    fn regions_one() {
        let ab = group(&[0, 1]);
        let groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 3)],
            unevaluated_start: 5,
        };

        assert!(groups.has_ranges());
        assert_eq!(groups.grouped_end(), 3);
        assert_eq!(groups.ungrouped_range(), 3..5);
        assert_eq!(groups.unevaluated_range(8), 5..8);
    }

    /// Asserts that with ranges of 2 and 3 and 1 ungrouped component,
    /// the grouped part ends after the second range.
    #[test]
    fn regions_two() {
        let ab = group(&[0, 1]);
        let ac = group(&[0, 2]);
        let groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 2), range(&ac, 2, 3)],
            unevaluated_start: 6,
        };

        assert_eq!(groups.grouped_end(), 5);
        assert_eq!(groups.ungrouped_range(), 5..6);
        assert_eq!(groups.unevaluated_range(9), 6..9);
    }

    /// Asserts that with one range of 2 and 2 ungrouped components,
    /// `region_of` maps indices 0 and 1 to the range, 2 and 3 to the ungrouped region and 4 to the unevaluated region.
    #[test]
    fn region_of() {
        let ab = group(&[0, 1]);
        let groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 2)],
            unevaluated_start: 4,
        };

        for index in 0..2 {
            assert!(
                matches!(groups.region_of(index), GroupRegion::Range(group) if Arc::ptr_eq(group, &ab))
            );
        }
        for index in 2..4 {
            assert!(matches!(groups.region_of(index), GroupRegion::Ungrouped));
        }
        assert!(matches!(groups.region_of(4), GroupRegion::Unevaluated));
    }

    /// Asserts that `region_of` maps every index of a storage without ranges to the unevaluated region.
    #[test]
    fn region_of_empty() {
        let groups = Groups::default();

        assert!(matches!(groups.region_of(0), GroupRegion::Unevaluated));
        assert!(matches!(groups.region_of(3), GroupRegion::Unevaluated));
    }

    /// Asserts that `range` finds a group by pointer, among several ranges.
    #[test]
    fn lookup() {
        let ab = group(&[0, 1]);
        let ac = group(&[0, 2]);
        let groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 2), range(&ac, 2, 3)],
            unevaluated_start: 5,
        };

        assert_eq!(groups.range(&ac).unwrap().start, 2);
    }

    /// Asserts that `range` doesn't match an equal group that was allocated separately.
    #[test]
    fn lookup_by_pointer() {
        let ab = group(&[0, 1]);
        let groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 2)],
            unevaluated_start: 2,
        };

        let other_ab = group(&[0, 1]);

        assert_eq!(ab, other_ab);
        assert!(groups.range(&other_ab).is_none());
    }

    /// Asserts that `range_mut` finds a group by pointer and modifies it in place.
    #[test]
    fn lookup_mut() {
        let ab = group(&[0, 1]);
        let mut groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 2)],
            unevaluated_start: 2,
        };

        groups.range_mut(&ab).unwrap().trust = 1;

        assert_eq!(groups.ranges[0].trust, 1);
        assert!(groups.range_mut(&group(&[0, 1])).is_none());
    }

    /// Asserts that `reset` empties every range and the ungrouped region but keeps ranges attached.
    #[test]
    fn reset() {
        let ab = group(&[0, 1]);
        let ac = group(&[0, 2]);
        let mut groups = Groups {
            storage_id: Some(StorageId::Custom(0)),
            ranges: vec![range(&ab, 0, 2), range(&ac, 2, 3)],
            unevaluated_start: 6,
        };

        groups.reset();

        assert_eq!(groups.ranges.len(), 2);
        for range in &groups.ranges {
            assert_eq!(range.start, 0);
            assert_eq!(range.len, 0);
            assert_eq!(range.trust, 0);
        }
        assert!(Arc::ptr_eq(&groups.ranges[0].group, &ab));
        assert!(Arc::ptr_eq(&groups.ranges[1].group, &ac));
        assert_eq!(groups.unevaluated_start, 0);
        assert_eq!(groups.storage_id, Some(StorageId::Custom(0)));
    }

    /// Asserts that `attach` on a storage without ranges adds an empty range at index 0,
    /// keeps every component unevaluated and records the storage's id.
    #[test]
    fn attach() {
        let ab = group(&[0, 1]);
        let mut groups = Groups::default();

        groups.attach(ab.clone(), StorageId::Custom(0));

        assert_eq!(groups.ranges.len(), 1);
        assert!(Arc::ptr_eq(&groups.ranges[0].group, &ab));
        assert_eq!(groups.ranges[0].start, 0);
        assert_eq!(groups.ranges[0].len, 0);
        assert_eq!(groups.ranges[0].shift, 0);
        assert_eq!(groups.ranges[0].trust, 0);
        assert_eq!(groups.unevaluated_start, 0);
        assert_eq!(groups.storage_id, Some(StorageId::Custom(0)));
    }

    /// Asserts that `find_by_value` matches an equal group that was allocated separately.
    #[test]
    fn find_by_value() {
        let ab = group(&[0, 1]);
        let mut groups = Groups::default();
        groups.attach(ab.clone(), StorageId::Custom(0));

        let other_ab = [StorageId::Custom(0), StorageId::Custom(1)];

        assert!(Arc::ptr_eq(groups.find_by_value(&other_ab).unwrap(), &ab));
    }

    /// Asserts that `find_by_value` doesn't match a different group.
    #[test]
    fn find_by_value_other() {
        let mut groups = Groups::default();
        groups.attach(group(&[0, 1]), StorageId::Custom(0));

        let ac = [StorageId::Custom(0), StorageId::Custom(2)];

        assert!(groups.find_by_value(&ac).is_none());
    }

    #[cfg(debug_assertions)]
    mod invariants {
        use super::*;

        /// Asserts that two ranges are rejected while a storage can only belong to one group.
        #[test]
        #[should_panic(expected = "a storage can only belong to one group for now")]
        fn two_ranges() {
            let ab = group(&[0, 1]);
            let ac = group(&[0, 2]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2), range(&ac, 2, 3)],
                unevaluated_start: 5,
            };

            groups.assert_invariants(5);
        }

        /// Asserts that a first range that doesn't start at index 0 is rejected.
        #[test]
        #[should_panic(expected = "group ranges must tile the start of the storage in order")]
        fn tiling_first_start() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 1, 2)],
                unevaluated_start: 3,
            };

            groups.assert_invariants(3);
        }

        /// Asserts that a gap between two ranges is rejected.
        #[test]
        #[should_panic(expected = "group ranges must tile the start of the storage in order")]
        fn tiling_gap() {
            let ab = group(&[0, 1]);
            let ac = group(&[0, 2]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2), range(&ac, 3, 3)],
                unevaluated_start: 6,
            };

            groups.assert_invariants(6);
        }

        /// Asserts that a range trusted over its whole length is accepted.
        #[test]
        fn trust() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2)],
                unevaluated_start: 2,
            };

            groups.assert_invariants(2);
        }

        /// Asserts that a range trusted past its length is rejected.
        #[test]
        #[should_panic(expected = "a range's trust cannot exceed its length")]
        fn trust_past_len() {
            let ab = group(&[0, 1]);
            let mut ab_range = range(&ab, 0, 2);
            ab_range.trust = 3;
            let groups = Groups {
                storage_id: None,
                ranges: vec![ab_range],
                unevaluated_start: 2,
            };

            groups.assert_invariants(2);
        }

        /// Asserts that a range with a shift of 0 is accepted.
        #[test]
        fn shift() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2)],
                unevaluated_start: 2,
            };

            groups.assert_invariants(2);
        }

        /// Asserts that a rotated range is rejected while ranges can't be rings.
        #[test]
        #[should_panic(expected = "ranges cannot be rotated yet")]
        fn shift_rotated() {
            let ab = group(&[0, 1]);
            let mut ab_range = range(&ab, 0, 2);
            ab_range.shift = -1;
            let groups = Groups {
                storage_id: None,
                ranges: vec![ab_range],
                unevaluated_start: 2,
            };

            groups.assert_invariants(2);
        }

        /// Asserts that an unevaluated region starting right after the last range is accepted.
        #[test]
        fn unevaluated_after_ranges() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2)],
                unevaluated_start: 2,
            };

            groups.assert_invariants(4);
        }

        /// Asserts that an unevaluated region starting inside a range is rejected.
        #[test]
        #[should_panic(expected = "the unevaluated region cannot start inside a group range")]
        fn unevaluated_inside_range() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2)],
                unevaluated_start: 1,
            };

            groups.assert_invariants(4);
        }

        /// Asserts that an unevaluated region starting at the end of the storage is accepted.
        #[test]
        fn unevaluated_at_end() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2)],
                unevaluated_start: 4,
            };

            groups.assert_invariants(4);
        }

        /// Asserts that an unevaluated region starting past the end of the storage is rejected.
        #[test]
        #[should_panic(
            expected = "the unevaluated region cannot start past the end of the storage"
        )]
        fn unevaluated_past_end() {
            let ab = group(&[0, 1]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2)],
                unevaluated_start: 5,
            };

            groups.assert_invariants(4);
        }

        /// Asserts that a storage without ranges and fully unevaluated is accepted.
        #[test]
        fn no_ranges() {
            let groups = Groups::default();

            groups.assert_invariants(3);
        }

        /// Asserts that a storage without ranges but with an ungrouped region is rejected.
        #[test]
        #[should_panic(expected = "a storage without group ranges must be fully unevaluated")]
        fn no_ranges_ungrouped() {
            let groups = Groups {
                storage_id: None,
                ranges: vec![],
                unevaluated_start: 2,
            };

            groups.assert_invariants(3);
        }
    }
}
