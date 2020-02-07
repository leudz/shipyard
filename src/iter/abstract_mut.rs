use crate::not::Not;
use crate::sparse_set::{Pack, RawWindowMut, Window};
use crate::storage::EntityId;
use core::ptr;

// Abstracts different types of view to iterate over
// mutable and immutable views with the same iterator
#[doc(hidden)]
pub trait AbstractMut {
    type Out;
    type Slice;
    /// # Safety
    ///
    /// `index` has to be inbound and `Out` need a correct lifetime.
    unsafe fn get_data(&mut self, index: usize) -> Self::Out;
    /// # Safety
    ///
    /// `index` has to be inbound and `Out` need a correct lifetime.
    unsafe fn get_update_data(&mut self, index: usize) -> Self::Out;
    /// # Safety
    ///
    /// `index` has to be inbound and `Slice` need a correct lifetime.
    unsafe fn get_data_slice(&mut self, indices: core::ops::Range<usize>) -> Self::Slice;
    fn dense(&self) -> *const EntityId;
    unsafe fn id_at(&self, index: usize) -> EntityId;
    fn index_of(&self, entity: EntityId) -> Option<usize>;
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize;
    unsafe fn flag_all(&mut self);
    unsafe fn flag(&mut self, entity: EntityId);
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
                unsafe fn get_update_data(&mut self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn get_data_slice(&mut self, indices: core::ops::Range<usize>) -> Self::Slice {
                    core::slice::from_raw_parts(
                        self.data.get_unchecked(indices.start),
                        indices.end - indices.start,
                    )
                }
                fn dense(&self) -> *const EntityId {
                    self.dense.as_ptr()
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
                unsafe fn flag_all(&mut self) {}
                unsafe fn flag(&mut self, _: EntityId) {}
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
                unsafe fn get_update_data(&mut self, mut index: usize) -> Self::Out {
                    if let Pack::Update(pack) = &(*self.pack_info).pack {
                        // index of the first element non modified
                        let non_mod = pack.inserted + pack.modified;
                        if index >= non_mod {
                            ptr::swap(self.dense.add(non_mod), self.dense.add(index));
                            ptr::swap(self.data.add(non_mod), self.data.add(index));

                            let non_mod_index = ptr::read(self.dense.add(non_mod)).index();
                            *self.sparse.add(non_mod_index) = non_mod;

                            let index_index = ptr::read(self.dense.add(index)).index();
                            *self.sparse.add(index_index) = index;

                            index = non_mod;
                        }
                    }
                    &mut *self.data.add(index)
                }
                unsafe fn get_data_slice(&mut self, indices: core::ops::Range<usize>) -> Self::Slice {
                    core::slice::from_raw_parts_mut(
                        self.data.add(indices.start),
                        indices.end - indices.start,
                    )
                }
                fn dense(&self) -> *const EntityId {
                    self.dense
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
                unsafe fn flag_all(&mut self) {
                    if let Pack::Update(update) = &mut (*self.pack_info).pack {
                        if self.dense_len > update.inserted + update.modified {
                            update.modified = self.dense_len - update.inserted;
                        }
                    }
                }
                unsafe fn flag(&mut self, entity: EntityId) {
                    if let Pack::Update(update) = &mut (*self.pack_info).pack {
                        if ptr::read(self.sparse.add(entity.index())) >= update.inserted + update.modified {
                            update.modified += 1;
                        }
                    }
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
                    if index != core::usize::MAX {
                        unreachable!()
                    }
                }
                unsafe fn get_update_data(&mut self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn get_data_slice(&mut self, _: core::ops::Range<usize>) -> Self::Slice {
                    unreachable!()
                }
                fn dense(&self) -> *const EntityId {
                    unreachable!()
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    *self.0.dense.get_unchecked(index)
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
                unsafe fn flag_all(&mut self) {}
                unsafe fn flag(&mut self, _: EntityId) {}
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
                    if index != core::usize::MAX {
                        unreachable!()
                    }
                }
                unsafe fn get_update_data(&mut self, index: usize) -> Self::Out {
                    self.get_data(index)
                }
                unsafe fn get_data_slice(&mut self, _: core::ops::Range<usize>) -> Self::Slice {
                    unreachable!()
                }
                fn dense(&self) -> *const EntityId {
                    unreachable!()
                }
                unsafe fn id_at(&self, index: usize) -> EntityId {
                    *self.0.dense.add(index)
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
                unsafe fn flag_all(&mut self) {}
                unsafe fn flag(&mut self, _: EntityId) {}
            }
        )+
    }
}

not_window_mut![Not<RawWindowMut<'a, T>>; Not<&mut RawWindowMut<'a, T>>];
