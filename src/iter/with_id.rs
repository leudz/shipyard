use crate::entity_id::EntityId;
use crate::iter::{Shiperator, ShiperatorCaptain, ShiperatorSailor};
use core::iter::FusedIterator;

/// Iterator that returns the [`EntityId`] alongside the component(s).
pub struct WithId<S>(pub(crate) S);

impl<S> Shiperator<S> {
    /// Returns the [`EntityId`] alongside the component(s).
    pub fn with_id(self) -> WithId<Shiperator<S>> {
        WithId(self)
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor> Shiperator<S> {
    /// Returns the [`EntityId`] of the matching components.
    #[allow(clippy::type_complexity)]
    pub fn ids(self) -> core::iter::Map<WithId<Shiperator<S>>, fn((EntityId, S::Out)) -> EntityId> {
        WithId(self).map(|(eid, _)| eid)
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor> Iterator for WithId<Shiperator<S>> {
    type Item = (EntityId, S::Out);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next() {
            let entity_id = unsafe { self.0.entities.get(self.0.start - 1) };

            Some((entity_id, item))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

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
                } else {
                    return init;
                }
            };

            if self.0.is_exact_sized {
                while self.0.start < self.0.end {
                    let current = self.0.start;
                    self.0.start += 1;

                    let entity_id = unsafe { self.0.entities.get(current) };
                    let data = unsafe { self.0.shiperator.get_captain_data(current) };

                    init = f(init, (entity_id, data));
                }
            } else {
                while self.0.start < self.0.end {
                    let current = self.0.start;
                    self.0.start += 1;

                    let entity_id = unsafe { self.0.entities.get(current) };

                    if let Some(indices) = self.0.shiperator.indices_of(entity_id, current) {
                        let data = unsafe { self.0.shiperator.get_sailor_data(indices) };

                        init = f(init, (entity_id, data));
                    }
                }
            }
        }
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for WithId<I>
where
    WithId<I>: Iterator,
{
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor> DoubleEndedIterator for WithId<Shiperator<S>> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next_back() {
            let entity_id = unsafe { self.0.entities.get(self.0.end + 1) };

            Some((entity_id, item))
        } else {
            None
        }
    }

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

            if self.0.is_exact_sized {
                while self.0.start < self.0.end {
                    self.0.end -= 1;

                    let entity_id = unsafe { self.0.entities.get(self.0.end) };
                    let data = unsafe { self.0.shiperator.get_captain_data(self.0.end) };

                    init = f(init, (entity_id, data));
                }
            } else {
                while self.0.start < self.0.end {
                    self.0.end -= 1;

                    let entity_id = unsafe { self.0.entities.get(self.0.end) };

                    if let Some(indices) = self.0.shiperator.indices_of(entity_id, self.0.end) {
                        let data = unsafe { self.0.shiperator.get_sailor_data(indices) };

                        init = f(init, (entity_id, data));
                    }
                }
            }
        }
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor> FusedIterator for WithId<Shiperator<S>> {}

#[cfg(feature = "parallel")]
impl<S: ShiperatorCaptain + ShiperatorSailor + Send + Clone>
    rayon::iter::plumbing::UnindexedProducer for WithId<Shiperator<S>>
{
    type Item = (EntityId, S::Out);

    fn split(self) -> (Self, Option<Self>) {
        let (left, right) = self.0.split();

        (WithId(left), right.map(WithId))
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        folder.consume_iter(self)
    }
}
