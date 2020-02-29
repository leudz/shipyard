use super::{AbstractMut, IntoAbstract, IntoIterator, Update1};
use rayon::iter::plumbing::{bridge, Consumer, Producer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

/// Update parallel iterator over a single component.
pub struct ParUpdate1<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
}

impl<T: IntoAbstract> From<Update1<T>> for ParUpdate1<T> {
    fn from(update: Update1<T>) -> Self {
        ParUpdate1 {
            data: update.data,
            current: update.current,
            end: update.end,
        }
    }
}

impl<T: IntoAbstract> Iterator for IntoIterator<ParUpdate1<T>> {
    type Item = <T::AbsView as AbstractMut>::Out;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.0.current;
        if current < self.0.end {
            self.0.current += 1;
            // SAFE we checked for OOB and the lifetime is ok
            Some(unsafe { self.0.data.get_data(current) })
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<T: IntoAbstract> ExactSizeIterator for IntoIterator<ParUpdate1<T>> {
    fn len(&self) -> usize {
        self.0.end - self.0.current
    }
}

impl<T: IntoAbstract> DoubleEndedIterator for IntoIterator<ParUpdate1<T>> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.0.current < self.0.end {
            self.0.end -= 1;
            // SAFE we checked for OOB and the lifetime is ok
            Some(unsafe { self.0.data.get_data(self.0.end) })
        } else {
            None
        }
    }
}

impl<T: IntoAbstract> Producer for ParUpdate1<T>
where
    T::AbsView: Clone + Send,
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    type IntoIter = IntoIterator<Self>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIterator(self)
    }
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let clone = ParUpdate1 {
            data: self.data.clone(),
            current: self.current + index,
            end: self.end,
        };
        self.end = clone.current;
        (self, clone)
    }
}

impl<T: IntoAbstract> ParallelIterator for ParUpdate1<T>
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

impl<T: IntoAbstract> IndexedParallelIterator for ParUpdate1<T>
where
    T::AbsView: Clone + Send,
    <T::AbsView as AbstractMut>::Out: Send,
{
    fn len(&self) -> usize {
        self.end - self.current
    }
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        bridge(self, consumer)
    }
    fn with_producer<CB>(mut self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        self.data.flag_all();
        callback.callback(self)
    }
}
