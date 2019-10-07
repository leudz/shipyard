#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParTightWithIdFilter1, ParUpdateWithIdFilter1};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedConsumer;
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(feature = "parallel")]
pub enum ParWithIdFilter1<T: IntoAbstract, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    Tight(ParTightWithIdFilter1<T, P>),
    Update(ParUpdateWithIdFilter1<T, P>),
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParWithIdFilter1<T, P>
where
    <T::AbsView as AbstractMut>::Out: Send,
    P: Fn(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool + Send + Sync,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        match self {
            ParWithIdFilter1::Tight(iter) => iter.drive_unindexed(consumer),
            ParWithIdFilter1::Update(iter) => iter.drive_unindexed(consumer),
        }
    }
}
