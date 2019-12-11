#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightWithIdFilter1, TightWithId1};
#[cfg(feature = "parallel")]
use crate::storage::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[cfg(feature = "parallel")]
pub struct ParTightWithId1<T: IntoAbstract>(pub(super) TightWithId1<T>);

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParTightWithId1<T> {
    pub fn filtered<P: Fn(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool + Send + Sync>(
        self,
        pred: P,
    ) -> ParTightWithIdFilter1<T, P> {
        ParTightWithIdFilter1 { iter: self, pred }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParallelIterator for ParTightWithId1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
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

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> IndexedParallelIterator for ParTightWithId1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
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
