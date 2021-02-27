use super::abstract_mut::FastAbstractMut;
use super::iter::FastIter;
use super::par_mixed::FastParMixed;
use super::par_tight::FastParTight;
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[allow(missing_docs)]
pub enum FastParIter<Storage> {
    Tight(FastParTight<Storage>),
    Mixed(FastParMixed<Storage>),
}

impl<Storage: FastAbstractMut> From<FastIter<Storage>> for FastParIter<Storage> {
    fn from(iter: FastIter<Storage>) -> Self {
        match iter {
            FastIter::Tight(tight) => FastParIter::Tight(tight.into()),
            FastIter::Mixed(mixed) => FastParIter::Mixed(mixed.into()),
        }
    }
}

impl<Storage: FastAbstractMut> ParallelIterator for FastParIter<Storage>
where
    Storage: Clone + Send,
    <Storage as FastAbstractMut>::Out: Send,
{
    type Item = <Storage as FastAbstractMut>::Out;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        match self {
            FastParIter::Tight(tight) => tight.drive(consumer),
            FastParIter::Mixed(mixed) => mixed.drive_unindexed(consumer),
        }
    }
    fn opt_len(&self) -> Option<usize> {
        match self {
            FastParIter::Tight(tight) => tight.opt_len(),
            FastParIter::Mixed(mixed) => mixed.opt_len(),
        }
    }
}
