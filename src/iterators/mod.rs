mod iter;

use crate::not::Not;
use crate::sparse_array::{RawViewMut, View, ViewMut};
pub use iter::{Iter, NonPacked, Packed, ParIter, ParNonPacked, ParPacked, Chunk, ChunkExact};
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

#[doc(hidden)]
#[derive(Clone, Copy)]
pub enum Len {
    Indices((*const usize, usize)),
    Packed(usize),
}

// Allows to make ViewMut's sparse and dense fields immutable
// This is necessary to index into them
#[doc(hidden)]
pub trait IntoAbstract {
    type AbsView;
    fn into_abstract(self) -> Self::AbsView;
    // Assumes `type_ids` is sorted
    fn indices(&self, type_ids: &[TypeId]) -> Len;
    fn add_type_id(type_ids: &mut Vec<TypeId>);
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for View<'a, T> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        let pack_types = self.pack_types_owned();
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(self.len())
        } else if type_ids.len() == pack_types.len()
            && pack_types
                .iter()
                .zip(type_ids.iter())
                .all(|(pack_type_id, type_id)| pack_type_id == type_id)
        {
            Len::Packed(self.pack_len())
        } else {
            Len::Indices((self.dense.as_ptr(), self.dense.len()))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for &View<'a, T> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        let pack_types = self.pack_types_owned();
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(self.len())
        } else if type_ids.len() == pack_types.len()
            && pack_types
                .iter()
                .zip(type_ids.iter())
                .all(|(pack_type_id, type_id)| pack_type_id == type_id)
        {
            Len::Packed(self.pack_len())
        } else {
            Len::Indices((self.dense.as_ptr(), self.dense.len()))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for ViewMut<'a, T> {
    type AbsView = RawViewMut<'a, T>;
    fn into_abstract(self) -> Self::AbsView {
        self.into_raw()
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        let pack_types = self.pack_types_owned();
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(self.len())
        } else if type_ids.len() == pack_types.len()
            && pack_types
                .iter()
                .zip(type_ids.iter())
                .all(|(pack_type_id, type_id)| pack_type_id == type_id)
        {
            Len::Packed(self.pack_len())
        } else {
            Len::Indices((self.dense.as_ptr(), self.dense.len()))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b ViewMut<'a, T> {
    type AbsView = View<'b, T>;
    fn into_abstract(self) -> Self::AbsView {
        self.non_mut()
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        let pack_types = self.pack_types_owned();
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(self.len())
        } else if type_ids.len() == pack_types.len()
            && pack_types
                .iter()
                .zip(type_ids.iter())
                .all(|(pack_type_id, type_id)| pack_type_id == type_id)
        {
            Len::Packed(self.pack_len())
        } else {
            Len::Indices((self.dense.as_ptr(), self.dense.len()))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b mut ViewMut<'a, T> {
    type AbsView = RawViewMut<'b, T>;
    fn into_abstract(self) -> Self::AbsView {
        self.raw()
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        let pack_types = self.pack_types_owned();
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(self.len())
        } else if type_ids.len() == pack_types.len()
            && pack_types
                .iter()
                .zip(type_ids.iter())
                .all(|(pack_type_id, type_id)| pack_type_id == type_id)
        {
            Len::Packed(self.pack_len())
        } else {
            Len::Indices((self.dense.as_ptr(), self.dense.len()))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for Not<View<'a, T>> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for &Not<View<'a, T>> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for Not<&View<'a, T>> {
    type AbsView = Self;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for Not<ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'a, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.into_raw())
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b Not<ViewMut<'a, T>> {
    type AbsView = Not<View<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.non_mut())
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b mut Not<ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.raw())
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for Not<&'b ViewMut<'a, T>> {
    type AbsView = Not<View<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.non_mut())
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for Not<&'b mut ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'b, T>>;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.raw())
    }
    fn indices(&self, type_ids: &[TypeId]) -> Len {
        if type_ids.len() == 1 && type_ids[0] == TypeId::of::<T>() {
            Len::Packed(0)
        } else {
            Len::Indices((self.0.dense.as_ptr(), std::usize::MAX))
        }
    }
    fn add_type_id(type_ids: &mut Vec<TypeId>) {
        type_ids.push(TypeId::of::<T>());
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
    unsafe fn get_data(&mut self, index: usize) -> Self::Out;
    // # Safety
    // The lifetime has to be valid
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
    unsafe fn get_data(&mut self, _: usize) -> Self::Out {
        unreachable!()
    }
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
}

macro_rules! impl_abstract_mut {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: IntoAbstract),+> IntoAbstract for ($($type,)+) {
            type AbsView = ($($type::AbsView,)+);
            fn into_abstract(self) -> Self::AbsView {
                ($(self.$index.into_abstract(),)+)
            }
            // Recursively calls indices, if one of them can pack owned, return it
            // else return the smallest len
            fn indices(&self, type_ids: &[TypeId]) -> Len {
                let lens = [
                    $({
                        let len = self.$index.indices(type_ids);
                        if let Len::Packed(_) = len {
                            return len;
                        }
                        len
                    },)+
                ];
                *lens.iter().min_by(|len1, len2| if let Len::Indices(len1) = len1 {
                    if let Len::Indices(len2) = len2 {
                        len1.1.cmp(&len2.1)
                    } else {
                        unreachable!()
                    }
                } else {
                    unreachable!()
                }).unwrap()
            }
            fn add_type_id(type_ids: &mut Vec<TypeId>) {
                $(
                    $type::add_type_id(type_ids);
                )+
            }
        }
        impl<$($type: AbstractMut),+> AbstractMut for ($($type,)+) {
            type Out = ($($type::Out,)+);
            type Slice = ($($type::Slice,)+);
            unsafe fn abs_get(&mut self, index: usize) -> Option<Self::Out> {
                Some(($(self.$index.abs_get(index)?,)+))
            }
            unsafe fn get_data(&mut self, index: usize) -> Self::Out {
                ($(self.$index.get_data(index),)+)
            }
            unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
                ($(self.$index.get_data_slice(indices.clone()),)+)
            }
        }
    };
}

macro_rules! abstract_mut {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_abstract_mut![$(($type, $index))*];
        abstract_mut![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_abstract_mut![$(($type, $index))*];
    }
}

abstract_mut![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
