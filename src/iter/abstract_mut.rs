use crate::not::Not;
use crate::sparse_set::{RawWindowMut, Window};
use crate::storage::EntityId;

// Abstracts different types of view to iterate over
// mutable and immutable views with the same iterator
pub trait AbstractMut {
    type Out;
    type Slice;
    /// # Safety
    ///
    /// `index` has to be between 0 and self.len() and `Out` needs a correct lifetime when used on `Window` or `RawWindowMut`.
    unsafe fn get_data(&self, index: usize) -> Self::Out;
    /// # Safety
    ///
    /// `index` has to be between 0 and self.len().  
    /// No other borrow should be in progress on `index` on `RawWindowMut`.  
    /// Only one call to this function can happen at a time on `RawWindowMut`.  
    /// `Out` needs a correct lifetime when used on `Window` or `RawWindowMut`.
    unsafe fn get_update_data(&self, index: usize) -> Self::Out;
    /// # Safety
    ///
    /// `indices` has to be between 0 and self.len() and `Slice` needs a correct lifetime when used on `Window` or `RawWindowMut`.
    unsafe fn get_data_slice(&self, indices: core::ops::Range<usize>) -> Self::Slice;
    fn dense(&self) -> *const EntityId;
    /// # Safety
    ///
    /// `index` has to be between 0 and window.len().
    unsafe fn id_at(&self, index: usize) -> EntityId;
    fn index_of(&self, entity: EntityId) -> Option<usize>;
    /// # Safety
    ///
    /// `entity` has to own a component in `self` when used on `Window` or `RawWindowMut`.
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize;
    fn flag_all(&mut self);
    /// # Safety
    ///
    /// When used on `RawWindowMut`:\
    /// `entity` has to own a component in `self`.\
    /// This method can only be called once at a time.\
    /// No borrow must be in progress on `entity` nor `first_non_mod`.
    unsafe fn flag(&self, entity: EntityId);
}

macro_rules! window {
    ($($window: ty);+) => {
        $(
            impl<'w, T> AbstractMut for $window {
                type Out = &'w T;
                type Slice = &'w [T];
                unsafe fn get_data(&self, index: usize) -> Self::Out {
                    self.get_at_unbounded_0(index)
                }
                unsafe fn get_update_data(&self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn get_data_slice(&self, indices: core::ops::Range<usize>) -> Self::Slice {
                    self.get_at_unbounded_slice_0(indices)
                }
                fn dense(&self) -> *const EntityId {
                    self.dense_ptr()
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    <Window<'_, T>>::try_id_at(self, index).unwrap()
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    (*self).index_of(entity)
                }
                unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
                    (*self).index_of_unchecked(entity)
                }
                fn flag_all(&mut self) {}
                unsafe fn flag(&self, _: EntityId) {}
            }
        )+
    }
}

window![Window<'w, T>; &Window<'w, T>];

macro_rules! window_mut {
    ($($window_mut: ty);+) => {
        $(
            impl<'w, T> AbstractMut for $window_mut {
                type Out = &'w mut T;
                type Slice = &'w mut [T];
                unsafe fn get_data(&self, index: usize) -> Self::Out {
                    self.get_at_unbounded(index)
                }
                unsafe fn get_update_data(&self, index: usize) -> Self::Out {
                    self.swap_with_last_non_modified(index)
                }
                unsafe fn get_data_slice(&self, indices: core::ops::Range<usize>) -> Self::Slice {
                    self.get_at_unbounded_slice(indices)
                }
                fn dense(&self) -> *const EntityId {
                    <RawWindowMut<'_, T>>::dense(self)
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    <RawWindowMut<'_, T>>::id_at(self, index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    <RawWindowMut<'_, T>>::index_of(self, entity)
                }
                unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
                    <RawWindowMut<'_, T>>::index_of_unchecked_0(self, entity)
                }
                fn flag_all(&mut self) {
                    <RawWindowMut<'_, T>>::flag_all(self)
                }
                unsafe fn flag(&self, entity: EntityId) {
                    <RawWindowMut<'_, T>>::flag(self, entity)
                }
            }
        )+
    }
}

window_mut![RawWindowMut<'w, T>; &mut RawWindowMut<'w, T>];

macro_rules! not_window {
    ($($not_window: ty);+) => {
        $(
            impl<'w, T> AbstractMut for $not_window {
                type Out = ();
                type Slice = ();
                unsafe fn get_data(&self, index: usize) -> Self::Out {
                    if self.0.contains_index(index) {
                        unreachable!()
                    }
                }
                unsafe fn get_update_data(&self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn get_data_slice(&self, _: core::ops::Range<usize>) -> Self::Slice {
                    unreachable!()
                }
                fn dense(&self) -> *const EntityId {
                    unreachable!()
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    <Window<'_, T>>::id_at(&self.0, index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    if self.0.contains(entity) {
                        None
                    } else {
                        Some(core::usize::MAX)
                    }
                }
                unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
                    core::usize::MAX
                }
                fn flag_all(&mut self) {}
                unsafe fn flag(&self, _: EntityId) {}
            }
        )+
    }
}

not_window![Not<Window<'w, T>>; Not<&Window<'w, T>>];

macro_rules! not_window_mut {
    ($($not_window_mut: ty);+) => {
        $(
            impl<'w, T> AbstractMut for $not_window_mut {
                type Out = ();
                type Slice = ();
                unsafe fn get_data(&self, index: usize) -> Self::Out {
                    if self.0.contains_index(index) {
                        unreachable!()
                    }
                }
                unsafe fn get_update_data(&self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn get_data_slice(&self, _: core::ops::Range<usize>) -> Self::Slice {
                    unreachable!()
                }
                fn dense(&self) -> *const EntityId {
                    unreachable!()
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    <RawWindowMut<'_, T>>::id_at(&self.0, index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    if self.0.contains(entity) {
                        None
                    } else {
                        Some(core::usize::MAX)
                    }
                }
                unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
                    core::usize::MAX
                }
                fn flag_all(&mut self) {}
                unsafe fn flag(&self, _: EntityId) {}
            }
        )+
    }
}

not_window_mut![Not<RawWindowMut<'w, T>>; Not<&mut RawWindowMut<'w, T>>];
