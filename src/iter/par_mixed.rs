use super::abstract_mut::AbstractMut;
use super::mixed::Mixed;
use rayon::iter::plumbing::{bridge_unindexed, UnindexedConsumer};
use rayon::iter::ParallelIterator;

#[allow(missing_docs)]
pub struct ParMixed<Storage>(Mixed<Storage>);

impl<Storage: AbstractMut> From<Mixed<Storage>> for ParMixed<Storage> {
    fn from(iter: Mixed<Storage>) -> Self {
        ParMixed(iter)
    }
}

impl<Storage: AbstractMut> ParallelIterator for ParMixed<Storage>
where
    Storage: Clone + Send,
    <Storage as AbstractMut>::Out: Send,
{
    type Item = <Mixed<Storage> as Iterator>::Item;

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_unindexed(self.0, consumer)
    }
}
