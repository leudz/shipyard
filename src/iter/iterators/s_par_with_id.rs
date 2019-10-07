#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightWithId1, ParUpdateWithId1, ParWithIdFilter1};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[cfg(feature = "parallel")]
pub enum ParWithId1<T: IntoAbstract> {
    Tight(ParTightWithId1<T>),
    Update(ParUpdateWithId1<T>),
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParWithId1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    pub fn filtered<F: Fn(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool + Send + Sync>(
        self,
        pred: F,
    ) -> ParWithIdFilter1<T, F> {
        match self {
            ParWithId1::Tight(iter) => ParWithIdFilter1::Tight(iter.filtered(pred)),
            ParWithId1::Update(iter) => ParWithIdFilter1::Update(iter.filtered(pred)),
        }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParallelIterator for ParWithId1<T>
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
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> IndexedParallelIterator for ParWithId1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    fn len(&self) -> usize {
        match self {
            ParWithId1::Tight(iter) => iter.len(),
            ParWithId1::Update(iter) => iter.len(),
        }
    }
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        match self {
            ParWithId1::Tight(iter) => iter.with_producer(callback),
            ParWithId1::Update(iter) => iter.with_producer(callback),
        }
    }
}
