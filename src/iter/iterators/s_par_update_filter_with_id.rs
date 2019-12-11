#[cfg(feature = "parallel")]
use super::{AbstractMut, InnerParUpdate1, IntoAbstract, ParBuf, ParUpdateFilter1};
#[cfg(feature = "parallel")]
use crate::storage::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
#[cfg(feature = "parallel")]
use rayon::iter::{plumbing::Producer, ParallelIterator};

#[cfg(feature = "parallel")]
pub struct ParUpdateFilterWithId1<T: IntoAbstract, P>(pub(super) ParUpdateFilter1<T, P>);

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P> ParallelIterator for ParUpdateFilterWithId1<T, P>
where
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        use std::sync::atomic::Ordering;

        let mut data = self.0.iter.0.data.clone();
        let len = self.0.iter.0.end - self.0.iter.0.current;
        let indices = ParBuf::new(len);

        let producer = UpdateFilterWithIdProducer1 {
            inner: InnerParUpdate1 {
                iter: self.0.iter.0,
                indices: &indices,
            },
            pred: &self.0.pred,
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
pub struct UpdateFilterWithIdProducer1<'a, T: IntoAbstract, P> {
    inner: InnerParUpdate1<'a, T>,
    pred: &'a P,
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract, P> UnindexedProducer for UpdateFilterWithIdProducer1<'a, T, P>
where
    P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.inner.iter.end - self.inner.iter.current;
        if len >= 2 {
            let (left, right) = self.inner.split_at(len / 2);
            self.inner = left;
            let clone = UpdateFilterWithIdProducer1 {
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
                let id = unsafe { self.inner.iter.data.id_at(index) };
                self.inner.indices.push(index);
                folder = folder.consume((id, item));
            }
        }
        folder
    }
}
