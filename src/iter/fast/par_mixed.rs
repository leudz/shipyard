use super::abstract_mut::FastAbstractMut;
use super::mixed::FastMixed;
use rayon::iter::plumbing::{bridge_unindexed, UnindexedConsumer};
use rayon::iter::ParallelIterator;

pub struct FastParMixed<Storage>(FastMixed<Storage>);

impl<Storage: FastAbstractMut> From<FastMixed<Storage>> for FastParMixed<Storage> {
    fn from(iter: FastMixed<Storage>) -> Self {
        FastParMixed(iter)
    }
}

impl<Storage: FastAbstractMut> ParallelIterator for FastParMixed<Storage>
where
    Storage: Clone + Send,
    <Storage as FastAbstractMut>::Out: Send,
{
    type Item = <FastMixed<Storage> as Iterator>::Item;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(self.0, consumer)
    }
}
