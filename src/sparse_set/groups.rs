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
    /// In creation order, tiling the physical prefix from index 0. Empty ranges stay attached.
    pub(crate) ranges: Vec<GroupRange>,
    /// Physical index where the unevaluated region starts.
    pub(crate) unevaluated_start: usize,
}

/// The part of a storage's vectors that holds a group's components.
///
/// A range is a ring, see [`Ring`].
pub(crate) struct GroupRange {
    /// The group's sorted member ids. Shared by every member and compared by pointer.
    pub(crate) group: Arc<[StorageId]>,
    pub(crate) start: usize,
    pub(crate) len: usize,
    /// Rotation of the ring, see [`Ring`]. It's never normalized.
    pub(crate) shift: isize,
    /// Length of the logical prefix that is unchanged since the last regroup.
    pub(crate) trust: usize,
}

/// Position and rotation of a group range, with the arithmetic between its logical and physical orders.
///
/// Members are aligned by logical position, physical positions differ from one member to the next.
/// For a non-empty ring:
///
/// ```text
/// physical index of logical k = start + (shift + k).rem_euclid(len)
/// ```
///
/// Calling `s` the shift's remainder, physical offsets `[s, len)` hold logical `[0, len - s)`
/// and offsets `[0, s)` hold logical `[len - s, len)`.\
/// The boundary between the logical last position and logical 0 is the wrap point.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Ring {
    pub(crate) start: usize,
    pub(crate) len: usize,
    pub(crate) shift: isize,
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

    /// Returns this range's position and rotation.
    pub(crate) fn ring(&self) -> Ring {
        Ring {
            start: self.start,
            len: self.len,
            shift: self.shift,
        }
    }

    /// Moves the whole range one slot to the left, keeping every component's logical position.
    ///
    /// The caller moves the range's physical last component to the slot before `start`.
    pub(crate) fn move_left(&mut self) {
        // The component that was at offset `len - 1` is now at offset 0, every other one moved
        // one offset further. One more shift keeps them all at their logical position.
        self.start -= 1;
        self.shift = self
            .shift
            .checked_add(1)
            .expect("ring shifts fit in an isize");
    }

    /// Moves the whole range one slot to the right, keeping every component's logical position.
    ///
    /// The caller moves the range's physical first component to the slot after its end.
    pub(crate) fn move_right(&mut self) {
        // The component that was at offset 0 is now at offset `len - 1`, every other one moved
        // one offset back. One less shift keeps them all at their logical position.
        self.start += 1;
        self.shift = self
            .shift
            .checked_sub(1)
            .expect("ring shifts fit in an isize");
    }

    /// Changes the range's length, logical 0 then being `wrap_offset` slots after `start`.
    ///
    /// The length is the ring's modulus, so the shift has to be recomputed when it changes.
    /// We move it by less than the new length to reach the new remainder, instead of normalizing it.\
    /// An empty range keeps its shift as is, it has no position to place.
    pub(crate) fn resize(&mut self, len: usize, wrap_offset: usize) {
        self.len = len;
        if len == 0 {
            return;
        }

        assert!(wrap_offset < len, "the wrap point is inside the range");

        let current = self.ring().wrap_offset();
        // Both offsets are smaller than `len`, which fits in an isize like every dense length.
        let to_isize = |offset| isize::try_from(offset).expect("range lengths fit in an isize");
        let delta = to_isize(wrap_offset) - to_isize(current);

        self.shift = self
            .shift
            .checked_add(delta)
            .expect("ring shifts fit in an isize");
    }
}

impl Ring {
    /// Returns the physical index right after this ring.
    pub(crate) fn end(&self) -> usize {
        self.start + self.len
    }

    /// Returns `true` when `physical` is one of this ring's slots.
    pub(crate) fn contains(&self, physical: usize) -> bool {
        self.start <= physical && physical < self.end()
    }

