use super::{Pack, PackInfo};
use crate::error;
use crate::EntityId;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use core::ptr;

/// Shared slice of a storage.
pub struct Window<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [EntityId],
    pub(crate) data: &'a [T],
    pub(crate) pack_info: &'a PackInfo<T>,
    pub(super) offset: usize,
}

impl<T> Clone for Window<'_, T> {
    fn clone(&self) -> Self {
        Window {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
            offset: self.offset,
        }
    }
}

impl<T> Window<'_, T> {
    /// Returns true if the window contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        entity.uindex() < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(entity.uindex()) } < self.dense.len()
            && unsafe {
                *self
                    .dense
                    .get_unchecked(*self.sparse.get_unchecked(entity.uindex()))
                    == entity
            }
    }
    /// Returns the length of the window.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// Returns true if the window's length is 0.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked(*self.sparse.get_unchecked(entity.uindex()))
            })
        } else {
            None
        }
    }
    /// Returns the *inserted* section of an update packed window.
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset == 0 && self.len() >= pack.inserted {
                Ok(Window {
                    sparse: &self.sparse,
                    dense: &self.dense[0..pack.inserted],
                    data: &self.data[0..pack.inserted],
                    pack_info: &self.pack_info,
                    offset: 0,
                })
            } else {
                Err(error::Inserted::NotInbound)
            }
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window.  
    /// Unwraps errors.
    pub fn inserted(&self) -> Window<'_, T> {
        self.try_inserted().unwrap()
    }
    /// Returns the *modified* section of an update packed window.
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::Modified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset <= pack.inserted && self.len() >= pack.modified {
                Ok(Window {
                    sparse: &self.sparse,
                    dense: &self.dense[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    data: &self.data[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    pack_info: &self.pack_info,
                    offset: (pack.inserted - self.offset),
                })
            } else {
                Err(error::Modified::NotInbound)
            }
        } else {
            Err(error::Modified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    pub fn modified(&self) -> Window<'_, T> {
        self.try_modified().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed window.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::InsertedOrModified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset == 0 && self.len() >= pack.inserted + pack.modified {
                Ok(Window {
                    sparse: &self.sparse,
                    dense: &self.dense[0..pack.inserted + pack.modified],
                    data: &self.data[0..pack.inserted + pack.modified],
                    pack_info: &self.pack_info,
                    offset: 0,
                })
            } else {
                Err(error::InsertedOrModified::NotInbound)
            }
        } else {
            Err(error::InsertedOrModified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        self.try_inserted_or_modified().unwrap()
    }
    /// Returns the *deleted* components of an update packed window.
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *deleted* components of an update packed window.  
    /// Unwraps errors.
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.try_deleted().unwrap()
    }
    pub(crate) fn is_unique(&self) -> bool {
        self.sparse.is_empty() && self.dense.is_empty() && self.data.len() == 1
    }
    /// Returns the `EntityId` at a given `index`.
    pub fn try_id_at(&self, index: usize) -> Option<EntityId> {
        self.dense.get(index).copied()
    }
    /// Returns the `EntityId` at a given `index`.  
    /// Unwraps errors.
    pub fn id_at(&self, index: usize) -> EntityId {
        self.try_id_at(index).unwrap()
    }
    /// Returns a slice of all the components in this window.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    /// Returns a window over `range`.
    pub fn try_as_window<R: core::ops::RangeBounds<usize>>(
        &self,
        range: R,
    ) -> Result<Window<'_, T>, error::NotInbound> {
        use core::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => *start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => *end,
            Bound::Excluded(end) => end.checked_sub(1).unwrap_or(0),
            Bound::Unbounded => self.len(),
        };
        let range = start..end;

        if range.end <= self.len() {
            Ok(Window {
                offset: range.start + self.offset,
                sparse: &self.sparse,
                dense: &self.dense[range.clone()],
                data: &self.data[range],
                pack_info: &self.pack_info,
            })
        } else {
            Err(error::NotInbound::Window)
        }
    }
    /// Returns a window over `range`.  
    /// Unwraps errors.
    pub fn as_window<R: core::ops::RangeBounds<usize>>(&self, range: R) -> Window<'_, T> {
        self.try_as_window(range).unwrap()
    }
}

impl<T> Index<EntityId> for Window<'_, T> {
    type Output = T;
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

/// Exclusive slice of a storage.
pub struct WindowMut<'w, T> {
    pub(crate) sparse: &'w mut [usize],
    pub(crate) dense: &'w mut [EntityId],
    pub(crate) data: &'w mut [T],
    pub(crate) pack_info: &'w mut PackInfo<T>,
    pub(super) offset: usize,
}

