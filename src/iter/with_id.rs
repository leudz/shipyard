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

impl<S: Shiperator> Iterator for WithId<Shiper<S>> {
    type Item = (EntityId, S::Out);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let shiper = &mut self.0;

        while shiper.has_candidate() {
            let local_index = shiper.entities.local_index();
            // SAFE: `has_candidate` returned `true` so the current range isn't exhausted
            // and `local_index` is the local index of its current candidate.
            let indices = unsafe {
                shiper
                    .shiperator
                    .captain_indices_of(&shiper.entities, local_index)
            };

            let Some(indices) = indices else {
                shiper.entities.advance();
                continue;
            };

            // We have to read the id before consuming the candidate.
            // SAFE: The current range isn't exhausted, we haven't advanced yet.
            let entity_id = unsafe { shiper.entities.current() };
            shiper.entities.advance();

            // SAFE: `indices` was just returned by `captain_indices_of`.
            let data = unsafe { shiper.shiperator.get_data(indices) };

            return Some((entity_id, data));
        }

        None
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
            let local_index = shiper.entities.local_index();
            // SAFE: `has_candidate` returned `true` so the current range isn't exhausted
            // and `local_index` is the local index of its current candidate.
            let indices = unsafe {
                shiper
                    .shiperator
                    .captain_indices_of(&shiper.entities, local_index)
            };

            let Some(indices) = indices else {
                shiper.entities.advance();
                continue;
            };

            // SAFE: The current range isn't exhausted, we haven't advanced yet.
            let entity_id = unsafe { shiper.entities.current() };
            // We consume the candidate before creating any output or calling `f`.
            shiper.entities.advance();

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
