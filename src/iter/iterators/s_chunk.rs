use super::{AbstractMut, IntoAbstract};

/// Chunk iterator over a single component.
///
/// Returns `size` long slices and not single elements.
///
/// The last chunk's length will be smaller than `size` if `size` does not divide the iterator's length perfectly.
pub struct Chunk1<T: IntoAbstract> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) step: usize,
}

impl<T: IntoAbstract> Iterator for Chunk1<T> {
    type Item = <T::AbsView as AbstractMut>::Slice;
    fn next(&mut self) -> Option<Self::Item> {
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
}
