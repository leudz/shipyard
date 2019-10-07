#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParBuf, ParUpdateFilter1, ParUpdateWithId1, Update1};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, Producer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

#[cfg(feature = "parallel")]
pub struct ParUpdate1<T: IntoAbstract>(pub(super) Update1<T>);

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParUpdate1<T> {
    pub fn filtered<
        P: Fn(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool + Send + Sync,
    >(
        self,
        pred: P,
    ) -> ParUpdateFilter1<T, P> {
        ParUpdateFilter1 { iter: self, pred }
    }
    pub fn with_id(self) -> ParUpdateWithId1<T> {
        ParUpdateWithId1(self)
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> ParallelIterator for ParUpdate1<T>
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
    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> IndexedParallelIterator for ParUpdate1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    fn len(&self) -> usize {
        self.0.end - self.0.current
    }
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        use std::sync::atomic::Ordering;

        let mut data = self.0.data.clone();
        let len = self.0.end - self.0.current;
        let indices = ParBuf::new(len);

        let inner = InnerParUpdate1 {
            iter: self.0,
            indices: &indices,
        };

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
pub struct InnerParUpdate1<'a, T: IntoAbstract> {
    pub(super) iter: Update1<T>,
    pub(super) indices: &'a ParBuf<usize>,
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> Producer for InnerParUpdate1<'a, T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    type IntoIter = ParSeqUpdate1<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        ParSeqUpdate1 {
            indices: self.indices,
            data: self.iter.data,
            current: self.iter.current,
            end: self.iter.end,
        }
    }
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let clone = InnerParUpdate1 {
            // last_id is never read
            iter: Update1 {
                data: self.iter.data.clone(),
                current: self.iter.current + index,
                end: self.iter.end,
                last_id: Key::dead(),
            },
            indices: self.indices,
        };
        self.iter.end = clone.iter.current;
        (self, clone)
    }
}

#[cfg(feature = "parallel")]
pub struct ParSeqUpdate1<'a, T: IntoAbstract> {
    indices: &'a ParBuf<usize>,
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> Iterator for ParSeqUpdate1<'a, T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            self.indices.push(current);
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.get_data(current) })
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> DoubleEndedIterator for ParSeqUpdate1<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end > self.current {
            self.end -= 1;
            self.indices.push(self.end);
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.get_data(self.end) })
        } else {
            None
        }
    }
}

#[cfg(feature = "parallel")]
impl<'a, T: IntoAbstract> ExactSizeIterator for ParSeqUpdate1<'a, T> {
    fn len(&self) -> usize {
        self.end - self.current
    }
}
