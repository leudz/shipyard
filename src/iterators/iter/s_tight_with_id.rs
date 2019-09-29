use crate::entity::Key;
use crate::iterators;
use iterators::{AbstractMut, IntoAbstract, Tight1};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

pub struct TightWithId1<T: IntoAbstract>(pub(super) Tight1<T>);

impl<T: IntoAbstract> TightWithId1<T> {
    pub fn filtered<F: FnMut(&<Self as Iterator>::Item) -> bool>(
        self,
        pred: F,
    ) -> std::iter::Filter<Self, F> {
        self.filter(pred)
    }
}

impl<T: IntoAbstract> Iterator for TightWithId1<T> {
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|item| {
            let id = unsafe { self.0.data.id_at(self.0.current - 1) };
            (id, item)
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<T: IntoAbstract> DoubleEndedIterator for TightWithId1<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|item| {
            let id = unsafe { self.0.data.id_at(self.0.end) };
            (id, item)
        })
    }
}

impl<T: IntoAbstract> ExactSizeIterator for TightWithId1<T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> Producer for TightWithId1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    type IntoIter = Self;
    fn into_iter(self) -> Self::IntoIter {
        self
    }
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let (left, right) = self.0.split_at(index);
        self.0 = left;
        let clone = TightWithId1(right);
        (self, clone)
    }
}
