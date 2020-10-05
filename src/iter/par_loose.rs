use super::abstract_mut::AbstractMut;
use super::loose::Loose;
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

pub struct ParLoose<Storage>(Loose<Storage>);

impl<Storage: AbstractMut> From<Loose<Storage>> for ParLoose<Storage> {
    fn from(iter: Loose<Storage>) -> Self {
        ParLoose(iter)
    }
}

impl<Storage: AbstractMut> ParallelIterator for ParLoose<Storage>
where
    Storage: Clone + Send,
    <Storage as AbstractMut>::Out: Send,
{
    type Item = <Loose<Storage> as Iterator>::Item;

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

impl<Storage: AbstractMut> IndexedParallelIterator for ParLoose<Storage>
where
    Storage: Clone + Send,
    <Storage as AbstractMut>::Out: Send,
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
