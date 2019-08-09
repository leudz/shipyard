mod iter;

use crate::not::Not;
use crate::sparse_array::{RawViewMut, View, ViewMut};
pub use iter::{
    Iter2, Iter3, Iter4, Iter5, NonPacked2, NonPacked3, NonPacked4, NonPacked5, Packed1, Packed2,
    Packed3, Packed4, Packed5, ParIter2, ParIter3, ParIter4, ParIter5, ParNonPacked2,
    ParNonPacked3, ParNonPacked4, ParNonPacked5, ParPacked1, ParPacked2, ParPacked3, ParPacked4,
    ParPacked5,
};
use std::any::TypeId;

// This trait exists because of conflicting implementations
// when using std::iter::IntoIterator
/// Trait used to create iterators.
///
/// `std::iter::Iterator` can't be used because of conflicting implementation.
/// This trait serves as substitute.
pub trait IntoIter {
    type IntoIter;
    type IntoParIter;
    /// Returns an iterator over storages yielding only components meeting the requirements.
    ///
    /// Iterators can only be made inside [run] closure and systems.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.new_entity((0usize, 1u32));
    /// world.new_entity((2usize, 3u32));
    /// world.run::<(&mut usize, &u32), _>(|(usizes, u32s)| {
    ///     for (x, &y) in (usizes, u32s).iter() {
    ///         *x += y as usize;
    ///     }
    /// });
    /// ```
    /// [run]: ../struct.World.html#method.run
    fn iter(self) -> Self::IntoIter;
    /// Returns a parallel iterator over storages yielding only components meeting the requirements.
    ///
    /// Iterators can only be made inside [run] closure and systems.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// use rayon::prelude::ParallelIterator;
    ///
    /// let world = World::new::<(usize, u32)>();
    /// world.new_entity((0usize, 1u32));
    /// world.new_entity((2usize, 3u32));
    /// world.run::<(&mut usize, &u32, ThreadPool), _>(|(usizes, u32s, thread_pool)| {
    ///     thread_pool.install(|| {
    ///         (usizes, u32s).par_iter().for_each(|(x, &y)| {
    ///             *x += y as usize;
    ///         });
    ///     })
    /// });
    /// ```
    /// [run]: ../struct.World.html#method.run
    fn par_iter(self) -> Self::IntoParIter;
}

// Allows to make ViewMut's sparse and dense fields immutable
// This is necessary to index into them
#[doc(hidden)]
pub trait IntoAbstract {
    type AbsView: AbstractMut;
    fn into_abstract(self) -> Self::AbsView;
    fn indices(&self) -> (*const usize, Option<usize>);
    fn abs_is_packed(&self) -> bool;
    fn abs_pack_types_owned(&self) -> &[TypeId];
    fn abs_pack_len(&self) -> usize;
}

impl<'a, T: Send + Sync> IntoAbstract for View<'a, T> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
    fn abs_is_packed(&self) -> bool {
        self.is_packed_owned()
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        self.pack_types_owned()
    }
    fn abs_pack_len(&self) -> usize {
        self.pack_len()
    }
}

impl<'a, T: Send + Sync> IntoAbstract for &View<'a, T> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
    fn abs_is_packed(&self) -> bool {
        self.is_packed_owned()
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        self.pack_types_owned()
    }
    fn abs_pack_len(&self) -> usize {
        self.pack_len()
    }
}

impl<'a, T: Send + Sync> IntoAbstract for ViewMut<'a, T> {
    type AbsView = RawViewMut<'a, T>;
    fn into_abstract(self) -> Self::AbsView {
        self.into_raw()
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
    fn abs_is_packed(&self) -> bool {
        self.is_packed_owned()
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        self.pack_types_owned()
    }
    fn abs_pack_len(&self) -> usize {
        self.pack_len()
    }
}

impl<'a: 'b, 'b, T: Send + Sync> IntoAbstract for &'b ViewMut<'a, T> {
    type AbsView = View<'b, T>;
    fn into_abstract(self) -> Self::AbsView {
        self.non_mut()
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
    fn abs_is_packed(&self) -> bool {
        self.is_packed_owned()
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        self.pack_types_owned()
    }
    fn abs_pack_len(&self) -> usize {
        self.pack_len()
    }
}

impl<'a: 'b, 'b, T: Send + Sync> IntoAbstract for &'b mut ViewMut<'a, T> {
    type AbsView = RawViewMut<'b, T>;
    fn into_abstract(self) -> Self::AbsView {
        self.raw()
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.dense.as_ptr(), Some(self.dense.len()))
    }
    fn abs_is_packed(&self) -> bool {
        self.is_packed_owned()
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        self.pack_types_owned()
    }
    fn abs_pack_len(&self) -> usize {
        self.pack_len()
    }
}

