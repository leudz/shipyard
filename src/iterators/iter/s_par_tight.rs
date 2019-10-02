#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightFilter1, ParTightWithId1, Tight1, TightWithId1};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::prelude::{IndexedParallelIterator, ParallelIterator};

/// Parallel iterator over a single component.
#[cfg(feature = "parallel")]
pub struct ParTight1<T: IntoAbstract>(pub(super) Tight1<T>);

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParTight1<T> {
    pub fn with_id(self) -> ParTightWithId1<T> {
        ParTightWithId1(TightWithId1(self.0))
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParTight1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    pub fn filtered<F: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync>(
        self,
        pred: F,
    ) -> ParTightFilter1<T, F> {
        ParTightFilter1 { iter: self, pred }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParallelIterator for ParTight1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
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
impl<T: IntoAbstract> IndexedParallelIterator for ParTight1<T>
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
