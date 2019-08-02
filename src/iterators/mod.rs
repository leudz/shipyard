mod non_packed;
mod packed;

use crate::not::Not;
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
    fn indices(&self) -> (*const usize, Option<usize>);
}

impl<'a, T> IntoAbstract for View<'a, T> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
}

impl<'a, T> IntoAbstract for &View<'a, T> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
}

impl<'a, T> IntoAbstract for ViewMut<'a, T> {
    type View = ViewSemiMut<'a, T>;
    fn into_abstract(self) -> Self::View {
        self.into_semi_mut()
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for &'b ViewMut<'a, T> {
    type View = View<'b, T>;
    fn into_abstract(self) -> Self::View {
        self.non_mut()
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for &'b mut ViewMut<'a, T> {
    type View = ViewSemiMut<'b, T>;
    fn into_abstract(self) -> Self::View {
        self.semi_mut()
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
}

impl<'a, T> IntoAbstract for Not<View<'a, T>> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a, T> IntoAbstract for &Not<View<'a, T>> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a, T> IntoAbstract for Not<&View<'a, T>> {
    type View = Self;
    fn into_abstract(self) -> Self::View {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a, T> IntoAbstract for Not<ViewMut<'a, T>> {
    type View = Not<ViewSemiMut<'a, T>>;
    fn into_abstract(self) -> Self::View {
        Not(self.0.into_semi_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for &'b Not<ViewMut<'a, T>> {
    type View = Not<View<'b, T>>;
    fn into_abstract(self) -> Self::View {
        Not(self.0.non_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for &'b mut Not<ViewMut<'a, T>> {
    type View = Not<ViewSemiMut<'b, T>>;
    fn into_abstract(self) -> Self::View {
        Not(self.0.semi_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for Not<&'b ViewMut<'a, T>> {
    type View = Not<View<'b, T>>;
    fn into_abstract(self) -> Self::View {
        Not(self.0.non_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
}

impl<'a: 'b, 'b, T> IntoAbstract for Not<&'b mut ViewMut<'a, T>> {
    type View = Not<ViewSemiMut<'b, T>>;
    fn into_abstract(self) -> Self::View {
        Not(self.0.semi_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
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

impl<'a, T> AbstractMut for Not<View<'a, T>> {
    type Out = ();
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.0.contains_index(index) {
            None
        } else {
            Some(())
        }
    }
    unsafe fn abs_get_unchecked(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
    unsafe fn get_data(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
}

impl<'a, T> AbstractMut for &Not<View<'a, T>> {
    type Out = ();
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.0.contains_index(index) {
            None
        } else {
            Some(())
        }
    }
    unsafe fn abs_get_unchecked(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
    unsafe fn get_data(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
}

impl<'a, T> AbstractMut for Not<&View<'a, T>> {
    type Out = ();
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.0.contains_index(index) {
            None
        } else {
            Some(())
        }
    }
    unsafe fn abs_get_unchecked(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
    unsafe fn get_data(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
}

impl<'a, T> AbstractMut for Not<ViewSemiMut<'a, T>> {
    type Out = ();
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.0.contains_index(index) {
            None
        } else {
            Some(())
        }
    }
    unsafe fn abs_get_unchecked(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
    unsafe fn get_data(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
}
