mod into_shiperator;
pub(crate) mod mixed;
#[cfg(feature = "parallel")]
mod parallel;
pub(crate) mod plan;
pub(crate) mod planner;
mod raw_entity_id_access;
mod shiperator;
mod with_id;

#[cfg(test)]
mod tests;
// Rustdoc doesn't set `test` when it collects doc tests, these checks need their own gate.
#[cfg(doctest)]
#[path = "tests/compile_fail.rs"]
mod compile_fail_tests;

pub use crate::iter_component::{IntoIterRef, IterComponent};
pub use crate::not::Not;
pub use crate::optional::Optional;
pub use crate::or::{OneOfTwo, Or};
#[doc(hidden)]
pub use crate::sparse_set::EntityCursor;
pub use crate::tracking::{Inserted, InsertedOrModified, Modified};
pub use into_shiperator::{IntoIter, IntoShiperator};
pub use mixed::Mixed;
#[cfg(feature = "parallel")]
#[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
pub use parallel::ParShiperator;
#[doc(hidden)]
pub use plan::{SegmentEntry, SegmentRow};
#[doc(hidden)]
pub use planner::ConstructionMetadata;
#[doc(hidden)]
pub use raw_entity_id_access::RawEntityIdAccess;
pub use shiperator::Shiperator;
pub use with_id::WithId;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::Tracking;
use core::iter::FusedIterator;
use plan::{FollowUpCut, SegmentSchedule};

/// Handles storages iteration.
pub struct Shiper<S> {
    pub(crate) shiperator: S,
    /// Candidates left in the current segment.
    pub(crate) cursor: EntityCursor,
    /// Follow-up segments, `None` when the traversal is a single segment.
    pub(crate) schedule: Option<SegmentSchedule>,
    pub(crate) is_exact_sized: bool,
}

impl<S: Shiperator> Shiper<S> {
    /// Returns an iterator starting with `entities`' initial cursor then entering its follow-ups.
    ///
    /// ### Safety
    ///
    /// - `entities`' initial cursor must hold, at each local index, the entity whose components
    ///   `shiperator`'s installed state resolves that local index to
    /// - every follow-up of `entities` must have a row built for `shiperator`'s layout,
    ///   pairing its segment with the entities its captains resolve, see
    ///   [`Shiperator::enter_segment`]
    /// - `shiperator` and `entities` must come from the same storage borrows and uphold the rest
    ///   of [`IntoShiperator`]'s contract, from an implementation of it or from the planner
    ///   rebuilding one of its results
    ///
    /// We then rely on that contract for the whole traversal: entering follow-ups, resolving
    /// candidates and splitting.
    #[inline]
    pub(crate) unsafe fn new(shiperator: S, entities: RawEntityIdAccess) -> Shiper<S> {
        let is_exact_sized = is_exact_traversal(&shiperator, &entities);

        Shiper {
            shiperator,
            cursor: entities.cursor,
            schedule: entities.schedule,
            is_exact_sized,
        }
    }

    /// Enters the next follow-up segment.
    ///
    /// Installs the state of all the nodes driving the segment, then exposes its candidates.\
    /// Segments are never empty so a successful entry always has a candidate.
    ///
    /// Returns `false` when there is no follow-up left.
    #[inline(always)]
    fn enter_next_segment(&mut self) -> bool {
        let Some(schedule) = &mut self.schedule else {
            return false;
        };
        let Some(segment) = schedule.take_next() else {
            return false;
        };

        let mut entry = SegmentEntry::new(schedule.plan(), &segment);
        // SAFE: `new`'s caller guarantees our schedule's rows were built for `shiperator`'s
        // layout, pairing each row with the entities its own nodes resolve: windows' segment
        // starts are the storage indices of the segment's first entity.
        // We own the schedule sharing the plan, it stays alive as long as we use `shiperator`.
        // A split half gets its own share along with its clone of `shiperator`.
        unsafe { self.shiperator.enter_segment(&mut entry) };
        debug_assert!(entry.is_done());

        self.cursor = segment.cursor();
        debug_assert!(!self.cursor.is_empty());

        true
    }

    /// Returns the number of candidates left, follow-ups included.
    ///
    /// Returns `None` when the count overflows.
    #[inline]
    fn remaining_candidates(&self) -> Option<usize> {
        let follow_ups_len = self
            .schedule
            .as_ref()
            .map_or(0, SegmentSchedule::remaining_len);

        self.cursor.len().checked_add(follow_ups_len)
    }

