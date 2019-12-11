use super::{AbstractMut, InnerShiperator, IntoAbstract};
use crate::storage::Key;

pub struct Update1<T: IntoAbstract> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) last_id: Key,
}

impl<T: IntoAbstract> Iterator for Update1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let first = self.first_pass()?;
        self.post_process(first)
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

impl<T: IntoAbstract> InnerShiperator for Update1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    type Index = usize;
    fn first_pass(&mut self) -> Option<(Self::Index, Self::Item)> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            Some((current, unsafe { self.data.get_data(current) }))
        } else {
            None
        }
    }
    #[inline]
    fn post_process(&mut self, (index, _): (Self::Index, Self::Item)) -> Option<Self::Item> {
        self.last_id = unsafe { self.data.id_at(index) };
        Some(unsafe { self.data.mark_modified(index) })
    }
    #[inline]
    fn last_id(&self) -> Key {
        self.last_id
    }
}
