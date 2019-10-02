#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightFilter1, TightFilter1};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge_unindexed, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(feature = "parallel")]
pub struct ParTightFilterWithId1<T: IntoAbstract, P>(pub(super) ParTightFilter1<T, P>);

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParTightFilterWithId1<T, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let pred = self.0.pred;
        let producer = TightFilter1 {
            iter: self.0.iter.0,
            pred: &pred,
        }
        .with_id();
        bridge_unindexed(producer, consumer)
    }
}
