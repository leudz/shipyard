#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightFilterWithId1, ParUpdateFilterWithId1};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedConsumer;
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(feature = "parallel")]
pub enum ParFilterWithId1<T: IntoAbstract, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    Tight(ParTightFilterWithId1<T, P>),
    Update(ParUpdateFilterWithId1<T, P>),
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParFilterWithId1<T, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        match self {
            ParFilterWithId1::Tight(iter) => iter.drive_unindexed(consumer),
            ParFilterWithId1::Update(iter) => iter.drive_unindexed(consumer),
        }
    }
}
