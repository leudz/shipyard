#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTight1, ParTightFilterWithId1};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedConsumer;
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(feature = "parallel")]
pub struct ParTightFilter1<T: IntoAbstract, P> {
    pub(super) iter: ParTight1<T>,
    pub(super) pred: P,
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParTightFilter1<T, P> {
    pub fn with_id(self) -> ParTightFilterWithId1<T, P> {
        ParTightFilterWithId1(self)
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParTightFilter1<T, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.iter.filter(self.pred).drive_unindexed(consumer)
    }
}