impl<'a, T: Send + Sync> IntoAbstract for Not<View<'a, T>> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a, T: Send + Sync> IntoAbstract for &Not<View<'a, T>> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a, T: Send + Sync> IntoAbstract for Not<&View<'a, T>> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a, T: Send + Sync> IntoAbstract for Not<ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'a, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.into_raw())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a: 'b, 'b, T: Send + Sync> IntoAbstract for &'b Not<ViewMut<'a, T>> {
    type AbsView = Not<View<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.non_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a: 'b, 'b, T: Send + Sync> IntoAbstract for &'b mut Not<ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.raw())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a: 'b, 'b, T: Send + Sync> IntoAbstract for Not<&'b ViewMut<'a, T>> {
    type AbsView = Not<View<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.non_mut())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

impl<'a: 'b, 'b, T: Send + Sync> IntoAbstract for Not<&'b mut ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.raw())
    }
    fn indices(&self) -> (*const usize, Option<usize>) {
        (self.0.dense.as_ptr(), None)
    }
    fn abs_is_packed(&self) -> bool {
        false
    }
    fn abs_pack_types_owned(&self) -> &[TypeId] {
        &[]
    }
    fn abs_pack_len(&self) -> usize {
        0
    }
}

// Abstracts different types of view to iterate over
// mutable and immutable views with the same iterator
#[doc(hidden)]
pub trait AbstractMut: Clone + Send {
    type Out;
    type Slice;
    // # Safety
    // The lifetime has to be valid
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out>;
    // # Safety
    // The lifetime has to be valid
    unsafe fn abs_get_unchecked(&mut self, index: usize) -> Self::Out;
    // # Safety
    // The lifetime has to be valid
    unsafe fn get_data(&mut self, index: usize) -> Self::Out;
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice;
}

impl<'a, T: Send + Sync> AbstractMut for View<'a, T> {
    type Out = &'a T;
    type Slice = &'a [T];
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
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
        &std::slice::from_raw_parts(
            self.data.as_ptr().add(indices.start),
            indices.end - indices.start,
        )
    }
}

impl<'a, T: Send + Sync> AbstractMut for &View<'a, T> {
    type Out = &'a T;
    type Slice = &'a [T];
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
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
        std::slice::from_raw_parts(
            self.data.as_ptr().add(indices.start),
            indices.end - indices.start,
        )
    }
}

impl<'a, T: 'a + Send + Sync> AbstractMut for RawViewMut<'a, T> {
    type Out = &'a mut T;
    type Slice = &'a mut [T];
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.contains(index) {
            Some(&mut *(self.data.add(*self.sparse.get_unchecked(index)) as *mut _))
        } else {
            None
        }
    }
    unsafe fn abs_get_unchecked(&mut self, index: usize) -> Self::Out {
        &mut *(self.data.add(*self.sparse.get_unchecked(index)) as *mut _)
    }
    unsafe fn get_data(&mut self, count: usize) -> Self::Out {
        &mut *self.data.add(count)
    }
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
        std::slice::from_raw_parts_mut(self.data.add(indices.start), indices.end - indices.start)
    }
}

impl<'a, T: Send + Sync> AbstractMut for Not<View<'a, T>> {
    type Out = ();
    type Slice = ();
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
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
}

impl<'a, T: Send + Sync> AbstractMut for &Not<View<'a, T>> {
    type Out = ();
    type Slice = ();
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
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
}

impl<'a, T: Send + Sync> AbstractMut for Not<&View<'a, T>> {
    type Out = ();
    type Slice = ();
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
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
}

impl<'a, T: Send + Sync> AbstractMut for Not<RawViewMut<'a, T>> {
    type Out = ();
    type Slice = ();
    unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
        if self.0.contains(index) {
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
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
}
