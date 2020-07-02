use super::Pack;
use super::WindowMut;
use crate::error;
use crate::sparse_set::sort::IntoSortable;
use alloc::vec::Vec;
use core::cmp::Ordering;

pub struct WindowSort1<'tmp, 'w, T>(&'tmp mut WindowMut<'w, T>);

impl<'tmp, 'w, T> IntoSortable for &'tmp mut WindowMut<'w, T> {
    type IntoSortable = WindowSort1<'tmp, 'w, T>;
    fn sort(self) -> Self::IntoSortable {
        WindowSort1(self)
    }
}

impl<'tmp, 'w, T> WindowSort1<'tmp, 'w, T> {
    /// Sorts the storage(s) using an unstable algorithm, it may reorder equal components.
    pub fn try_unstable(self, mut cmp: impl FnMut(&T, &T) -> Ordering) -> Result<(), error::Sort> {
        if core::mem::discriminant(&self.0.metadata().pack)
            == core::mem::discriminant(&Pack::NoPack)
        {
            let mut transform: Vec<usize> = (0..self.0.len()).collect();

            transform.sort_unstable_by(|&i, &j| {
                // SAFE dense and data have the same length
                cmp(unsafe { self.0.data.get_unchecked(i) }, unsafe {
                    self.0.data.get_unchecked(j)
                })
            });

            let mut pos;
            for i in 0..transform.len() {
                // SAFE we're in bound
                pos = unsafe { *transform.get_unchecked(i) };
                while pos < i {
                    // SAFE we're in bound
                    pos = unsafe { *transform.get_unchecked(pos) };
                }
                self.0.dense.swap(i, pos);
                self.0.data.swap(i, pos);
            }

            for i in 0..self.0.dense.len() {
                // SAFE dense can always index into sparse
                unsafe {
                    let dense = *self.0.dense.get_unchecked(i);
                    self.0
                        .sparse
                        .set_sparse_index_unchecked(dense, i + self.0.offset);
                }
            }

            Ok(())
        } else {
            Err(error::Sort::MissingPackStorage)
        }
    }
    /// Sorts the storage(s) using an unstable algorithm, it may reorder equal components.  
    /// Unwraps errors.
    pub fn unstable(self, cmp: impl FnMut(&T, &T) -> Ordering) {
        self.try_unstable(cmp).unwrap()
    }
}
