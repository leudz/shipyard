mod into_shiperator;
pub(crate) mod mixed;
#[cfg(feature = "parallel")]
mod parallel;
pub(crate) mod plan;
mod raw_entity_id_access;
mod shiperator;
mod with_id;

#[cfg(test)]
mod tests;

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
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub use parallel::ParShiperator;
#[doc(hidden)]
pub use plan::{SegmentEntry, SegmentRow};
#[doc(hidden)]
pub use raw_entity_id_access::RawEntityIdAccess;
pub use shiperator::Shiperator;
pub use with_id::WithId;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use core::iter::FusedIterator;
use plan::SegmentSchedule;

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
    /// `entities`' initial cursor must match the states `shiperator` has installed
    /// and `shiperator` must be able to enter every follow-up of `entities`.
    #[inline]
    pub(crate) fn new(shiperator: S, entities: RawEntityIdAccess) -> Shiper<S> {
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
    #[inline]
    fn enter_next_segment(&mut self) -> bool {
        let Some(schedule) = &mut self.schedule else {
            return false;
        };
        let Some(segment) = schedule.take_next() else {
            return false;
        };

        let mut entry = SegmentEntry::new(schedule.plan(), &segment);
        // SAFE: Schedules are only built by this Shiperator's constructors (`IntoShiperator`),
        // they pair each row with the entities its own nodes resolve:
        // windows' segment starts are the storage indices of the segment's first entity.
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
        .is_some_and(|schedule| !schedule.follow_ups().is_empty());

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

impl<'tmp, T: Component> ExactSizeIterator for Shiper<FullRawWindow<'tmp, T>> {}

impl<'tmp, T: Component, Track> ExactSizeIterator for Shiper<FullRawWindowMut<'tmp, T, Track>> where
    Self: Iterator
{
}
