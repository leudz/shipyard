#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightWithId1};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedConsumer;
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(feature = "parallel")]
pub struct ParTightWithIdFilter1<T: IntoAbstract, P> {
    pub(super) iter: ParTightWithId1<T>,
    pub(super) pred: P,
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParTightWithIdFilter1<T, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
    P: Fn(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool + Send + Sync,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.iter.filter(self.pred).drive_unindexed(consumer)
    }
}
