use super::FastAbstractMut;
use crate::iter::InsertedOrModified;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use core::ops::Range;

impl<'tmp, T> FastAbstractMut for InsertedOrModified<&'tmp SparseSet<T>> {
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

impl<'tmp, T> FastAbstractMut for InsertedOrModified<FullRawWindowMut<'tmp, T>> {
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