impl<'w, T> WindowMut<'w, T> {
    pub(crate) fn as_non_mut(&self) -> Window<'_, T> {
        Window {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
            offset: self.offset,
        }
    }
    pub(crate) fn as_raw(&mut self) -> RawWindowMut<'_, T> {
        RawWindowMut {
            sparse: self.sparse.as_mut_ptr(),
            sparse_len: self.sparse.len(),
            dense: self.dense.as_mut_ptr(),
            dense_len: self.dense.len(),
            data: self.data.as_mut_ptr(),
            pack_info: self.pack_info,
            _phantom: PhantomData,
        }
    }
    pub(crate) fn into_raw(self) -> RawWindowMut<'w, T> {
        RawWindowMut {
            sparse: self.sparse.as_mut_ptr(),
            sparse_len: self.sparse.len(),
            dense: self.dense.as_mut_ptr(),
            dense_len: self.dense.len(),
            data: self.data.as_mut_ptr(),
            pack_info: self.pack_info,
            _phantom: PhantomData,
        }
    }
    /// Returns true if the window contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.as_non_mut().contains(entity)
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked(*self.sparse.get_unchecked(entity.uindex()))
            })
        } else {
            None
        }
    }
    pub(crate) fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        if self.contains(entity) {
            // SAFE we checked the window countains the entity
            let mut index = unsafe { *self.sparse.get_unchecked(entity.uindex()) };
            if let Pack::Update(pack) = &mut self.pack_info.pack {
                if index >= pack.modified {
                    // index of the first element non modified
                    let non_mod = pack.inserted + pack.modified;
                    if index >= non_mod {
                        // SAFE we checked the window contains the entity
                        unsafe {
                            ptr::swap(
                                self.dense.get_unchecked_mut(non_mod),
                                self.dense.get_unchecked_mut(index),
                            );
                            ptr::swap(
                                self.data.get_unchecked_mut(non_mod),
                                self.data.get_unchecked_mut(index),
                            );
                            *self
                                .sparse
                                .get_unchecked_mut((*self.dense.get_unchecked(non_mod)).uindex()) =
                                non_mod;
                            *self
                                .sparse
                                .get_unchecked_mut((*self.dense.get_unchecked(index)).uindex()) =
                                index;
                        }
                        pack.modified += 1;
                        index = non_mod;
                    }
                }
            }
            Some(unsafe { self.data.get_unchecked_mut(index) })
        } else {
            None
        }
    }
    /// Returns the length of the window.
    pub fn len(&self) -> usize {
        self.as_non_mut().len()
    }
    /// Returns true if the window's length is 0.
    pub fn is_empty(&self) -> bool {
        self.as_non_mut().is_empty()
    }

    /// Returns the *inserted* section of an update packed window.
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset == 0 && self.len() >= pack.inserted {
                Ok(Window {
                    sparse: &self.sparse,
                    dense: &self.dense[0..pack.inserted],
                    data: &self.data[0..pack.inserted],
                    pack_info: &self.pack_info,
                    offset: 0,
                })
            } else {
                Err(error::Inserted::NotInbound)
            }
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window.  
    /// Unwraps errors.
    pub fn inserted(&self) -> Window<'_, T> {
        self.try_inserted().unwrap()
    }
    /// Returns the *inserted* section of an update packed window mutably.
    pub fn try_inserted_mut(&mut self) -> Result<WindowMut<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset == 0 && self.len() >= pack.inserted {
                Ok(WindowMut {
                    sparse: &mut self.sparse,
                    dense: &mut self.dense[0..pack.inserted],
                    data: &mut self.data[0..pack.inserted],
                    pack_info: &mut self.pack_info,
                    offset: 0,
                })
            } else {
                Err(error::Inserted::NotInbound)
            }
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window mutably.  
    /// Unwraps errors.
    pub fn inserted_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_mut().unwrap()
    }
    /// Returns the *modified* section of an update packed window.
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::Modified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset <= pack.inserted && self.len() >= pack.modified {
                Ok(Window {
                    sparse: &self.sparse,
                    dense: &self.dense[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    data: &self.data[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    pack_info: &self.pack_info,
                    offset: (pack.inserted - self.offset),
                })
            } else {
                Err(error::Modified::NotInbound)
            }
        } else {
            Err(error::Modified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    pub fn modified(&self) -> Window<'_, T> {
        self.try_modified().unwrap()
    }
    /// Returns the *modified* section of an update packed window mutably.
    pub fn try_modified_mut(&mut self) -> Result<WindowMut<'_, T>, error::Modified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset <= pack.inserted && self.len() >= pack.modified {
                Ok(WindowMut {
                    sparse: &mut self.sparse,
                    dense: &mut self.dense[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    data: &mut self.data[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    offset: (pack.inserted - self.offset),
                    pack_info: &mut self.pack_info,
                })
            } else {
                Err(error::Modified::NotInbound)
            }
        } else {
            Err(error::Modified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window mutably.  
    /// Unwraps errors.
    pub fn modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_modified_mut().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed window.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::InsertedOrModified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset == 0 && self.len() >= pack.inserted + pack.modified {
                Ok(Window {
                    sparse: &self.sparse,
                    dense: &self.dense[0..pack.inserted + pack.modified],
                    data: &self.data[0..pack.inserted + pack.modified],
                    pack_info: &self.pack_info,
                    offset: 0,
                })
            } else {
                Err(error::InsertedOrModified::NotInbound)
            }
        } else {
            Err(error::InsertedOrModified::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window.  
    /// Unwraps errors.
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        self.try_inserted_or_modified().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed window mutably.
    pub fn try_inserted_or_modified_mut(
        &mut self,
    ) -> Result<WindowMut<'_, T>, error::InsertedOrModified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            if self.offset == 0 && self.len() >= pack.inserted + pack.modified {
                Ok(WindowMut {
                    sparse: &mut self.sparse,
                    dense: &mut self.dense[0..pack.inserted + pack.modified],
                    data: &mut self.data[0..pack.inserted + pack.modified],
                    pack_info: &mut self.pack_info,
                    offset: 0,
                })
            } else {
                Err(error::InsertedOrModified::NotInbound)
            }
        } else {
            Err(error::InsertedOrModified::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window mutably.  
    /// Unwraps errors.
    pub fn inserted_or_modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_or_modified_mut().unwrap()
    }
    /// Returns the *deleted* components of an update packed window.
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *deleted* components of an update packed window.  
    /// Unwraps errors.
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.try_deleted().unwrap()
    }
    /// Takes ownership of the *deleted* components of an update packed window.
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            let mut vec = Vec::with_capacity(pack.deleted.capacity());
            core::mem::swap(&mut vec, &mut pack.deleted);
            Ok(vec)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Takes ownership of the *deleted* components of an update packed window.  
    /// Unwraps errors.
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.try_take_deleted().unwrap()
    }
    /// Moves all component in the *inserted* section of an update packed window to the *neutral* section.
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            if pack.modified == 0 {
                pack.inserted = 0;
            } else {
                let new_len = pack.inserted;
                while pack.inserted > 0 {
                    let new_end =
                        core::cmp::min(pack.inserted + pack.modified - 1, self.dense.len());
                    self.dense.swap(new_end, pack.inserted - 1);
                    self.data.swap(new_end, pack.inserted - 1);
                    pack.inserted -= 1;
                }
                for i in pack.modified.saturating_sub(new_len)..pack.modified + new_len {
                    unsafe {
                        *self
                            .sparse
                            .get_unchecked_mut(self.dense.get_unchecked(i).uindex()) = i;
                    }
                }
            }
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *inserted* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    pub fn clear_inserted(&mut self) {
        self.try_clear_inserted().unwrap()
    }
    /// Moves all component in the *modified* section of an update packed window to the *neutral* section.
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            pack.modified = 0;
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *modified* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    pub fn clear_modified(&mut self) {
        self.try_clear_modified().unwrap()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed window to the *neutral* section.
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            pack.inserted = 0;
            pack.modified = 0;
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    pub fn clear_inserted_and_modified(&mut self) {
        self.try_clear_inserted_and_modified().unwrap()
    }
    pub(crate) fn pack(&mut self, entity: EntityId) {
        if self.contains(entity) {
            let dense_index = self.sparse[entity.uindex()];
            match &mut self.pack_info.pack {
                Pack::Tight(pack) => {
                    if dense_index >= pack.len {
                        self.sparse
                            .swap(self.dense[pack.len].uindex(), entity.uindex());
                        self.dense.swap(pack.len, dense_index);
                        self.data.swap(pack.len, dense_index);
                        pack.len += 1;
                    }
                }
                Pack::Loose(pack) => {
                    if dense_index >= pack.len {
                        self.sparse
                            .swap(self.dense[pack.len].uindex(), entity.uindex());
                        self.dense.swap(pack.len, dense_index);
                        self.data.swap(pack.len, dense_index);
                        pack.len += 1;
                    }
                }
                Pack::Update(_) => {}
                Pack::NoPack => {}
            }
        }
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) {
        let dense_index = unsafe { *self.sparse.get_unchecked(entity.uindex()) };
        match &mut self.pack_info.pack {
            Pack::Tight(pack) => {
                if dense_index < pack.len {
                    pack.len -= 1;
                    // swap index and last packed element (can be the same)
                    unsafe {
                        self.sparse.swap(
                            self.dense.get_unchecked(pack.len).uindex(),
                            self.dense.get_unchecked(dense_index).uindex(),
                        )
                    };
                    self.dense.swap(dense_index, pack.len);
                    self.data.swap(dense_index, pack.len);
                }
            }
            Pack::Loose(pack) => {
                if dense_index < pack.len {
                    pack.len -= 1;
                    // swap index and last packed element (can be the same)
                    unsafe {
                        self.sparse.swap(
                            self.dense.get_unchecked(pack.len).uindex(),
                            self.dense.get_unchecked(dense_index).uindex(),
                        )
                    };
                    self.dense.swap(dense_index, pack.len);
                    self.data.swap(dense_index, pack.len);
                }
            }
            Pack::Update(_) => {}
            Pack::NoPack => {}
        }
    }
    /// Returns the `EntityId` at a given `index`.
    pub fn try_id_at(&self, index: usize) -> Option<EntityId> {
        self.dense.get(index).copied()
    }
    /// Returns the `EntityId` at a given `index`.  
    /// Unwraps errors.
    pub fn id_at(&self, index: usize) -> EntityId {
        self.try_id_at(index).unwrap()
    }
    /// Returns a slice of all the components in this window.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    /// Returns a window over `range`.
    pub fn try_as_window<R: core::ops::RangeBounds<usize>>(
        &self,
        range: R,
    ) -> Result<Window<'_, T>, error::NotInbound> {
        use core::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => *start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => *end + 1,
            Bound::Excluded(end) => *end,
            Bound::Unbounded => self.len(),
        };
        let range = start..end;

        if range.end <= self.len() {
            Ok(Window {
                offset: range.start + self.offset,
                sparse: &self.sparse,
                dense: &self.dense[range.clone()],
                data: &self.data[range],
                pack_info: &self.pack_info,
            })
        } else {
            Err(error::NotInbound::Window)
        }
    }
    /// Returns a window over `range`.  
    /// Unwraps errors.
    pub fn as_window<R: core::ops::RangeBounds<usize>>(&self, range: R) -> Window<'_, T> {
        self.try_as_window(range).unwrap()
    }
    /// Returns a mutable window over `range`.
    pub fn try_as_window_mut<R: core::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> Result<WindowMut<'_, T>, error::NotInbound> {
        use core::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => *start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => *end + 1,
            Bound::Excluded(end) => *end,
            Bound::Unbounded => self.len(),
        };
        let range = start..end;

        if range.end <= self.len() {
            if let Pack::Update(update) = &self.pack_info.pack {
                if !(range.start + self.offset..range.end + self.offset)
                    .contains(&(update.inserted + update.modified))
                {
                    return Err(error::NotInbound::UpdatePack);
                }
            }
            Ok(WindowMut {
                offset: range.start + self.offset,
                sparse: &mut self.sparse,
                dense: &mut self.dense[range.clone()],
                data: &mut self.data[range],
                pack_info: &mut self.pack_info,
            })
        } else {
            Err(error::NotInbound::Window)
        }
    }
    /// Returns a mutable window over `range`.  
    /// Unwraps errors.
    pub fn as_window_mut<R: core::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> WindowMut<'_, T> {
        self.try_as_window_mut(range).unwrap()
    }
}