    /// Returns the offset of logical 0 from `start`, `shift.rem_euclid(len)`.
    ///
    /// ### Panics
    ///
    /// - The ring is empty.
    pub(crate) fn wrap_offset(&self) -> usize {
        assert_ne!(self.len, 0, "an empty ring has no wrap point");

        // We take the remainder of the magnitude so no signed value has to be converted.
        let remainder = self.shift.unsigned_abs() % self.len;

        if self.shift < 0 && remainder != 0 {
            self.len - remainder
        } else {
            remainder
        }
    }

    /// Returns the number of logical positions physically after the wrap point, `[0, len - s)`.
    fn unwrapped_len(&self) -> usize {
        self.len - self.wrap_offset()
    }

    /// Returns the physical index of logical position `logical`.
    ///
    /// ### Panics
    ///
    /// - `logical` isn't smaller than the ring's length.
    pub(crate) fn physical_of(&self, logical: usize) -> usize {
        assert!(logical < self.len, "logical positions are inside the ring");

        let unwrapped_len = self.unwrapped_len();

        if logical < unwrapped_len {
            self.start + self.wrap_offset() + logical
        } else {
            self.start + (logical - unwrapped_len)
        }
    }

    /// Returns the logical position of the component at physical index `physical`.
    ///
    /// ### Panics
    ///
    /// - `physical` isn't one of the ring's slots.
    pub(crate) fn logical_of(&self, physical: usize) -> usize {
        assert!(
            self.contains(physical),
            "physical indices are inside the ring"
        );

        let offset = physical - self.start;
        let wrap_offset = self.wrap_offset();

        if offset >= wrap_offset {
            offset - wrap_offset
        } else {
            offset + self.unwrapped_len()
        }
    }

    /// Returns how many logical positions from `logical` on are physically consecutive,
    /// up to the wrap point or the ring's end.
    ///
    /// ### Panics
    ///
    /// - `logical` isn't smaller than the ring's length.
    pub(crate) fn contiguous_len(&self, logical: usize) -> usize {
        assert!(logical < self.len, "logical positions are inside the ring");

        let unwrapped_len = self.unwrapped_len();

        if logical < unwrapped_len {
            unwrapped_len - logical
        } else {
            self.len - logical
        }
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
        if let Some(range_index) = self.range_index_of(index) {
            GroupRegion::Range(&self.ranges[range_index].group)
        } else if index < self.unevaluated_start {
            GroupRegion::Ungrouped
        } else {
            GroupRegion::Unevaluated
        }
    }

    /// Returns the position in `ranges` of the range holding the component at physical `index`.
    ///
    /// Empty ranges hold nothing, so they're never returned.
    pub(crate) fn range_index_of(&self, index: usize) -> Option<usize> {
        self.ranges
            .iter()
            .position(|range| range.ring().contains(index))
    }

