mod sort;

pub use sort::WindowSort1;

use super::SparseSet;
use super::{Pack, PackInfo};
use crate::error;
use crate::EntityId;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use core::ptr;

/// Shared slice of a storage.
pub struct Window<'a, T> {
    sparse: &'a [Option<Box<[usize; super::BUCKET_SIZE]>>],
    dense: &'a [EntityId],
    data: &'a [T],
    pack_info: &'a PackInfo<T>,
    offset: usize,
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

impl<'w, T> Window<'w, T> {
    pub(crate) fn new(sparse_set: &'w SparseSet<T>, range: core::ops::Range<usize>) -> Self {
        Window {
            offset: range.start,
            sparse: &sparse_set.sparse,
            dense: &sparse_set.dense[range.clone()],
            data: &sparse_set.data[range],
            pack_info: &sparse_set.pack_info,
        }
    }
    /// Returns true if the window contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }
    pub(crate) fn contains_index(&self, index: usize) -> bool {
        index >= self.offset && index < self.offset + self.len()
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
            // SAFE we checked for OOB
            unsafe {
                Some(
                    self.data.get_unchecked(
                        *self
                            .sparse
                            .get_unchecked(entity.bucket())
                            .as_ref()
                            .unwrap()
                            .get_unchecked(entity.bucket_index() - self.offset),
                    ),
                )
            }
        } else {
            None
        }
    }
    /// # Safety
    ///
    /// `index` has to be between 0 and self.len()
    pub(crate) unsafe fn get_at_unbounded_0(&self, index: usize) -> &'w T {
        self.data.get_unchecked(index)
    }
    /// # Safety
    ///
    /// `range` has to be between 0 and self.len()
    pub(crate) unsafe fn get_at_unbounded_slice_0(
        &self,
        range: core::ops::Range<usize>,
    ) -> &'w [T] {
        core::slice::from_raw_parts(
            self.data.get_unchecked(range.start),
            range.end - range.start,
        )
    }
    pub(crate) fn dense_ptr(&self) -> *const EntityId {
        self.dense.as_ptr()
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
    /// Returns the `EntityId` at a given `index`.
    pub fn try_id_at(&self, index: usize) -> Option<EntityId> {
        self.dense.get(index).copied()
    }
    /// Returns the `EntityId` at a given `index`.  
    /// Unwraps errors.
    pub fn id_at(&self, index: usize) -> EntityId {
        self.try_id_at(index).unwrap()
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.
    /// This index is only valid for this window.
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        if let Some(bucket) = self.sparse.get(entity.bucket()).and_then(Option::as_ref) {
            // SAFE bucket_index is always is bound
            let index = unsafe { *bucket.get_unchecked(entity.bucket_index()) };
            if index >= self.offset && index < self.offset + self.dense.len() {
                Some(index - self.offset)
            } else {
                None
            }
        } else {
            None
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        if let Some(bucket) = self.sparse.get_unchecked(entity.bucket()) {
            *bucket.get_unchecked(entity.bucket_index()) - self.offset
        } else {
            core::hint::unreachable_unchecked()
        }
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
    pub(crate) fn pack_info(&self) -> &PackInfo<T> {
        self.pack_info
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
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
    sparse: &'w mut [Option<Box<[usize; super::BUCKET_SIZE]>>],
    dense: &'w mut [EntityId],
    data: &'w mut [T],
    pack_info: &'w mut PackInfo<T>,
    offset: usize,
}

impl<'w, T> WindowMut<'w, T> {
    pub(crate) fn new(sparse_set: &'w mut SparseSet<T>, range: core::ops::Range<usize>) -> Self {
        WindowMut {
            sparse: &mut sparse_set.sparse,
            offset: range.start,
            dense: &mut sparse_set.dense[range.clone()],
            data: &mut sparse_set.data[range],
            pack_info: &mut sparse_set.pack_info,
        }
    }
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
        let sparse_len = self.sparse.len();
        let sparse: *mut Option<Box<[usize; super::BUCKET_SIZE]>> = self.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut usize;

        RawWindowMut {
            sparse,
            sparse_len,
            dense: self.dense.as_mut_ptr(),
            dense_len: self.dense.len(),
            data: self.data.as_mut_ptr(),
            pack_info: self.pack_info,
            offset: self.offset,
            _phantom: PhantomData,
        }
    }
    pub(crate) fn into_raw(self) -> RawWindowMut<'w, T> {
        let sparse_len = self.sparse.len();
        let sparse: *mut Option<Box<[usize; super::BUCKET_SIZE]>> = self.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut usize;

        RawWindowMut {
            sparse,
            sparse_len,
            dense: self.dense.as_mut_ptr(),
            dense_len: self.dense.len(),
            data: self.data.as_mut_ptr(),
            pack_info: self.pack_info,
            offset: self.offset,
            _phantom: PhantomData,
        }
    }
    /// Returns true if the window contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.as_non_mut().contains(entity)
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        if self.contains(entity) {
            // SAFE we checked for OOB
            unsafe {
                Some(
                    self.data.get_unchecked(
                        *self
                            .sparse
                            .get_unchecked(entity.bucket())
                            .as_ref()
                            .unwrap()
                            .get_unchecked(entity.bucket_index() - self.offset),
                    ),
                )
            }
        } else {
            None
        }
    }
    pub(crate) fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        if self.contains(entity) {
            // SAFE we checked the window countains the entity
            let mut index = unsafe {
                *self
                    .sparse
                    .get_unchecked(entity.bucket())
                    .as_ref()
                    .unwrap()
                    .get_unchecked(entity.bucket_index() - self.offset)
            };
            if let Pack::Update(pack) = &mut self.pack_info.pack {
                if index >= pack.modified - self.offset {
                    // index of the first element non modified
                    let non_mod = pack.inserted + pack.modified - self.offset;
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
                            let id = self.dense.get_unchecked(non_mod);
                            *self
                                .sparse
                                .get_unchecked_mut(id.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(id.bucket_index()) = non_mod + self.offset;
                            let id = *self.dense.get_unchecked(index);
                            *self
                                .sparse
                                .get_unchecked_mut(id.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(id.bucket_index()) = index + self.offset;
                        }
                        pack.modified += 1;
                        index = non_mod + self.offset;
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
                    // SAFE i is in bound
                    unsafe {
                        let dense = *self.dense.get_unchecked(i);
                        // SAFE dense can always index into sparse
                        *self
                            .sparse
                            .get_unchecked_mut(dense.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(dense.bucket_index()) = i;
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
    pub(crate) fn unpack(&mut self, entity: EntityId) {
        if self.contains(entity) {
            // SAFE we checked for OOB
            let dense_index = unsafe {
                *self
                    .sparse
                    .get_unchecked(entity.bucket())
                    .as_ref()
                    .unwrap()
                    .get_unchecked(entity.bucket_index())
            };
            match &mut self.pack_info.pack {
                Pack::Tight(pack) => {
                    if dense_index < pack.len {
                        pack.len -= 1;
                        // swap index and last packed element (can be the same)
                        unsafe {
                            // SAFE PACK;LEN IS VALID
                            let last_pack = *self.dense.get_unchecked(pack.len);
                            // SAFE dense can always index into sparse
                            let mut last_pack_index = *self
                                .sparse
                                .get_unchecked(last_pack.bucket())
                                .as_ref()
                                .unwrap()
                                .get_unchecked(last_pack.bucket_index());
                            core::mem::swap(
                                &mut last_pack_index,
                                // SAFE we checked for OOB
                                self.sparse
                                    .get_unchecked_mut(entity.bucket())
                                    .as_mut()
                                    .unwrap()
                                    .get_unchecked_mut(entity.bucket_index()),
                            );
                            // SAFE dense can always index into sparse
                            *self
                                .sparse
                                .get_unchecked_mut(last_pack.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(last_pack.bucket_index()) = last_pack_index;
                        }
                    }
                    self.dense.swap(dense_index, pack.len);
                    self.data.swap(dense_index, pack.len);
                }
                Pack::Loose(pack) => {
                    if dense_index < pack.len {
                        pack.len -= 1;
                        // swap index and last packed element (can be the same)
                        unsafe {
                            // SAFE pack.len is valid
                            let last_pack = *self.dense.get_unchecked(pack.len);
                            // SAFE dense can always index into sparse
                            let mut last_pack_index = *self
                                .sparse
                                .get_unchecked(last_pack.bucket())
                                .as_ref()
                                .unwrap()
                                .get_unchecked(last_pack.bucket_index());
                            core::mem::swap(
                                &mut last_pack_index,
                                // SAFE we checked for OOB
                                self.sparse
                                    .get_unchecked_mut(entity.bucket())
                                    .as_mut()
                                    .unwrap()
                                    .get_unchecked_mut(entity.bucket_index()),
                            );
                            // SAFE dense can always index into sparse
                            *self
                                .sparse
                                .get_unchecked_mut(last_pack.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(last_pack.bucket_index()) = last_pack_index;
                        }
                        self.dense.swap(dense_index, pack.len);
                        self.data.swap(dense_index, pack.len);
                    }
                }
                Pack::Update(_) => {}
                Pack::NoPack => {}
            }
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
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.
    /// This index is only valid for this window and until a modification happens.
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        self.as_non_mut().index_of(entity)
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        self.as_non_mut().index_of_unchecked(entity)
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
    pub(crate) fn pack_info(&self) -> &PackInfo<T> {
        self.pack_info
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
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
    sparse: *mut *mut usize,
    sparse_len: usize,
    dense: *mut EntityId,
    dense_len: usize,
    data: *mut T,
    pack_info: *mut PackInfo<T>,
    offset: usize,
    _phantom: PhantomData<&'a mut T>,
}

unsafe impl<T: Send> Send for RawWindowMut<'_, T> {}

impl<'w, T> RawWindowMut<'w, T> {
    pub(crate) fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }
    pub(crate) fn contains_index(&self, index: usize) -> bool {
        index >= self.offset && index < self.offset + self.dense_len
    }
    pub(crate) fn index_of(&self, entity: EntityId) -> Option<usize> {
        if entity.bucket() < self.sparse_len {
            // SAFE we checked for OOB
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };
            if !bucket.is_null() {
                // SAFE we checked for null and bucket_index is always in bound
                let index = unsafe { ptr::read(bucket.add(entity.bucket_index())) };
                if index >= self.offset && index < self.offset + self.dense_len {
                    Some(index - self.offset)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub(crate) unsafe fn index_of_unchecked_0(&self, entity: EntityId) -> usize {
        let bucket = ptr::read(self.sparse.add(entity.bucket()));
        ptr::read(bucket.add(entity.bucket_index()))
    }
    /// Returns the component at `index`.
    /// # Safety
    ///
    /// `index` has to be in the interval `[0, self.dense_len)`.
    pub(crate) unsafe fn get_at_unbounded(&self, index: usize) -> &'w mut T {
        &mut *self.data.add(index)
    }
    /// # Safety
    ///
    /// `range` has to be in the interval `[0, self.dense_len)`.
    pub(crate) unsafe fn get_at_unbounded_slice(
        &self,
        range: core::ops::Range<usize>,
    ) -> &'w mut [T] {
        core::slice::from_raw_parts_mut(self.data.add(range.start), range.len())
    }
    pub(crate) fn dense(&self) -> *const EntityId {
        self.dense
    }
    /// # Safety
    ///
    /// `index` has to be in the interval `[0, self.dense_len)`.
    pub(crate) unsafe fn id_at(&self, index: usize) -> EntityId {
        ptr::read(self.dense.add(index))
    }
    /// # Safety
    ///
    /// This method can only be called once at a time.  
    /// `entity` must own a component in this storage.  
    /// No borrow must be in progress on `entity` nor `first_non_mod`.
    pub(crate) unsafe fn flag(&self, entity: EntityId) {
        if let Pack::Update(pack) = &mut (*self.pack_info).pack {
            let first_non_mod = pack.inserted + pack.modified;
            if self.index_of_unchecked_0(entity) >= first_non_mod {
                pack.modified += 1;
            }
        }
    }
    pub(crate) fn flag_all(&mut self) {
        // SAFE we have exclusive access
        if let Pack::Update(pack) = unsafe { &mut (*self.pack_info).pack } {
            if self.offset + self.dense_len > pack.inserted + pack.modified {
                pack.modified = self.offset + self.dense_len - pack.inserted;
            }
        }
    }
    /// # Safety
    ///
    /// `index` has to be between 0 and self.len().  
    /// No other borrow should be in progress on `index`.  
    /// Only one call to this function can happen at a time.
    pub(crate) unsafe fn swap_with_last_non_modified(&self, mut index: usize) -> &'w mut T {
        if let Pack::Update(pack) = &(*self.pack_info).pack {
            let last_non_mut = pack.inserted + pack.modified;
            if self.offset + index >= last_non_mut {
                ptr::swap(
                    self.dense.add(index),
                    self.dense.add(last_non_mut - self.offset),
                );
                ptr::swap(
                    self.data.add(index),
                    self.data.add(last_non_mut - self.offset),
                );
                let entity = ptr::read(self.dense.add(index));
                let bucket = ptr::read(self.sparse.add(entity.bucket()));
                *bucket.add(entity.bucket_index()) = index;
                let entity = ptr::read(self.dense.add(last_non_mut));
                let bucket = ptr::read(self.sparse.add(entity.bucket()));
                *bucket.add(entity.bucket_index()) = last_non_mut;
                index = last_non_mut;
            }
        }
        self.get_at_unbounded(index)
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
            offset: self.offset,
            _phantom: PhantomData,
        }
    }
}
