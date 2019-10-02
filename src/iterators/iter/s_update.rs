use super::{AbstractMut, IntoAbstract, UpdateFilter1, UpdateWithId1};
use crate::entity::Key;

pub struct Update1<T: IntoAbstract> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) last_id: Key,
}

impl<T: IntoAbstract> Update1<T> {
    pub fn with_id(self) -> UpdateWithId1<T> {
        UpdateWithId1(self)
    }
}

impl<T: IntoAbstract> Update1<T> {
    pub fn filtered<P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>(
        self,
        pred: P,
    ) -> UpdateFilter1<T, P> {
        UpdateFilter1 {
            data: self.data,
            current: self.current,
            end: self.end,
            pred,
            last_id: Key::dead(),
        }
    }
}

impl<T: IntoAbstract> Iterator for Update1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            self.last_id = unsafe { self.data.id_at(current) };
            Some(unsafe { self.data.mark_modified(current) })
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
    fn filter<P>(self, _: P) -> std::iter::Filter<Self, P>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        panic!("use filtered instead");
    }
}

impl<T: IntoAbstract> DoubleEndedIterator for Update1<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end > self.current {
            self.end -= 1;
            self.last_id = unsafe { self.data.id_at(self.end) };
            let data = unsafe { self.data.mark_modified(self.end) };
            Some(data)
        } else {
            None
        }
    }
}

impl<T: IntoAbstract> ExactSizeIterator for Update1<T> {
    fn len(&self) -> usize {
        self.end - self.current
    }
}