    /// Returns the next item of the current segment, without entering any other segment.
    #[inline(always)]
    fn next_in_segment(&mut self) -> Option<S::Out> {
        while !self.cursor.is_empty() {
            let local_index = self.cursor.local_index();
            // SAFE: The cursor isn't empty and `local_index` is the local index of its current candidate.
            // We own the schedule holding the plan of the last segment we entered, if any.
            let indices = unsafe {
                self.shiperator
                    .captain_indices_of(&self.cursor, local_index)
            };

            // The candidate is consumed whether it matched or not.
            self.cursor.advance();

            if let Some(indices) = indices {
                // SAFE: `indices` was just returned by `captain_indices_of`.
                return Some(unsafe { self.shiperator.get_data(indices) });
            }
        }

        None
    }

    /// Returns the next item, entering follow-up segments once the current one is exhausted.
    #[inline]
    fn next_planned(&mut self) -> Option<S::Out> {
        loop {
            if let Some(item) = self.next_in_segment() {
                return Some(item);
            }

            if !self.enter_next_segment() {
                return None;
            }
        }
    }
}

impl<S: Clone> Shiper<S> {
    /// Splits the candidates left in the current segment in two halves.
    ///
    /// The first half keeps `floor(n / 2)` candidates and no follow-up, the second half gets
    /// the rest of the segment followed by every follow-up left.\
    /// Returns `self` unchanged with `None` when fewer than 2 candidates are left in the current
    /// segment, even if follow-ups are left.
    ///
    /// This is the local primitive, [`split_remaining`](Shiper::split_remaining) balances the
    /// whole traversal.
    #[inline]
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    pub(crate) fn split_current_segment(self) -> (Self, Option<Self>) {
        // We never enter a segment here, that's `split_remaining`'s job.
        let len = self.cursor.len();
        if len < 2 {
            return (self, None);
        }

        let (left, right) = self.split_current_segment_at(len / 2);

        (left, Some(right))
    }

    /// Splits the candidates left in the current segment after the first `index` of them.
    ///
    /// The first half keeps no follow-up, the second half gets every follow-up left.\
    /// Either half can be left without candidates in the current segment.
    ///
    /// ### Panics
    ///
    /// Panics when `index` is greater than the number of candidates left in the current segment.
    #[inline]
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    pub(crate) fn split_current_segment_at(mut self, index: usize) -> (Self, Self) {
        let (left_cursor, right_cursor) = self.cursor.split_at(index);

        // Both halves keep the original segment's local indices and our windows keep their
        // segment starts, so the right half still resolves `segment_start + local_index`.
        // Only the cursors and the follow-ups change hands.
        //
        // The left half keeps a share of the plan even without follow-ups:
        // our current state, coverage included, might point into it.
        let right_schedule = self
            .schedule
            .as_mut()
            .map(SegmentSchedule::split_follow_ups);
        self.cursor = left_cursor;

        let right = Shiper {
            shiperator: self.shiperator.clone(),
            cursor: right_cursor,
            schedule: right_schedule,
            is_exact_sized: self.is_exact_sized,
        };

        (self, right)
    }
}

impl<S: Shiperator + Clone> Shiper<S> {
    /// Splits the candidates left, follow-ups included, near their middle.
    ///
    /// With `n` candidates left, the left half keeps the first `floor(n / 2)` and the right half
    /// gets the rest. When the cut falls in a follow-up, the right half starts there and the left
    /// half keeps the part of that segment before the cut.\
    /// Returns `self` unchanged with `None` when fewer than 2 candidates are left.
    ///
    /// Splitting only reads segment descriptors and installs state. It doesn't read any entity,
    /// create any item or call any consumer.
    #[inline]
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    pub(crate) fn split_remaining(mut self) -> (Self, Option<Self>) {
        let current_len = self.cursor.len();
        let Some(total) = self.remaining_candidates() else {
            // We can't count every candidate, but the current segment alone still splits correctly.
            return self.split_current_segment();
        };
        if total < 2 {
            return (self, None);
        }

        let middle = total / 2;
        if middle < current_len {
            let (left, right) = self.split_current_segment_at(middle);

            return (left, Some(right));
        }

        // `middle < total`, so the follow-ups keep at least one candidate after the cut.
        let right = self.split_follow_ups_at(middle - current_len);

        (self, Some(right))
    }

