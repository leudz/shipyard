use super::{
    AbstractMut, CurrentId, DoubleEndedShiperator, ExactSizeShiperator, IntoAbstract, Shiperator,
};
use crate::EntityId;

pub struct Update1<T: IntoAbstract> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    current_id: EntityId,
}

impl<T: IntoAbstract> Update1<T> {
    pub(crate) fn new(data: T) -> Self {
        Update1 {
            current: 0,
            end: data.len().unwrap_or(0),
            current_id: EntityId::dead(),
            data: data.into_abstract(),
        }
    }
}

impl<T: IntoAbstract> Shiperator for Update1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;

    fn first_pass(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            self.current_id = unsafe { self.data.id_at(current) };
            Some(unsafe { self.data.get_update_data(current) })
        } else {
            None
        }
    }
    fn post_process(&mut self) {
        unsafe { self.data.flag(self.current_id) }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end - self.current;
        (len, Some(len))
    }
}

impl<T: IntoAbstract> CurrentId for Update1<T> {
    type Id = EntityId;

    unsafe fn current_id(&self) -> Self::Id {
        self.current_id
    }
}

impl<T: IntoAbstract> ExactSizeShiperator for Update1<T> {}

impl<T: IntoAbstract> DoubleEndedShiperator for Update1<T> {
    fn first_pass_back(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.end -= 1;
            self.current_id = unsafe { self.data.id_at(self.end) };
            Some(unsafe { self.data.get_update_data(self.end) })
        } else {
            None
        }
    }
}
