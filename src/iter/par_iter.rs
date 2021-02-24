use super::abstract_mut::AbstractMut;
use super::iter::Iter;
use super::par_mixed::ParMixed;
use super::par_tight::ParTight;
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

pub enum ParIter<Storage> {
    #[allow(missing_docs)]
    Tight(ParTight<Storage>),
    #[allow(missing_docs)]
    Mixed(ParMixed<Storage>),
}

impl<Storage: AbstractMut> From<Iter<Storage>> for ParIter<Storage> {
    fn from(iter: Iter<Storage>) -> Self {
        match iter {
            Iter::Tight(tight) => ParIter::Tight(tight.into()),
            Iter::Mixed(mixed) => ParIter::Mixed(mixed.into()),
        }
    }
}

impl<Storage: AbstractMut> ParallelIterator for ParIter<Storage>
where
    Storage: Clone + Send,
    <Storage as AbstractMut>::Out: Send,
{
    type Item = <Storage as AbstractMut>::Out;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        match self {
            ParIter::Tight(tight) => tight.drive(consumer),
            ParIter::Mixed(mixed) => mixed.drive_unindexed(consumer),
        }
    }
    fn opt_len(&self) -> Option<usize> {
        match self {
            ParIter::Tight(tight) => tight.opt_len(),
            ParIter::Mixed(mixed) => mixed.opt_len(),
        }
    }
}