    /// Splits our follow-ups after their first `count` candidates and returns the half walking
    /// the rest.
    ///
    /// We keep our current segment and the follow-ups before the cut. The returned half is a clone
    /// of our Shiperator that entered the segment holding the cut, it walks that segment from the
    /// cut on, then the follow-ups after it.
    ///
    /// ### Panics
    ///
    /// - We don't have more than `count` candidates in our follow-ups.
    fn split_follow_ups_at(&mut self, count: usize) -> Self {
        let schedule = self
            .schedule
            .as_mut()
            .expect("follow-ups belong to a schedule");
        let FollowUpCut {
            segment,
            local_index,
            rest,
        } = schedule.split_follow_ups_at(count);

        // The right half gets its own state for the segment, like `enter_next_segment` would
        // install it. Our state stays on our current segment.
        let mut shiperator = self.shiperator.clone();
        let mut entry = SegmentEntry::new(rest.plan(), &segment);
        // SAFE: `new`'s caller guarantees every row of our schedule was built for `shiperator`'s
        // layout, pairing its segment with the entities its nodes resolve. `segment` is one of
        // these segments, or a prefix of one, with its original row.
        // The right half only walks `segment`'s candidates with their original local indices.
        // `rest` shares the plan and stays with this Shiperator, as its schedule.
        unsafe { shiperator.enter_segment(&mut entry) };
        debug_assert!(entry.is_done());

        // Starting at the cut keeps the segment's local indices, the state we just installed
        // resolves them from the segment's own start. They must never restart at 0.
        let (_, cursor) = segment.cursor().split_at(local_index);

        Shiper {
            shiperator,
            cursor,
            schedule: Some(rest),
            // Splits only remove candidates, exactness can only stay true.
            is_exact_sized: self.is_exact_sized,
        }
    }
}

impl<S> Shiper<S> {
    /// Returns the number of items left when they're all in the current segment
    /// and every candidate yields one.
    ///
    /// Parallel iterators drive these traversals as indexed: the item at index `i` is
    /// the candidate at index `i`, Rayon can split them anywhere without us entering a segment.
    #[inline]
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    pub(crate) fn exact_len(&self) -> Option<usize> {
        let has_follow_ups = self
            .schedule
            .as_ref()
            .is_some_and(SegmentSchedule::has_follow_ups);

        if !self.is_exact_sized || has_follow_ups {
            return None;
        }

        Some(self.cursor.len())
    }
}

/// Returns `true` when every candidate of `entities`, follow-ups included, yields an item.
///
/// `shiperator` only knows if its current state can reject. Follow-ups install new states and
/// a composite node might reject in some of them.\
/// A single node's exactness doesn't depend on its state, a window stays exact in every segment.
pub(crate) fn is_exact_traversal<S: Shiperator>(
    shiperator: &S,
    entities: &RawEntityIdAccess,
) -> bool {
    let has_follow_ups = entities
        .schedule
        .as_ref()
        .is_some_and(SegmentSchedule::has_follow_ups);

    shiperator.is_exact_sized() && (S::node_count() == 1 || !has_follow_ups)
}

impl<S: Shiperator> Iterator for Shiper<S> {
    type Item = S::Out;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // Having a plan never changes during the iteration. Without one, there is only
        // the current segment to walk.
        if self.schedule.is_none() {
            return self.next_in_segment();
        }

        self.next_planned()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.remaining_candidates() {
            Some(max_len) if self.is_exact_sized => (max_len, Some(max_len)),
            Some(max_len) => (0, Some(max_len)),
            None if self.is_exact_sized => (usize::MAX, None),
            None => (0, None),
        }
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        // The adapters own their callback with `move`, see `fold_segments`.
        self.fold_segments::<false, B, _>(init, move |acc, _, item| f(acc, item))
    }

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        self.fold_segments::<false, (), _>((), move |(), _, item| f(item))
    }
}

impl<S: Shiperator> Shiper<S> {
    /// Folds every item left, segment after segment.
    ///
    /// `f` receives the item's [`EntityId`] when `WITH_ID` is `true`, `None` otherwise.
    #[inline]
    pub(crate) fn fold_segments<const WITH_ID: bool, B, F>(mut self, init: B, f: F) -> B
    where
        F: FnMut(B, Option<EntityId>, S::Out) -> B,
    {
        // `fold_segment` owns its callback for a single segment, we need `f` back for the
        // next one. Lending it as `&mut f` would add a reference layer between the loop and the
        // state `f` captures, in the exploration that layer kept aligned loops from vectorizing.
        // So `f` travels by value inside the accumulator and each segment hands it back.
        let mut acc_and_f = (init, f);

        loop {
            // We start at the first unconsumed candidate, `next` might have consumed a prefix.
            // SAFE: Our cursor only ever holds candidates of the segment `shiperator` last
            // entered, with that segment's local indices, even after `next` or a split.
            // We own the schedule holding the plan of that segment, if any.
            acc_and_f = unsafe {
                self.shiperator.fold_segment::<WITH_ID, _, _>(
                    &mut self.cursor,
                    acc_and_f,
                    |(acc, mut f), entity_id, item| (f(acc, entity_id, item), f),
                )
            };

            if !self.enter_next_segment() {
                return acc_and_f.0;
            }
        }
    }
}

impl<S: Shiperator> FusedIterator for Shiper<S> {}

impl<'tmp, T: Component, Track: Tracking> ExactSizeIterator
    for Shiper<FullRawWindow<'tmp, T, Track>>
{
}

impl<'tmp, T: Component, Track: Tracking> ExactSizeIterator
    for Shiper<FullRawWindowMut<'tmp, T, Track>>
where
    Self: Iterator,
{
}
