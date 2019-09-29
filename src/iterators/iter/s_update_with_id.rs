use super::Update1;
use crate::entity::Key;
use crate::iterators;
use iterators::{AbstractMut, IntoAbstract};

pub struct UpdateWithId1<T: IntoAbstract>(pub(super) Update1<T>);

impl<T: IntoAbstract> Iterator for UpdateWithId1<T> {
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.0.current;
        if current < self.0.end {
            self.0.current += 1;
            let id = unsafe { self.0.data.id_at(current) };
            let data = unsafe { self.0.data.mark_modified(current) };
            Some((id, data))
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<T: IntoAbstract> DoubleEndedIterator for UpdateWithId1<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.0.end > self.0.current {
            self.0.end -= 1;
            let id = unsafe { self.0.data.id_at(self.0.end) };
            let data = unsafe { self.0.data.mark_modified(self.0.end) };
            Some((id, data))
        } else {
            None
        }
    }
}

impl<T: IntoAbstract> ExactSizeIterator for UpdateWithId1<T> {
    fn len(&self) -> usize {
        self.0.end - self.0.current
    }
}
