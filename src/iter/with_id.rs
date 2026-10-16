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
    fn fold<B, F>(self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut shiper = self.0;

        // We start at the first unconsumed candidate, `next` might have consumed a prefix.
        while shiper.has_candidate() {
            let local_index = shiper.cursor.local_index();
            // SAFE: `has_candidate` returned `true` so the cursor isn't empty
            // and `local_index` is the local index of its current candidate.
            // We own the schedule holding the plan of the last segment we entered, if any.
            // We own the schedule holding the plan of the last segment we entered, if any.
            let indices = unsafe {
                shiper
                    .shiperator
                    .captain_indices_of(&shiper.cursor, local_index)
            };

            let Some(indices) = indices else {
                shiper.cursor.advance();
                continue;
            };

            // SAFE: The cursor isn't empty, we haven't advanced yet.
            let entity_id = unsafe { shiper.cursor.current() };
            // We consume the candidate before creating any output or calling `f`.
            shiper.cursor.advance();

            // SAFE: `indices` was just returned by `captain_indices_of`.
            let data = unsafe { shiper.shiperator.get_data(indices) };

            init = f(init, (entity_id, data));
        }

        init
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
        let (left, right) = self.0.split();

        (WithId(left), right.map(WithId))
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        folder.consume_iter(self)
    }
}
