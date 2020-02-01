use super::{AbstractMut, IntoAbstract, ParTight1, ParUpdate1};
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

pub enum ParIter1<T: IntoAbstract> {
    Tight(ParTight1<T>),
    Update(ParUpdate1<T>),
}

impl<T: IntoAbstract> From<ParTight1<T>> for ParIter1<T> {
    fn from(par_iter: ParTight1<T>) -> Self {
        ParIter1::Tight(par_iter)
    }
}

impl<T: IntoAbstract> From<ParUpdate1<T>> for ParIter1<T> {
    fn from(par_iter: ParUpdate1<T>) -> Self {
        ParIter1::Update(par_iter)
    }
}

impl<T: IntoAbstract> ParallelIterator for ParIter1<T>
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

impl<T: IntoAbstract> IndexedParallelIterator for ParIter1<T>
where
    T::AbsView: Clone + Send,
    <T::AbsView as AbstractMut>::Out: Send,
{
    fn len(&self) -> usize {
        match self {
            Self::Tight(tight) => tight.len(),
            Self::Update(update) => update.len(),
        }
    }
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }
    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        match self {
            Self::Tight(tight) => tight.with_producer(callback),
            Self::Update(update) => update.with_producer(callback),
        }
    }
}
