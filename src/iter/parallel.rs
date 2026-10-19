use crate::entity_id::EntityId;
use crate::iter::{Shiper, Shiperator, WithId};
use rayon::iter::plumbing::{
    Folder, Producer, UnindexedConsumer, UnindexedProducer, bridge_producer_consumer,
    bridge_unindexed,
};

#[allow(missing_docs)]
pub struct ParShiperator<S>(pub(crate) Shiper<S>);

impl<S> ParShiperator<S> {
    /// Returns the [`EntityId`] alongside the component(s).
    pub fn with_id(self) -> WithId<Self> {
        WithId(self)
    }
}

impl<S: Shiperator + Send + Clone> UnindexedProducer for Shiper<S> {
    type Item = S::Out;

    #[inline]
    fn split(self) -> (Self, Option<Self>) {
        // We balance the whole traversal, follow-ups included, so a short current segment
        // doesn't keep a long follow-up on a single thread.
        self.split_remaining()
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // `consume_iter` lets the folder stop once it's `full`, `find_any` for example.
        // Folding everything with our own `fold` would ignore that cancellation.
        folder.consume_iter(self)
    }
}

/// Producer of a traversal whose items are all in the current segment and that can't reject,
/// see [`Shiper::exact_len`].
///
/// `opt_len` reports a length only for these traversals and Rayon then requires us to drive
/// consumers with their indexed methods. We do that with `bridge_producer_consumer`,
/// which splits at the index it picks instead of letting us halve the segment.
pub(super) struct ExactProducer<P>(pub(super) P);

impl<S: Shiperator + Send + Clone> Producer for ExactProducer<Shiper<S>> {
    type Item = S::Out;
    type IntoIter = Self;

    #[inline]
    fn into_iter(self) -> Self {
        self
    }

    #[inline]
    fn split_at(self, index: usize) -> (Self, Self) {
        // Every candidate yields an item, the item at `index` is the candidate at `index`.
        let (left, right) = self.0.split_current_segment_at(index);

        (ExactProducer(left), ExactProducer(right))
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // Same as the unindexed producer, the folder can stop once it's `full`.
        folder.consume_iter(self.0)
    }
}

impl<S: Shiperator + Send + Clone> Producer for ExactProducer<WithId<Shiper<S>>> {
    type Item = (EntityId, S::Out);
    type IntoIter = Self;

    #[inline]
    fn into_iter(self) -> Self {
        self
    }

    #[inline]
    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = (self.0).0.split_current_segment_at(index);

        (ExactProducer(WithId(left)), ExactProducer(WithId(right)))
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        folder.consume_iter(self.0)
    }
}

// `Producer` needs its items as a double ended, exact size iterator.
// We only hand producers to `bridge_producer_consumer`, which never asks for that iterator:
// it only splits producers and folds them with our `fold_with`.
impl<P: Iterator> Iterator for ExactProducer<P> {
    type Item = P::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<P: Iterator> DoubleEndedIterator for ExactProducer<P> {
    fn next_back(&mut self) -> Option<Self::Item> {
        unreachable!("parallel traversals are never iterated backwards")
    }
}

// `ExactProducer` only wraps exact traversals, their size hint is exact.
impl<P: Iterator> ExactSizeIterator for ExactProducer<P> {}

impl<S: Shiperator + Send + Clone> rayon::iter::ParallelIterator for WithId<ParShiperator<S>>
where
    S::Out: Send,
{
    type Item = (EntityId, S::Out);

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let WithId(ParShiperator(producer)) = self;

        // This has to match `opt_len`, see `ParShiperator::drive_unindexed`.
        match producer.exact_len() {
            Some(len) => bridge_producer_consumer(len, ExactProducer(WithId(producer)), consumer),
            None => bridge_unindexed(WithId(producer), consumer),
        }
    }

    #[inline]
    fn opt_len(&self) -> Option<usize> {
        (self.0).0.exact_len()
    }
}

impl<S: Shiperator + Send + Clone> rayon::iter::ParallelIterator for ParShiperator<S>
where
    S::Out: Send,
{
    type Item = S::Out;

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        // Once `opt_len` reports a length, consumers like `collect`'s may only be split with their
        // indexed methods. Both use `exact_len` on the same state so they always agree.
        match self.0.exact_len() {
            Some(len) => bridge_producer_consumer(len, ExactProducer(self.0), consumer),
            None => bridge_unindexed(self.0, consumer),
        }
    }

    #[inline]
    fn opt_len(&self) -> Option<usize> {
        self.0.exact_len()
    }
}
