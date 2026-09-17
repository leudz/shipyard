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
        if let Some(item) = self.0.next() {
            let entity_id = unsafe { self.0.entities.get(self.0.start - 1) };

            Some((entity_id, item))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn fold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        loop {
            if self.0.start == self.0.end {
                if let Some(new_end) = self.0.entities.next_slice() {
                    self.0.start = 0;
                    self.0.end = new_end;

                    self.0.shiperator.next_slice();
                } else {
                    return init;
                }
            };

            while self.0.start < self.0.end {
                let current = self.0.start;
                self.0.start += 1;

                if let Some(indices) = unsafe {
                    self.0
                        .shiperator
                        .captain_indices_of(&mut self.0.entities, current)
                } {
                    let eid = unsafe { self.0.entities.get(current) };
                    let data = unsafe { self.0.shiperator.get_data(indices) };

                    init = f(init, (eid, data));
                }
            }
        }
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

impl<S: Shiperator> DoubleEndedIterator for WithId<Shiper<S>> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next_back() {
            let entity_id = unsafe { self.0.entities.get(self.0.end) };

            Some((entity_id, item))
        } else {
            None
        }
    }

    #[inline]
    fn rfold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        loop {
            if self.0.start == self.0.end {
                if let Some(new_end) = self.0.entities.next_slice() {
                    self.0.start = 0;
                    self.0.end = new_end;

                    self.0.shiperator.next_slice();
                } else {
                    return init;
                }
            };

            while self.0.start < self.0.end {
                self.0.end -= 1;

                if let Some(indices) = unsafe {
                    self.0
                        .shiperator
                        .captain_indices_of(&mut self.0.entities, self.0.end)
                } {
                    let eid = unsafe { self.0.entities.get(self.0.end) };
                    let data = unsafe { self.0.shiperator.get_data(indices) };

                    init = f(init, (eid, data));
                }
            }
        }
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
