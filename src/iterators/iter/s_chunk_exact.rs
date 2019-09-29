use crate::iterators::{AbstractMut, IntoAbstract};

/// Chunk exact iterator over a single component.
///
/// Returns `size` long slices and not single elements.
///
/// The slices length will always by the same. To get the remaining elements (if any) use [remainder].
///
/// [remainder]: struct.ChunkExact1.html#method.remainder
pub struct ChunkExact1<T: IntoAbstract> {
    pub(super) data: T::AbsView,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) step: usize,
}

impl<T: IntoAbstract> ChunkExact1<T> {
    /// Returns the items at the end of the slice.
    ///
    /// Will always return a slice smaller than `size`.
    pub fn remainder(&mut self) -> <T::AbsView as AbstractMut>::Slice {
        let remainder = std::cmp::min(self.end - self.current, self.end % self.step);
        let old_end = self.end;
        self.end -= remainder;
        unsafe { self.data.get_data_slice(self.end..old_end) }
    }
}

impl<T: IntoAbstract> Iterator for ChunkExact1<T> {
    type Item = <T::AbsView as AbstractMut>::Slice;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current + self.step <= self.end {
            self.current += self.step;
            Some(unsafe { self.data.get_data_slice(current..self.current) })
        } else {
            None
        }
    }
}
