use crate::iterators;
use iterators::{AbstractMut, IntoAbstract, Tight1};

pub struct UpdateFilter1<
    T: IntoAbstract,
    P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool,
> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) pred: P,
}

impl<T: IntoAbstract, P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>
    Iterator for UpdateFilter1<T, P>
{
    type Item = <Tight1<T> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            if (self.pred)(unsafe { &self.data.get_data(self.current - 1) }) {
                return Some(unsafe { self.data.mark_modified(self.current - 1) });
            }
        }
        None
    }
}
