use super::abstract_mut::FastAbstractMut;
use super::loose::FastLoose;
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

pub struct FastParLoose<Storage>(FastLoose<Storage>);

impl<Storage: FastAbstractMut> From<FastLoose<Storage>> for FastParLoose<Storage> {
    fn from(iter: FastLoose<Storage>) -> Self {
        FastParLoose(iter)
    }
}

impl<Storage: FastAbstractMut> ParallelIterator for FastParLoose<Storage>
where
    Storage: Clone + Send,
    <Storage as FastAbstractMut>::Out: Send,
{
    type Item = <FastLoose<Storage> as Iterator>::Item;

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

impl<Storage: FastAbstractMut> IndexedParallelIterator for FastParLoose<Storage>
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
