use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;

pub struct Update1<T: IntoAbstract> {
    pub(crate) data: T::AbsView,
    pub(crate) current: usize,
    pub(crate) end: usize,
    pub(crate) current_id: EntityId,
}

impl<T: IntoAbstract> Shiperator for Update1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;

    fn first_pass(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            self.current_id = unsafe { self.data.id_at(current) };
            Some(unsafe { self.data.get_data(current) })
        } else {
            None
        }
    }
    fn post_process(&mut self, _: Self::Item) -> Self::Item {
        unsafe { self.data.mark_id_modified(self.current_id) }
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
