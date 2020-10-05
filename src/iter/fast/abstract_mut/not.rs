use super::FastAbstractMut;
use crate::not::Not;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use core::ops::Range;

impl<'w, T> FastAbstractMut for Not<&'w SparseSet<T>> {
    type Out = ();
    type Slice = ();

    #[inline]
    unsafe fn get_data(&self, _: usize) -> <Self as FastAbstractMut>::Out {}
    #[inline]
    unsafe fn get_data_slice(&self, _: Range<usize>) -> Self::Slice {}
    #[inline]
    unsafe fn get_datas(&self, _: Self::Index) -> <Self as FastAbstractMut>::Out {}
}

impl<'w, T> FastAbstractMut for Not<FullRawWindowMut<'w, T>> {
    type Out = ();
    type Slice = ();

    #[inline]
    unsafe fn get_data(&self, _: usize) -> <Self as FastAbstractMut>::Out {}
    #[inline]
    unsafe fn get_data_slice(&self, _: Range<usize>) -> Self::Slice {}
    #[inline]
    unsafe fn get_datas(&self, _: Self::Index) -> <Self as FastAbstractMut>::Out {}
}
