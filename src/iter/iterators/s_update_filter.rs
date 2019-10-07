use super::{AbstractMut, IntoAbstract, UpdateFilterWithId1};
use crate::entity::Key;

pub struct UpdateFilter1<T: IntoAbstract, P> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) pred: P,
    pub(super) last_id: Key,
}

impl<T: IntoAbstract, P> UpdateFilter1<T, P> {
    pub fn with_id(self) -> UpdateFilterWithId1<T, P> {
        UpdateFilterWithId1(self)
    }
}

impl<T: IntoAbstract, P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>
    Iterator for UpdateFilter1<T, P>
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            if (self.pred)(unsafe { &self.data.get_data(self.current - 1) }) {
                self.last_id = unsafe { self.data.id_at(self.current - 1) };
                return Some(unsafe { self.data.mark_modified(self.current - 1) });
            }
        }
        None
    }
}