    /// Returns the position in `ranges` of `group`'s range.
    ///
    /// Groups are compared by pointer, an equal group allocated separately doesn't match.
    pub(crate) fn range_index(&self, group: &Arc<[StorageId]>) -> Option<usize> {
        self.ranges
            .iter()
            .position(|range| Arc::ptr_eq(&range.group, group))
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
    /// This compares by value. It's only meant to detect duplicate declarations and to reuse
    /// an existing union, everywhere else groups are compared by pointer.
    pub(crate) fn find_by_value(&self, ids: &[StorageId]) -> Option<&Arc<[StorageId]>> {
        self.ranges
            .iter()
            .map(|range| &range.group)
            .find(|group| group[..] == *ids)
    }

    /// Makes the whole storage unevaluated, without moving any component.
    ///
    /// Definitions, creation order and signed shifts survive. Every range becomes empty with
    /// zero trust, and the ungrouped region becomes empty too.
    pub(crate) fn reset(&mut self) {
        // This is a temporary reset rule.
        // Ordinary removal, deletion, retain, generation replacement, sorting, clear and drain
        // land here. Regroup's own movements preserve ranges instead.
        // Iteration stays correct: our trust is 0 so every group's aligned part is empty,
        // and all our components are found in the unevaluated region.
        // Shifts are kept, an empty range doesn't use it and growing it picks a new remainder.
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
        // Rings can have any shift, only their slots have to tile the start of the storage.
        // Empty ranges take no slot, several of them can share a start.
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

            previous_end = range.end();
        }

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

    fn ring(start: usize, len: usize, shift: isize) -> Ring {
        Ring { start, len, shift }
    }

    /// Returns the physical index of every logical position of `ring`, in logical order.
    fn physical_order(ring: Ring) -> Vec<usize> {
        (0..ring.len).map(|k| ring.physical_of(k)).collect()
    }

    /// Asserts that an unrotated ring maps logical `k` to `start + k`.
    #[test]
    fn ring_unrotated() {
        assert_eq!(physical_order(ring(2, 4, 0)), [2, 3, 4, 5]);
    }

    /// Asserts that a shift of 2 puts logical 0 two slots after `start`,
    /// the last two logical positions wrapping to the front: `[L3 L4 L0 L1 L2]`.
    #[test]
    fn ring_positive() {
        assert_eq!(physical_order(ring(3, 5, 2)), [5, 6, 7, 3, 4]);
    }

    /// Asserts that a shift of -1 puts logical 0 on the ring's last slot.
    #[test]
    fn ring_negative() {
        assert_eq!(physical_order(ring(0, 5, -1)), [4, 0, 1, 2, 3]);
    }

    /// Asserts that shifts larger than the length, in either direction, only count by their remainder.
    #[test]
    fn ring_large_shift() {
        assert_eq!(physical_order(ring(3, 5, 12)), [5, 6, 7, 3, 4]);
        assert_eq!(physical_order(ring(3, 5, -13)), [5, 6, 7, 3, 4]);
        assert_eq!(physical_order(ring(3, 5, -10)), [3, 4, 5, 6, 7]);
    }

    /// Asserts that the most extreme shifts give the same remainder as an exact `rem_euclid`.
    #[test]
    fn ring_extreme_shift() {
        for shift in [isize::MIN, isize::MIN + 1, isize::MAX] {
            for len in 1..=7usize {
                let expected = i128::try_from(shift)
                    .unwrap()
                    .rem_euclid(i128::try_from(len).unwrap());

                assert_eq!(
                    i128::try_from(ring(0, len, shift).wrap_offset()).unwrap(),
                    expected,
                    "shift {shift}, len {len}"
                );
            }
        }
    }

    /// Asserts that a ring of length 1 always holds logical 0 at `start`.
    #[test]
    fn ring_len_one() {
        for shift in [-3, -1, 0, 1, 7] {
            assert_eq!(physical_order(ring(4, 1, shift)), [4]);
        }
    }

    /// Asserts that, for every small length and shift, logical positions map to every slot once
    /// and `logical_of` maps each slot back.
    #[test]
    fn ring_round_trip() {
        for len in 1..=6 {
            for shift in -13..=13 {
                let ring = ring(3, len, shift);

                let mut slots = physical_order(ring);
                for (logical, &physical) in slots.iter().enumerate() {
                    assert_eq!(ring.logical_of(physical), logical, "{ring:?}");
                }

                slots.sort_unstable();
                assert_eq!(slots, (3..3 + len).collect::<Vec<_>>(), "{ring:?}");
            }
        }
    }

    /// Asserts that an empty ring has no wrap point instead of taking a remainder by zero.
    #[test]
    #[should_panic(expected = "an empty ring has no wrap point")]
    fn ring_empty() {
        let _ = ring(0, 0, 3).wrap_offset();
    }

    /// Asserts that an empty ring has no logical position.
    #[test]
    #[should_panic(expected = "logical positions are inside the ring")]
    fn ring_empty_position() {
        let _ = ring(0, 0, 0).physical_of(0);
    }

    /// Asserts that `logical_of` rejects slots outside the ring.
    #[test]
    #[should_panic(expected = "physical indices are inside the ring")]
    fn ring_outside() {
        let _ = ring(2, 3, 1).logical_of(5);
    }

    /// Asserts that consecutive logical positions are physically consecutive up to the wrap point
    /// and up to the ring's end, `[L3 L4 L0 L1 L2]` splitting into `L0..L3` and `L3..L5`.
    #[test]
    fn ring_contiguous_len() {
        let rotated = ring(3, 5, 2);

        assert_eq!(rotated.contiguous_len(0), 3);
        assert_eq!(rotated.contiguous_len(2), 1);
        assert_eq!(rotated.contiguous_len(3), 2);
        assert_eq!(rotated.contiguous_len(4), 1);
        assert_eq!(ring(3, 5, 0).contiguous_len(1), 4);
    }

    /// Returns a range over `ring` without a meaningful group or trust.
    fn ring_range(ring: Ring) -> GroupRange {
        GroupRange {
            group: group(&[0, 1]),
            start: ring.start,
            len: ring.len,
            shift: ring.shift,
            trust: 0,
        }
    }

    /// Asserts that moving a range left keeps every logical position,
    /// only the physical last component lands on the slot before the old start.
    #[test]
    fn move_left() {
        for len in 1..=4 {
            for shift in -5..=5 {
                let before = ring(3, len, shift);
                let mut range = ring_range(before);

                range.move_left();

                let expected: Vec<usize> = physical_order(before)
                    .into_iter()
                    .map(|physical| {
                        if physical == before.end() - 1 {
                            before.start - 1
                        } else {
                            physical
                        }
                    })
                    .collect();
                assert_eq!(physical_order(range.ring()), expected, "{before:?}");
            }
        }
    }

    /// Asserts that moving a range right keeps every logical position,
    /// only the physical first component lands on the slot after the old end.
    #[test]
    fn move_right() {
        for len in 1..=4 {
            for shift in -5..=5 {
                let before = ring(3, len, shift);
                let mut range = ring_range(before);

                range.move_right();

                let expected: Vec<usize> = physical_order(before)
                    .into_iter()
                    .map(|physical| {
                        if physical == before.start {
                            before.end()
                        } else {
                            physical
                        }
                    })
                    .collect();
                assert_eq!(physical_order(range.ring()), expected, "{before:?}");
            }
        }
    }

    /// Asserts that moving an empty range only moves its start.
    #[test]
    fn move_empty() {
        let mut range = ring_range(ring(3, 0, -2));

        range.move_left();
        assert_eq!(range.start, 2);
        range.move_right();
        range.move_right();
        assert_eq!(range.start, 4);
    }

    /// Asserts that resizing picks the requested wrap point by moving the shift by the difference
    /// between the new and current remainders, instead of normalizing it.
    #[test]
    fn resize() {
        let mut range = ring_range(ring(0, 5, 17));

        range.resize(4, 1);

        assert_eq!(range.ring().wrap_offset(), 1);
        // 17 has a remainder of 1 modulo 4, it's already right.
        assert_eq!(range.shift, 17);

        range.resize(6, 4);
        // 17 has a remainder of 5 modulo 6, one less gives 4.
        assert_eq!((range.ring().wrap_offset(), range.shift), (4, 16));

        range.resize(3, 2);
        // 16 has a remainder of 1 modulo 3, one more gives 2.
        assert_eq!((range.ring().wrap_offset(), range.shift), (2, 17));
    }

    /// Asserts that resizing to 0 keeps the shift, an empty range has no wrap point.
    #[test]
    fn resize_empty() {
        let mut range = ring_range(ring(0, 2, -9));

        range.resize(0, 0);

        assert_eq!((range.len, range.shift), (0, -9));
    }

    /// Asserts that an empty range with a large retained shift grows like any other.
    #[test]
    fn resize_from_empty() {
        let mut range = ring_range(ring(2, 0, isize::MIN + 1));

        range.resize(1, 0);
        assert_eq!(physical_order(range.ring()), [2]);
        range.resize(2, 1);
        assert_eq!(physical_order(range.ring()), [3, 2]);
    }

    /// Asserts that a reset storage can grow its ranges again whatever shift they kept.
    #[test]
    fn reset_reuse() {
        let ab = group(&[0, 1]);
        let mut ab_range = range(&ab, 0, 3);
        ab_range.shift = -7;
        let mut groups = Groups {
            storage_id: None,
            ranges: vec![ab_range],
            unevaluated_start: 3,
        };

        groups.reset();
        groups.ranges[0].resize(1, 0);
        groups.unevaluated_start = 1;

        assert_eq!(groups.ranges[0].shift, -7);
        assert_eq!(physical_order(groups.ranges[0].ring()), [0]);
        #[cfg(debug_assertions)]
        groups.assert_invariants(1);
    }

    /// Asserts that empty ranges between non-empty ones hold nothing,
    /// every slot is classified in the range actually covering it.
    #[test]
    fn region_of_empty_interior() {
        let ab = group(&[0, 1]);
        let ac = group(&[0, 2]);
        let ad = group(&[0, 3]);
        let groups = Groups {
            storage_id: None,
            ranges: vec![range(&ab, 0, 2), range(&ac, 2, 0), range(&ad, 2, 3)],
            unevaluated_start: 6,
        };

        assert_eq!(groups.range_index_of(1), Some(0));
        assert_eq!(groups.range_index_of(2), Some(2));
        assert_eq!(groups.range_index_of(4), Some(2));
        assert_eq!(groups.range_index_of(5), None);
        assert!(
            matches!(groups.region_of(2), GroupRegion::Range(group) if Arc::ptr_eq(group, &ad))
        );
        assert!(matches!(groups.region_of(5), GroupRegion::Ungrouped));
        assert_eq!(groups.range_index(&ac), Some(1));
        assert_eq!(groups.range_index(&group(&[0, 2])), None);
    }

    /// Asserts that rotations don't change which region holds a slot.
    #[test]
    fn region_of_rotated() {
        let ab = group(&[0, 1]);
        let ac = group(&[0, 2]);
        let mut ab_range = range(&ab, 0, 2);
        ab_range.shift = 1;
        let mut ac_range = range(&ac, 2, 3);
        ac_range.shift = -4;
        let groups = Groups {
            storage_id: None,
            ranges: vec![ab_range, ac_range],
            unevaluated_start: 5,
        };

        assert_eq!(groups.range_index_of(0), Some(0));
        assert_eq!(groups.range_index_of(1), Some(0));
        assert_eq!(groups.range_index_of(4), Some(1));
        assert!(matches!(groups.region_of(5), GroupRegion::Unevaluated));
    }

    #[cfg(debug_assertions)]
    mod invariants {
        use super::*;

        /// Asserts that several ranges, rotated or empty, are accepted.
        #[test]
        fn several_ranges() {
            let ab = group(&[0, 1]);
            let ac = group(&[0, 2]);
            let ad = group(&[0, 3]);
            let mut ab_range = range(&ab, 0, 2);
            ab_range.shift = 5;
            let mut ad_range = range(&ad, 2, 3);
            ad_range.shift = -2;
            let groups = Groups {
                storage_id: None,
                ranges: vec![ab_range, range(&ac, 2, 0), ad_range],
                unevaluated_start: 5,
            };

            groups.assert_invariants(5);
        }

        /// Asserts that a range overlapping the previous one is rejected.
        #[test]
        #[should_panic(expected = "group ranges must tile the start of the storage in order")]
        fn tiling_overlap() {
            let ab = group(&[0, 1]);
            let ac = group(&[0, 2]);
            let groups = Groups {
                storage_id: None,
                ranges: vec![range(&ab, 0, 2), range(&ac, 1, 3)],
                unevaluated_start: 4,
            };

            groups.assert_invariants(4);
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

        /// Asserts that a rotated range is accepted, whatever its shift.
        #[test]
        fn shift_rotated() {
            let ab = group(&[0, 1]);
            let mut ab_range = range(&ab, 0, 2);
            ab_range.shift = isize::MIN;
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
