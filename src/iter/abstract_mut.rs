use crate::not::Not;
use crate::sparse_set::{Pack, RawWindowMut, Window};
use crate::storage::EntityId;
use std::ptr;

// Abstracts different types of view to iterate over
// mutable and immutable views with the same iterator
#[doc(hidden)]
pub trait AbstractMut {
    type Out;
    type Slice;
    // # Safety
    // The lifetime has to be valid
    unsafe fn get_data(&mut self, index: usize) -> Self::Out;
    // # Safety
    // The lifetime has to be valid
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice;
    fn indices(&self) -> *const EntityId;
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out;
    unsafe fn mark_id_modified(&mut self, entity: EntityId) -> Self::Out {
        let index = self.index_of_unchecked(entity);
        self.mark_modified(index)
    }
    unsafe fn id_at(&self, index: usize) -> EntityId;
    fn index_of(&self, entity: EntityId) -> Option<usize>;
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize;
}

macro_rules! window {
    ($($window: ty);+) => {
        $(
            impl<'a, T> AbstractMut for $window {
                type Out = &'a T;
                type Slice = &'a [T];
                unsafe fn get_data(&mut self, index: usize) -> Self::Out {
                    self.data.get_unchecked(index)
                }
                unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
                    std::slice::from_raw_parts(
                        self.data.get_unchecked(indices.start),
                        indices.end - indices.start,
                    )
                }
                fn indices(&self) -> *const EntityId {
                    self.dense.as_ptr()
                }
                unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    *self.dense.get_unchecked(index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    if self.contains(entity) {
                        Some(unsafe { *self.sparse.get_unchecked(entity.index()) })
                    } else {
                        None
                    }
                }
                unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
                    *self.sparse.get_unchecked(entity.index())
                }
            }
        )+
    }
}

window![Window<'a, T>; &Window<'a, T>];

macro_rules! window_mut {
    ($($window_mut: ty);+) => {
        $(
            impl<'a, T> AbstractMut for $window_mut {
                type Out = &'a mut T;
                type Slice = &'a mut [T];
                unsafe fn get_data(&mut self, index: usize) -> Self::Out {
                    &mut *self.data.add(index)
                }
                unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
                    std::slice::from_raw_parts_mut(
                        self.data.add(indices.start),
                        indices.end - indices.start,
                    )
                }
                fn indices(&self) -> *const EntityId {
                    self.dense
                }
                unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
                    match &mut (*self.pack_info).pack {
                        Pack::Update(pack) => {
                            // index of the first element non modified
                            let non_mod = pack.inserted + pack.modified;
                            if index >= non_mod {
                                let dense_non_mod: *mut _ = self.dense.add(non_mod);
                                let dense_index: *mut _ = self.dense.add(index);
                                ptr::swap(dense_non_mod, dense_index);

                                let data_non_mod: *mut _ = self.data.add(non_mod);
                                let data_index: *mut _ = self.data.add(index);
                                ptr::swap(data_non_mod, data_index);

                                let non_mod_index = (*self.dense.add(non_mod)).index();
                                *self.sparse.add(non_mod_index) = non_mod;

                                let index_index = (*self.dense.add(index)).index();
                                *self.sparse.add(index_index) = index;

                                pack.modified += 1;

                                &mut *self.data.add(non_mod)
                            } else {
                                self.get_data(index)
                            }
                        }
                        _ => self.get_data(index),
                    }
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    *self.dense.add(index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    unsafe {
                        if self.contains(entity) {
                            Some(*self.sparse.add(entity.index()))
                        } else {
                            None
                        }
                    }
                }
                unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
                    *self.sparse.add(entity.index())
                }
            }
        )+
    }
}

window_mut![RawWindowMut<'a, T>; &mut RawWindowMut<'a, T>];

macro_rules! not_window {
    ($($not_window: ty);+) => {
        $(
            impl<'a, T> AbstractMut for $not_window {
                type Out = ();
                type Slice = ();
                unsafe fn get_data(&mut self, index: usize) -> Self::Out {
                    if index != std::usize::MAX {
                        unreachable!()
                    }
                }
                unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
                    unreachable!()
                }
                fn indices(&self) -> *const EntityId {
                    unreachable!()
                }
                unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn mark_id_modified(&mut self, _: EntityId) {}
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    *self.0.dense.get_unchecked(index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    if self.0.contains(entity) {
                        None
                    } else {
                        Some(std::usize::MAX)
                    }
                }
                unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
                    std::usize::MAX
                }
            }
        )+
    }
}

not_window![Not<Window<'a, T>>; Not<&Window<'a, T>>];

macro_rules! not_window_mut {
    ($($not_window_mut: ty);+) => {
        $(
            impl<'a, T> AbstractMut for $not_window_mut {
                type Out = ();
                type Slice = ();
                unsafe fn get_data(&mut self, index: usize) -> Self::Out {
                    if index != std::usize::MAX {
                        unreachable!()
                    }
                }
                unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
                    unreachable!()
                }
                fn indices(&self) -> *const EntityId {
                    unreachable!()
                }
                unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn mark_id_modified(&mut self, _: EntityId) {}
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    *self.0.dense.add(index)
                }
                fn index_of(&self, entity: EntityId) -> Option<usize> {
                    if self.0.contains(entity) {
                        None
                    } else {
                        Some(std::usize::MAX)
                    }
                }
                unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
                    std::usize::MAX
                }
            }
        )+
    }
}

not_window_mut![Not<RawWindowMut<'a, T>>; Not<&mut RawWindowMut<'a, T>>];
