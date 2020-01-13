use super::{AbstractMut, IntoAbstract, Shiperator};

pub struct Chunk1<T: IntoAbstract> {
    pub(crate) data: T::AbsView,
    pub(crate) current: usize,
    pub(crate) end: usize,
    pub(crate) step: usize,
}

impl<T: IntoAbstract> Shiperator for Chunk1<T> {
    type Item = <T::AbsView as AbstractMut>::Slice;

    fn first_pass(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current + self.step < self.end {
            self.current += self.step;
            Some(unsafe { self.data.get_data_slice(current..(current + self.step)) })
        } else if current < self.end {
            self.current = self.end;
            Some(unsafe { self.data.get_data_slice(current..self.end) })
        } else {
            None
        }
    }
    fn post_process(&mut self, item: Self::Item) -> Self::Item {
        item
    }
}
