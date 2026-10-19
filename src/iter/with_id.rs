use crate::entity_id::EntityId;
use crate::iter::{Shiper, Shiperator};
use core::iter::FusedIterator;

/// Iterator that returns the [`EntityId`] alongside the component(s).
pub struct WithId<S>(pub(crate) S);

impl<S> Shiper<S> {
    /// Returns the [`EntityId`] alongside the component(s).
    pub fn with_id(self) -> WithId<Shiper<S>> {
        WithId(self)
    }
}

impl<S: Shiperator> Shiper<S> {
    /// Returns the [`EntityId`] of the matching components.
    #[allow(clippy::type_complexity)]
    pub fn ids(self) -> core::iter::Map<WithId<Shiper<S>>, fn((EntityId, S::Out)) -> EntityId> {
        WithId(self).map(|(eid, _)| eid)
    }
}

impl<S: Shiperator> Shiper<S> {
    /// Returns the next item of the current segment and its id, without entering any other segment.
    #[inline(always)]
    fn next_with_id_in_segment(&mut self) -> Option<(EntityId, S::Out)> {
        while !self.cursor.is_empty() {
            let local_index = self.cursor.local_index();
            // SAFE: The cursor isn't empty and `local_index` is the local index of its current candidate.
            // We own the schedule holding the plan of the last segment we entered, if any.
            let indices = unsafe {
                self.shiperator
                    .captain_indices_of(&self.cursor, local_index)
            };

            let Some(indices) = indices else {
                self.cursor.advance();
                continue;
            };

            // We have to read the id before consuming the candidate.
            // SAFE: The cursor isn't empty, we haven't advanced yet.
            let entity_id = unsafe { self.cursor.current() };
            self.cursor.advance();

            // SAFE: `indices` was just returned by `captain_indices_of`.
            let data = unsafe { self.shiperator.get_data(indices) };

            return Some((entity_id, data));
        }

        None
    }

    /// Returns the next item and its id, entering follow-up segments once the current one is exhausted.
    #[inline]
    fn next_with_id_planned(&mut self) -> Option<(EntityId, S::Out)> {
        loop {
            if let Some(item) = self.next_with_id_in_segment() {
                return Some(item);
            }

            if !self.enter_next_segment() {
                return None;
            }
        }
    }
}

impl<S: Shiperator> Iterator for WithId<Shiper<S>> {
    type Item = (EntityId, S::Out);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let shiper = &mut self.0;

        // Same as `Shiper::next`, without a plan there is only the current segment to walk.
        if shiper.schedule.is_none() {
            return shiper.next_with_id_in_segment();
        }

        shiper.next_with_id_planned()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        // The adapters own their callback with `move`, see `Shiper::fold_segments`.
        self.0
            .fold_segments::<true, B, _>(init, move |acc, entity_id, item| {
                f(acc, (with_id(entity_id), item))
            })
    }

    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        self.0
            .fold_segments::<true, (), _>((), move |(), entity_id, item| {
                f((with_id(entity_id), item))
            })
    }
}

/// Unwraps the id `fold_segment` captures when asked to.
#[inline(always)]
pub(super) fn with_id(entity_id: Option<EntityId>) -> EntityId {
    match entity_id {
        Some(entity_id) => entity_id,
        None => unreachable!("`fold_segment` captures ids when `WITH_ID` is true"),
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for WithId<I>
where
    WithId<I>: Iterator,
{
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<S: Shiperator> FusedIterator for WithId<Shiper<S>> {}

#[cfg(feature = "parallel")]
impl<S: Shiperator + Send + Clone> rayon::iter::plumbing::UnindexedProducer for WithId<Shiper<S>> {
    type Item = (EntityId, S::Out);

    #[inline]
    fn split(self) -> (Self, Option<Self>) {
        // Like `Shiper`'s producer, we balance the whole traversal.
        let (left, right) = self.0.split_remaining();

        (WithId(left), right.map(WithId))
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        // Like `Shiper`'s producer, we fold in bulk.
        self.0
            .fold_chunks::<true, _, _, _>(folder, |entity_id, out| (with_id(entity_id), out))
    }
}
