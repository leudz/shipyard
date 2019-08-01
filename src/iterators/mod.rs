mod non_packed;
mod packed;

use crate::sparse_array::{View, ViewMut, ViewSemiMut};

// This trait exists because of conflicting implementations
// when using std::iter::IntoIterator
pub trait IntoIter {
    type IntoIter;
    fn into_iter(self) -> Self::IntoIter;
}

// Allows to make ViewMut's sparse and dense fields immutable
// This is necessary to index into them
pub trait IntoAbstract {
    type View: AbstractMut;
    fn into_abstract(self) -> Self::View;
    fn indices(&self) -> (*const usize, usize);
}

impl<'a, T> IntoAbstract for View<'a, T> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, usize) {
        (self.dense.as_ptr(), self.dense.len())
    }
}

impl<'a, T> IntoAbstract for &View<'a, T> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, usize) {
        (self.dense.as_ptr(), self.dense.len())
    }
}

impl<'a, T> IntoAbstract for ViewMut<'a, T> {
    type View = ViewSemiMut<'a, T>;
    fn into_abstract(self) -> Self::View {
        self.into_semi_mut()
    }
    fn indices(&self) -> (*const usize, usize) {
        (self.dense.as_ptr(), self.dense.len())
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for &'b ViewMut<'a, T> {
    type View = View<'b, T>;
    fn into_abstract(self) -> Self::View {
        self.non_mut()
    }
    fn indices(&self) -> (*const usize, usize) {
        (self.dense.as_ptr(), self.dense.len())
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for &'b mut ViewMut<'a, T> {
    type View = ViewSemiMut<'b, T>;
    fn into_abstract(self) -> Self::View {
        self.semi_mut()
    }
    fn indices(&self) -> (*const usize, usize) {
        (self.dense.as_ptr(), self.dense.len())
    }
}

// Abstracts different types of view to iterate over
// mutable and immutable views with the same iterator
pub trait AbstractMut {
    type Out;
    // # Safety
    // The lifetime has to be valid
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out>;
    // # Safety
    // The lifetime has to be valid
    unsafe fn abs_get_unchecked(&mut self, index: usize) -> Self::Out;
    // # Safety
    // The lifetime has to be valid
    unsafe fn get_data(&mut self, index: usize) -> Self::Out;
}

impl<'a, T> AbstractMut for View<'a, T> {
    type Out = &'a T;
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.contains_index(index) {
            Some(self.data.get_unchecked(*self.sparse.get_unchecked(index)))
        } else {
            None
        }
    }
    unsafe fn abs_get_unchecked(&mut self, index: usize) -> Self::Out {
        self.data.get_unchecked(*self.sparse.get_unchecked(index))
    }
    unsafe fn get_data(&mut self, count: usize) -> Self::Out {
        &*self.data.as_ptr().add(count)
    }
}

impl<'a, T> AbstractMut for &View<'a, T> {
    type Out = &'a T;
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.contains_index(index) {
            Some(self.data.get_unchecked(*self.sparse.get_unchecked(index)))
        } else {
            None
        }
    }
    unsafe fn abs_get_unchecked(&mut self, index: usize) -> Self::Out {
        self.data.get_unchecked(*self.sparse.get_unchecked(index))
    }
    unsafe fn get_data(&mut self, count: usize) -> Self::Out {
        &*self.data.as_ptr().add(count)
    }
}

impl<'a, T> AbstractMut for ViewSemiMut<'a, T> {
    type Out = &'a mut T;
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.contains_index(index) {
            Some(
                &mut *(self
                    .data
                    .get_unchecked_mut(*self.sparse.get_unchecked(index))
                    as *mut _),
            )
        } else {
            None
        }
    }
    unsafe fn abs_get_unchecked(&mut self, index: usize) -> Self::Out {
        &mut *(self
            .data
            .get_unchecked_mut(*self.sparse.get_unchecked(index)) as *mut _)
    }
    unsafe fn get_data(&mut self, count: usize) -> Self::Out {
        &mut *self.data.as_mut_ptr().add(count)
    }
}
