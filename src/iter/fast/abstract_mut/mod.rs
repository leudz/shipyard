mod inserted;
mod inserted_or_modified;
mod modified;
mod not;

use crate::iter::abstract_mut::AbstractMut;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use core::ops::Range;

pub trait FastAbstractMut: AbstractMut {
    type Out;
    type Slice;

    unsafe fn get_data(&self, index: usize) -> <Self as FastAbstractMut>::Out;
    unsafe fn get_data_slice(&self, index: Range<usize>) -> Self::Slice;
    unsafe fn get_datas(&self, index: Self::Index) -> <Self as FastAbstractMut>::Out;
}

impl<'tmp, T> FastAbstractMut for &'tmp SparseSet<T> {
    type Out = &'tmp T;
    type Slice = &'tmp [T];

    #[inline]
    unsafe fn get_data(&self, index: usize) -> <Self as FastAbstractMut>::Out {
        self.data.get_unchecked(index)
    }
    #[inline]
    unsafe fn get_data_slice(&self, index: Range<usize>) -> Self::Slice {
        self.data.get_unchecked(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> <Self as FastAbstractMut>::Out {
        self.data.get_unchecked(index)
    }
}

impl<'tmp, T> FastAbstractMut for FullRawWindowMut<'tmp, T> {
    type Out = &'tmp mut T;
    type Slice = &'tmp mut [T];

    #[inline]
    unsafe fn get_data(&self, index: usize) -> <Self as FastAbstractMut>::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    unsafe fn get_data_slice(&self, range: Range<usize>) -> Self::Slice {
        &mut *core::slice::from_raw_parts_mut(self.data.add(range.start), range.end - range.start)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> <Self as FastAbstractMut>::Out {
        &mut *self.data.add(index)
    }
}

macro_rules! impl_abstract_mut {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: FastAbstractMut),+> FastAbstractMut for ($($type,)+) where $(<$type as AbstractMut>::Index: From<usize>),+ {
            type Out = ($(<$type as FastAbstractMut>::Out,)+);
            type Slice = ($($type::Slice,)+);

            #[inline]
            unsafe fn get_data(&self, index: usize) -> <Self as FastAbstractMut>::Out {
                ($(FastAbstractMut::get_data(&self.$index, index),)+)
            }#[inline]
            unsafe fn get_data_slice(&self, range: Range<usize>) -> Self::Slice {
                ($(FastAbstractMut::get_data_slice(&self.$index, range.clone()),)+)
            }
            #[inline]
            unsafe fn get_datas(&self, index: Self::Index) -> <Self as FastAbstractMut>::Out {
                ($(FastAbstractMut::get_datas(&self.$index, index.$index),)+)
            }
        }
    }
}

macro_rules! abstract_mut {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_abstract_mut![$(($type, $index))*];
        abstract_mut![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_abstract_mut![$(($type, $index))*];
    }
}

abstract_mut![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
