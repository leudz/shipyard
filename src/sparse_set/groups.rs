#[cfg(test)]
mod tests;

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
/// A storage without any range is implicitly ungrouped: every component is in the ungrouped region.
/// Its stored `unevaluated_start` is unused and stays at 0, so ordinary insertions and removals
/// don't have to maintain it. The region helpers derive the effective regions from the storage's length.
#[doc(hidden)]
#[derive(Default)]
pub struct Groups {
    /// This storage's own id.
    pub(crate) storage_id: Option<StorageId>,
    /// In creation order, tiling the physical prefix from index 0. Empty ranges stay attached.
    pub(crate) ranges: Vec<GroupRange>,
    /// Physical index where the unevaluated region starts.
    ///
    /// Unused and kept at 0 while there's no range, see [`Groups`].
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
    /// Rotation of the ring, see [`Ring`]. It's only normalized by [`Groups::reset`].
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
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
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

    /// Returns the physical indices of the ungrouped region of a storage of `dense_len` components.
    ///
    /// A storage without any range is entirely ungrouped.
    pub(crate) fn ungrouped_range(&self, dense_len: usize) -> Range<usize> {
        if !self.has_ranges() {
            return 0..dense_len;
        }

        self.grouped_end()..self.unevaluated_start
    }

    /// Returns the physical indices of the unevaluated region of a storage of `dense_len` components.
    ///
    /// A storage without any range has no unevaluated component.
    #[cfg(test)]
    pub(crate) fn unevaluated_range(&self, dense_len: usize) -> Range<usize> {
        if !self.has_ranges() {
            return dense_len..dense_len;
        }

        self.unevaluated_start..dense_len
    }

    /// Returns the region holding the component at physical `index`.
    ///
    /// `index` must be the index of one of the storage's components.
    /// Every component of a storage without any range is ungrouped.
    pub(crate) fn region_of(&self, index: usize) -> GroupRegion<'_> {
        if !self.has_ranges() {
            GroupRegion::Ungrouped
        } else if let Some(range_index) = self.range_index_of(index) {
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
    /// No component moves and the regions keep their bounds.\
    /// On a storage's first range, the unused boundary left at 0 turns every implicitly ungrouped
    /// component into an unevaluated one, for the next regroup to evaluate.
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
    /// Definitions and creation order survive. Every range becomes empty and unrotated with
    /// zero trust, and the ungrouped region becomes empty too.
    pub(crate) fn reset(&mut self) {
        // Clear and drain land here, they empty the whole storage.
        // Removals and generation replacements damage their range locally instead,
        // sorting only reorders the ungrouped region and regroup's own movements preserve ranges.
        // Iteration stays correct: our trust is 0 so every group's aligned part is empty,
        // and all our components are found in the unevaluated region.
        // An empty range doesn't use its shift, we put it back to 0. Shifts are never normalized
        // otherwise, so without this, each clear and refill would keep adding to them.
        for range in &mut self.ranges {
            range.start = 0;
            range.len = 0;
            range.shift = 0;
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
                "a storage without group ranges keeps its unused unevaluated boundary at 0"
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
