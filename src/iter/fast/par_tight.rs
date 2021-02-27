use super::abstract_mut::FastAbstractMut;
use super::tight::FastTight;
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[allow(missing_docs)]
pub struct FastParTight<Storage>(FastTight<Storage>);

impl<Storage: FastAbstractMut> From<FastTight<Storage>> for FastParTight<Storage> {
    fn from(iter: FastTight<Storage>) -> Self {
        FastParTight(iter)
    }
}

impl<Storage: FastAbstractMut> ParallelIterator for FastParTight<Storage>
where
    Storage: Clone + Send,
    <Storage as FastAbstractMut>::Out: Send,
{
    type Item = <FastTight<Storage> as Iterator>::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge(self, consumer)
    }
    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<Storage: FastAbstractMut> IndexedParallelIterator for FastParTight<Storage>
where
    Storage: Clone + Send,
    <Storage as FastAbstractMut>::Out: Send,
{
    fn len(&self) -> usize {
        self.0.len()
    }
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        callback.callback(self.0)
    }
}