impl<T> Index<EntityId> for WindowMut<'_, T> {
    type Output = T;
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

impl<T> IndexMut<EntityId> for WindowMut<'_, T> {
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.get_mut(entity).unwrap()
    }
}

pub struct RawWindowMut<'a, T> {
    pub(crate) sparse: *mut usize,
    pub(crate) sparse_len: usize,
    pub(crate) dense: *mut EntityId,
    pub(crate) dense_len: usize,
    pub(crate) data: *mut T,
    pub(crate) pack_info: *mut PackInfo<T>,
    pub(super) _phantom: PhantomData<&'a mut T>,
}

unsafe impl<T: Send> Send for RawWindowMut<'_, T> {}

impl<T> RawWindowMut<'_, T> {
    pub(crate) fn contains(&self, entity: EntityId) -> bool {
        use core::ptr::read;

        entity.uindex() < self.sparse_len
            && unsafe { read(self.sparse.add(entity.uindex())) } < self.dense_len
            && unsafe { read(self.dense.add(read(self.sparse.add(entity.uindex())))) == entity }
    }
}

impl<T> Clone for RawWindowMut<'_, T> {
    fn clone(&self) -> Self {
        RawWindowMut {
            sparse: self.sparse,
            sparse_len: self.sparse_len,
            dense: self.dense,
            dense_len: self.dense_len,
            data: self.data,
            pack_info: self.pack_info,
            _phantom: PhantomData,
        }
    }
}
