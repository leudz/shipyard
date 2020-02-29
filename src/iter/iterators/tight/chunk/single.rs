use super::{AbstractMut, IntoAbstract, Shiperator};

/// Chunk iterator over a single component.  
/// Returns slices `size` long except for the last one that might be smaller.
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
            // SAFE we checked for OOB and the lifetime is ok
            Some(unsafe { self.data.get_data_slice(current..(current + self.step)) })
        } else if current < self.end {
            self.current = self.end;
            // SAFE we checked for OOB and the lifetime is ok
            Some(unsafe { self.data.get_data_slice(current..self.end) })
        } else {
            None
        }
    }
    fn post_process(&mut self) {}
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.end - self.current + self.step - 1) / self.step;
        (len, Some(len))
    }
}
