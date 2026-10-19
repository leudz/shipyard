use crate::entity_id::EntityId;
use crate::iter::with_id::with_id;
use crate::iter::{Shiper, Shiperator, WithId};
use rayon::iter::plumbing::{
    Folder, Producer, UnindexedConsumer, UnindexedProducer, bridge_producer_consumer,
    bridge_unindexed,
};

/// Candidates a producer folds in bulk between two checks of its folder's `full`.
///
/// A folder that gets full mid-chunk still receives the rest of that chunk.
/// Short-circuiting folders like `find_any`'s ignore these items, adapters in front of them
/// like `inspect` still see them.
pub(super) const FOLD_CHUNK: usize = 128;

/// Pieces per thread a parallel traversal splits into at most, see [`min_piece_len`].
const PIECES_PER_THREAD: usize = 4;

/// Candidates a piece of a parallel traversal keeps at least, see [`min_piece_len`].
const MIN_PIECE_LEN: usize = 32;

/// Returns the number of candidates each piece of a parallel traversal keeps at least,
/// for a traversal of `candidates` candidates.
///
/// Rayon splits again every time a piece is stolen. Down to single candidates, light work spends
/// more time splitting than working and gets slower the more threads there are.
/// We stop at a few pieces per thread, which is still enough to balance the work, and never go
/// below a small floor so heavy work on a few entities still spreads over every thread.
pub(super) fn min_piece_len(candidates: Option<usize>) -> usize {
    let Some(candidates) = candidates else {
        return MIN_PIECE_LEN;
    };

    let pieces = rayon::current_num_threads().saturating_mul(PIECES_PER_THREAD);

    (candidates / pieces).max(MIN_PIECE_LEN)
}

#[allow(missing_docs)]
pub struct ParShiperator<S>(pub(crate) Shiper<S>);

/// A piece of an unindexed parallel traversal, it stops splitting once it's small enough.
pub(super) struct Piece<P> {
    pub(super) producer: P,
    /// Each half of a split keeps at least this many candidates, see [`min_piece_len`].
    pub(super) min_len: usize,
}

impl<P: UnindexedProducer + Iterator> UnindexedProducer for Piece<P> {
    type Item = <P as UnindexedProducer>::Item;

    #[inline]
    fn split(self) -> (Self, Option<Self>) {
        // Our producers' size hint counts their candidates left, follow-ups included.
        // We can still split when they can't count them.
        let too_small = self
            .producer
            .size_hint()
            .1
            .is_some_and(|candidates| candidates < 2 * self.min_len);
        if too_small {
            return (self, None);
        }

        let min_len = self.min_len;
        let (left, right) = self.producer.split();

        (
            Piece {
                producer: left,
                min_len,
            },
            right.map(|producer| Piece { producer, min_len }),
        )
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        self.producer.fold_with(folder)
    }
}

impl<S: Shiperator> Shiper<S> {
    /// Folds every item left into `folder`, segment after segment, until it's full.
    ///
    /// `item` turns what `fold_segment` yields into the folder's items.
    #[inline]
    pub(super) fn fold_chunks<const WITH_ID: bool, T, F, M>(mut self, mut folder: F, item: M) -> F
    where
        F: Folder<T>,
        M: Fn(Option<EntityId>, S::Out) -> T + Copy,
    {
        loop {
            // A check per item would keep the bulk loops from vectorizing, so we only check
            // between chunks. Rayon's own producers check after every item.
            while !self.cursor.is_empty() {
                if folder.full() {
                    return folder;
                }

                let chunk_len = self.cursor.len().min(FOLD_CHUNK);
                let (mut chunk, rest) = self.cursor.split_at(chunk_len);
                self.cursor = rest;

                // SAFE: `chunk` is a prefix of our cursor, so it holds candidates of the segment
                // `shiperator` last entered, with that segment's local indices.
                // We own the schedule holding the plan of that segment, if any.
                folder = unsafe {
                    self.shiperator.fold_segment::<WITH_ID, _, _>(
                        &mut chunk,
                        folder,
                        move |folder, entity_id, out| folder.consume(item(entity_id, out)),
                    )
                };
            }

            if folder.full() || !self.enter_next_segment() {
                return folder;
            }
        }
    }
}

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
        // Rayon's `consume_iter` would drive us through `next`, we fold in bulk instead.
        self.fold_chunks::<false, _, _, _>(folder, |_, out| out)
    }
}

/// Producer of a traversal whose items are all in the current segment and that can't reject,
/// see [`Shiper::exact_len`].
///
/// `opt_len` reports a length only for these traversals and Rayon then requires us to drive
/// consumers with their indexed methods. We do that with `bridge_producer_consumer`,
/// which splits at the index it picks instead of letting us halve the segment.
pub(super) struct ExactProducer<P> {
    pub(super) producer: P,
    /// Each half of a split keeps at least this many items, see [`min_piece_len`].
    pub(super) min_len: usize,
}

impl<S: Shiperator + Send + Clone> Producer for ExactProducer<Shiper<S>> {
    type Item = S::Out;
    type IntoIter = Self;

    #[inline]
    fn into_iter(self) -> Self {
        self
    }

    #[inline]
    fn min_len(&self) -> usize {
        self.min_len
    }

    #[inline]
    fn split_at(self, index: usize) -> (Self, Self) {
        // Every candidate yields an item, the item at `index` is the candidate at `index`.
        let (left, right) = self.producer.split_current_segment_at(index);
        let min_len = self.min_len;

        (
            ExactProducer {
                producer: left,
                min_len,
            },
            ExactProducer {
                producer: right,
                min_len,
            },
        )
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        // Same as the unindexed producer.
        self.producer
            .fold_chunks::<false, _, _, _>(folder, |_, out| out)
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
    fn min_len(&self) -> usize {
        self.min_len
    }

    #[inline]
    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.producer.0.split_current_segment_at(index);
        let min_len = self.min_len;

        (
            ExactProducer {
                producer: WithId(left),
                min_len,
            },
            ExactProducer {
                producer: WithId(right),
                min_len,
            },
        )
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        self.producer
            .0
            .fold_chunks::<true, _, _, _>(folder, |entity_id, out| (with_id(entity_id), out))
    }
}

// `Producer` needs its items as a double ended, exact size iterator.
// We only hand producers to `bridge_producer_consumer`, which never asks for that iterator:
// it only splits producers and folds them with our `fold_with`.
impl<P: Iterator> Iterator for ExactProducer<P> {
    type Item = P::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.producer.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.producer.size_hint()
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
        let min_len = min_piece_len(producer.size_hint().1);
        let producer = WithId(producer);

        // This has to match `opt_len`, see `ParShiperator::drive_unindexed`.
        match producer.0.exact_len() {
            Some(len) => {
                bridge_producer_consumer(len, ExactProducer { producer, min_len }, consumer)
            }
            None => bridge_unindexed(Piece { producer, min_len }, consumer),
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
        let producer = self.0;
        let min_len = min_piece_len(producer.size_hint().1);

        // Once `opt_len` reports a length, consumers like `collect`'s may only be split with their
        // indexed methods. Both use `exact_len` on the same state so they always agree.
        match producer.exact_len() {
            Some(len) => {
                bridge_producer_consumer(len, ExactProducer { producer, min_len }, consumer)
            }
            None => bridge_unindexed(Piece { producer, min_len }, consumer),
        }
    }

    #[inline]
    fn opt_len(&self) -> Option<usize> {
        self.0.exact_len()
    }
}
