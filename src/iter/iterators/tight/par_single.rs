use super::{AbstractMut, ExactSizeShiperator, IntoAbstract, Tight1};
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

/// Tight parallel iterator over a single component.
#[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
pub struct ParTight1<T: IntoAbstract>(Tight1<T>);

impl<T: IntoAbstract> From<Tight1<T>> for ParTight1<T> {
    fn from(iter: Tight1<T>) -> Self {
        ParTight1(iter)
    }
}

impl<T: IntoAbstract> ParallelIterator for ParTight1<T>
where
    T::AbsView: Clone + Send,
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

impl<T: IntoAbstract> IndexedParallelIterator for ParTight1<T>
where
    T::AbsView: Clone + Send,
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
