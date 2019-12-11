#[cfg(feature = "parallel")]
use super::{
    s_par_update::ParSeqUpdate1, AbstractMut, InnerParUpdate1, IntoAbstract, ParBuf, ParUpdate1,
    ParUpdateWithIdFilter1,
};
#[cfg(feature = "parallel")]
use crate::storage::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, Producer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[cfg(feature = "parallel")]
pub struct ParUpdateWithId1<T: IntoAbstract>(pub(super) ParUpdate1<T>);

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParUpdateWithId1<T> {
    pub fn filtered<
        P: Fn(&(Key, <<T as IntoAbstract>::AbsView as AbstractMut>::Out)) -> bool + Send + Sync,
    >(
        self,
        pred: P,
    ) -> ParUpdateWithIdFilter1<T, P> {
        ParUpdateWithIdFilter1 { iter: self, pred }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParallelIterator for ParUpdateWithId1<T>
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
impl<T: IntoAbstract> IndexedParallelIterator for ParUpdateWithId1<T>
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
        use std::sync::atomic::Ordering;

        let mut data = (self.0).0.data.clone();
        let len = self.0.len();
        let indices = ParBuf::new(len);

        let inner = InnerParUpdateWithId1(InnerParUpdate1 {
            iter: (self.0).0,
            indices: &indices,
        });

        let result = callback.callback(inner);
        let slice = unsafe {
            std::slice::from_raw_parts_mut(indices.buf, indices.len.load(Ordering::Relaxed))
        };
        slice.sort();
        for &mut index in slice {
            unsafe { data.mark_modified(index) };
        }
        result
    }
}

#[cfg(feature = "parallel")]
pub struct InnerParUpdateWithId1<'a, T: IntoAbstract>(pub(super) InnerParUpdate1<'a, T>);

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> Producer for InnerParUpdateWithId1<'a, T> {
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    type IntoIter = ParSeqUpdateWithId1<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ParSeqUpdateWithId1(self.0.into_iter())
    }
    fn split_at(self, index: usize) -> (Self, Self) {
        let (first, second) = self.0.split_at(index);
        (InnerParUpdateWithId1(first), InnerParUpdateWithId1(second))
    }
}

#[cfg(feature = "parallel")]
pub struct ParSeqUpdateWithId1<'a, T: IntoAbstract>(ParSeqUpdate1<'a, T>);

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> Iterator for ParSeqUpdateWithId1<'a, T> {
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next() {
            let id = unsafe { self.0.data.id_at(self.0.current - 1) };
            Some((id, item))
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> DoubleEndedIterator for ParSeqUpdateWithId1<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next_back() {
            let id = unsafe { self.0.data.id_at(self.0.end) };
            Some((id, item))
        } else {
            None
        }
    }
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> ExactSizeIterator for ParSeqUpdateWithId1<'a, T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}
