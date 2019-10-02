#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParFilter1, ParTight1, ParUpdate1, ParWithId1};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[cfg(feature = "parallel")]
pub enum ParIter1<T: IntoAbstract> {
    Tight(ParTight1<T>),
    Update(ParUpdate1<T>),
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParIter1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    pub fn filtered<F: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync>(
        self,
        pred: F,
    ) -> ParFilter1<T, F> {
        match self {
            ParIter1::Tight(iter) => ParFilter1::Tight(iter.filtered(pred)),
            ParIter1::Update(iter) => ParFilter1::Update(iter.filtered(pred)),
        }
    }
    pub fn with_id(self) -> ParWithId1<T> {
        match self {
            ParIter1::Tight(iter) => ParWithId1::Tight(iter.with_id()),
            ParIter1::Update(iter) => ParWithId1::Update(iter.with_id()),
        }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParallelIterator for ParIter1<T>
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
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> IndexedParallelIterator for ParIter1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    fn len(&self) -> usize {
        match self {
            ParIter1::Tight(iter) => iter.len(),
            ParIter1::Update(iter) => iter.len(),
        }
    }
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        match self {
            ParIter1::Tight(iter) => iter.with_producer(callback),
            ParIter1::Update(iter) => iter.with_producer(callback),
        }
    }
}
