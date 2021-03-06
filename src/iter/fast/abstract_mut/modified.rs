use super::FastAbstractMut;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::tracking::Modified;
use core::ops::Range;

impl<'tmp, T> FastAbstractMut for Modified<&'tmp SparseSet<T>> {
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

impl<'tmp, T> FastAbstractMut for Modified<FullRawWindowMut<'tmp, T>> {
    type Out = &'tmp mut T;
    type Slice = &'tmp mut [T];

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
