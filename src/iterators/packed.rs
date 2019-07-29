use super::AbstractMut;
use crate::sparse_array::{View, ViewMut};
use std::marker::PhantomData;

// Packed iterators go from start to end without index lookup
// They only work in specific circumstances but are the fastest
pub struct Packed<'a, T: AbstractMut<'a>> {
    data: T,
    current: usize,
    end: usize,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T: 'a> IntoIterator for View<'a, T> {
    type IntoIter = Packed<'a, *const T>;
    type Item = <Self::IntoIter as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        Packed {
            end: self.len(),
            data: self.data.as_ptr(),
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a: 'b, 'b, T: 'a> IntoIterator for &'b View<'a, T> {
    type IntoIter = Packed<'b, *const T>;
    type Item = <Self::IntoIter as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        Packed {
            end: self.len(),
            data: self.data.as_ptr(),
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: 'a> IntoIterator for ViewMut<'a, T> {
    type IntoIter = Packed<'a, *mut T>;
    type Item = <Self::IntoIter as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        Packed {
            end: self.len(),
            data: self.data.as_mut_ptr(),
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a: 'b, 'b, T: 'a> IntoIterator for &'b ViewMut<'a, T> {
    type IntoIter = Packed<'b, *const T>;
    type Item = <Self::IntoIter as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        Packed {
            end: self.len(),
            data: self.data.as_ptr(),
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a: 'b, 'b, T: 'a> IntoIterator for &'b mut ViewMut<'a, T> {
    type IntoIter = Packed<'b, *mut T>;
    type Item = <Self::IntoIter as Iterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        Packed {
            end: self.len(),
            data: self.data.as_mut_ptr(),
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: AbstractMut<'a>> Iterator for Packed<'a, T> {
    type Item = T::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.add(current) })
        } else {
            None
        }
    }
}
