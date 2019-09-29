use crate::iterators;
use iterators::{AbstractMut, IntoAbstract};
#[cfg(feature = "parallel")]
use iterators::{ParTight1, ParUpdateFilter1};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedConsumer;
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(feature = "parallel")]
pub enum ParFilter1<T: IntoAbstract, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    Tight(rayon::iter::Filter<ParTight1<T>, P>),
    Update(ParUpdateFilter1<T, P>),
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParFilter1<T, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        match self {
            ParFilter1::Tight(iter) => iter.drive_unindexed(consumer),
            ParFilter1::Update(iter) => iter.drive_unindexed(consumer),
        }
    }
}
