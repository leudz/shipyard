use super::FastAbstractMut;
use crate::pack::shared::WithShared;
use crate::sparse_set::SparseSet;
use core::ops::Range;

impl<'tmp, T> FastAbstractMut for WithShared<&'tmp SparseSet<T>> {
    type Out = &'tmp T;
    type Slice = &'tmp [T];

    #[inline]
    unsafe fn get_data(&self, index: usize) -> <Self as FastAbstractMut>::Out {
        FastAbstractMut::get_data(&self.0, index)
    }
    #[inline]
    unsafe fn get_data_slice(&self, range: Range<usize>) -> Self::Slice {
        FastAbstractMut::get_data_slice(&self.0, range)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> <Self as FastAbstractMut>::Out {
        FastAbstractMut::get_datas(&self.0, index)
    }
}
