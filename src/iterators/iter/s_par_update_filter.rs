#[cfg(feature = "parallel")]
use super::{InnerParUpdate1, ParUpdate1};
use crate::iterators;
#[cfg(feature = "parallel")]
use iterators::ParBuf;
use iterators::{AbstractMut, IntoAbstract};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
#[cfg(feature = "parallel")]
use rayon::iter::{plumbing::Producer, ParallelIterator};

#[cfg(feature = "parallel")]
pub struct ParUpdateFilter1<T: IntoAbstract, P> {
    pub(super) iter: ParUpdate1<T>,
    pub(super) pred: P,
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParUpdateFilter1<T, P>
where
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        use std::sync::atomic::Ordering;

        let mut data = self.iter.0.data.clone();
        let len = self.iter.0.end - self.iter.0.current;
        let indices = ParBuf::new(len);

        let producer = UpdateFilterProducer1 {
            inner: InnerParUpdate1 {
                iter: self.iter.0,
                indices: &indices,
            },
            pred: &self.pred,
        };

        let result = bridge_unindexed(producer, consumer);

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
pub struct UpdateFilterProducer1<'a, T: IntoAbstract, P> {
    inner: InnerParUpdate1<'a, T>,
    pred: &'a P,
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract, P> UnindexedProducer for UpdateFilterProducer1<'a, T, P>
where
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.inner.iter.end - self.inner.iter.current;
        if len >= 2 {
            let (left, right) = self.inner.split_at(len / 2);
            self.inner = left;
            let clone = UpdateFilterProducer1 {
                inner: right,
                pred: self.pred,
            };
            (self, Some(clone))
        } else {
            (self, None)
        }
    }
    fn fold_with<F>(mut self, mut folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        for index in self.inner.iter.current..self.inner.iter.end {
            let item = unsafe { self.inner.iter.data.get_data(index) };
            if (self.pred)(&item) {
                self.inner.indices.push(index);
                folder = folder.consume(item);
            }
        }
        folder
    }
}
