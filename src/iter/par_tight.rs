use super::abstract_mut::AbstractMut;
use super::tight::Tight;
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[allow(missing_docs)]
pub struct ParTight<Storage>(Tight<Storage>);

impl<Storage: AbstractMut> From<Tight<Storage>> for ParTight<Storage> {
    fn from(iter: Tight<Storage>) -> Self {
        ParTight(iter)
    }
}

impl<Storage: AbstractMut> ParallelIterator for ParTight<Storage>
where
    Storage: Clone + Send,
    <Storage as AbstractMut>::Out: Send,
{
    type Item = <Tight<Storage> as Iterator>::Item;

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }

    #[inline]
    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<Storage: AbstractMut> IndexedParallelIterator for ParTight<Storage>
where
    Storage: Clone + Send,
    <Storage as AbstractMut>::Out: Send,
{
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    #[inline]
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        callback.callback(self.0)
    }
}
