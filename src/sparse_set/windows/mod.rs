mod sort;

pub use sort::WindowSort1;

use super::{Metadata, Pack};
use super::{SparseSet, SparseSlice, SparseSliceMut};
use crate::error;
use crate::EntityId;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use core::ptr;

/// Shared slice of a storage.
pub struct Window<'a, T> {
    sparse: SparseSlice<'a, [usize; super::BUCKET_SIZE]>,
    dense: &'a [EntityId],
    data: &'a [T],
    metadata: &'a Metadata<T>,
    offset: usize,
}

impl<T> Clone for Window<'_, T> {
    fn clone(&self) -> Self {
        Window {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            metadata: self.metadata,
            offset: self.offset,
        }
    }
}

impl<T> Copy for Window<'_, T> {}

impl<'w, T> Window<'w, T> {
    pub(crate) fn new(sparse_set: &'w SparseSet<T>, range: core::ops::Range<usize>) -> Self {
        Window {
            offset: range.start,
            sparse: sparse_set.sparse.as_slice(),
            dense: &sparse_set.dense[range.clone()],
            data: &sparse_set.data[range],
            metadata: &sparse_set.metadata,
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
        unsafe {
            self.index_of(entity)
                .map(|index| self.data.get_unchecked(index))
        }
    }
    /// # Safety
    ///
    /// `index` has to be between 0 and self.len()
    pub(crate) unsafe fn get_at_unbounded(&self, index: usize) -> &'w T {
        self.data.get_unchecked(index)
    }
    /// # Safety
    ///
    /// `range` has to be between 0 and self.len()
    pub(crate) unsafe fn get_at_unbounded_slice(&self, range: core::ops::Range<usize>) -> &'w [T] {
        core::slice::from_raw_parts(
            self.data.get_unchecked(range.start),
            range.end - range.start,
        )
    }
    pub(crate) fn dense_ptr(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
    /// Returns the *inserted* section of an update packed window.
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset == 0 && self.len() >= pack.inserted {
                Ok(Window {
                    sparse: self.sparse,
                    dense: &self.dense[0..pack.inserted],
                    data: &self.data[0..pack.inserted],
                    metadata: &self.metadata,
                    offset: 0,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn inserted(&self) -> Window<'_, T> {
        match self.try_inserted() {
            Ok(inserted) => inserted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *modified* section of an update packed window.
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset <= pack.inserted && self.len() >= pack.modified {
                Ok(Window {
                    sparse: self.sparse,
                    dense: &self.dense[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    data: &self.data[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    metadata: &self.metadata,
                    offset: (pack.inserted - self.offset),
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn modified(&self) -> Window<'_, T> {
        match self.try_modified() {
            Ok(modified) => modified,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset == 0 && self.len() >= pack.inserted + pack.modified {
                Ok(Window {
                    sparse: self.sparse,
                    dense: &self.dense[0..pack.inserted + pack.modified],
                    data: &self.data[0..pack.inserted + pack.modified],
                    metadata: &self.metadata,
                    offset: 0,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        match self.try_inserted_or_modified() {
            Ok(inserted_or_modified) => inserted_or_modified,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *deleted* components of an update packed window.
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *deleted* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        match self.try_deleted() {
            Ok(deleted) => deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the ids of *removed* components of an update packed window.
    pub fn try_removed(&self) -> Result<&[EntityId], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(&pack.removed)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the ids of *removed* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn removed(&self) -> &[EntityId] {
        match self.try_removed() {
            Ok(removed) => removed,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the `EntityId` at a given `index`.
    pub fn try_id_at(&self, index: usize) -> Option<EntityId> {
        self.dense.get(index).copied()
    }
    /// Returns the `EntityId` at a given `index`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn id_at(&self, index: usize) -> EntityId {
        match self.try_id_at(index) {
            Some(id) => id,
            None => panic!(
                "Window contains {} components but trying to access the id at index {}.",
                self.len(),
                index
            ),
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.
    /// This index is only valid for this window.
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        self.index_of_owned(entity)
            .or_else(|| match self.sparse.sparse_index(entity) {
                Some(gen) if gen as u64 == entity.gen() => self
                    .metadata
                    .shared
                    .shared_index(entity)
                    .and_then(|id| self.index_of(id)),
                _ => None,
            })
    }
    pub fn index_of_owned(&self, entity: EntityId) -> Option<usize> {
        self.sparse.sparse_index(entity).and_then(|dense_index| {
            if dense_index != core::usize::MAX
                && self.contains_index(dense_index)
                && self
                    .dense
                    .get(dense_index - self.offset)
                    .map_or(false, |&dense| dense == entity)
            {
                Some(dense_index - self.offset)
            } else {
                None
            }
        })
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub unsafe fn index_of_owned_unchecked(&self, entity: EntityId) -> usize {
        match self.sparse.sparse_index(entity) {
            Some(dense_index) => dense_index - self.offset,
            None => core::hint::unreachable_unchecked(),
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
                sparse: self.sparse,
                dense: &self.dense[range.clone()],
                data: &self.data[range],
                metadata: &self.metadata,
            })
        } else {
            Err(error::NotInbound::Window)
        }
    }
    /// Returns a window over `range`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn as_window<R: core::ops::RangeBounds<usize>>(&self, range: R) -> Window<'_, T> {
        match self.try_as_window(range) {
            Ok(window) => window,
            Err(err) => panic!("{:?}", err),
        }
    }
    pub(crate) fn metadata(&self) -> &Metadata<T> {
        self.metadata
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
}

impl<T> Index<EntityId> for Window<'_, T> {
    type Output = T;
    #[track_caller]
    fn index(&self, entity: EntityId) -> &Self::Output {
        match self.get(entity) {
            Some(component) => component,
            None => panic!("Window does not contain entity {:?}.", entity),
        }
    }
}

/// Exclusive slice of a storage.
pub struct WindowMut<'w, T> {
    sparse: SparseSliceMut<'w, [usize; super::BUCKET_SIZE]>,
    dense: &'w mut [EntityId],
    data: &'w mut [T],
    metadata: &'w mut Metadata<T>,
    offset: usize,
}

impl<'w, T> WindowMut<'w, T> {
    pub(crate) fn new(sparse_set: &'w mut SparseSet<T>, range: core::ops::Range<usize>) -> Self {
        WindowMut {
            sparse: sparse_set.sparse.as_slice_mut(),
            offset: range.start,
            dense: &mut sparse_set.dense[range.clone()],
            data: &mut sparse_set.data[range],
            metadata: &mut sparse_set.metadata,
        }
    }
    pub(crate) fn as_non_mut(&self) -> Window<'_, T> {
        Window {
            sparse: self.sparse.as_non_mut(),
            dense: self.dense,
            data: self.data,
            metadata: self.metadata,
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
            metadata: self.metadata,
            offset: self.offset,
            _phantom: PhantomData,
        }
    }
    pub(crate) fn into_raw(mut self) -> RawWindowMut<'w, T> {
        let sparse_len = self.sparse.len();
        let sparse: *mut Option<Box<[usize; super::BUCKET_SIZE]>> = self.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut usize;

        RawWindowMut {
            sparse,
            sparse_len,
            dense: self.dense.as_mut_ptr(),
            dense_len: self.dense.len(),
            data: self.data.as_mut_ptr(),
            metadata: self.metadata,
            offset: self.offset,
            _phantom: PhantomData,
        }
    }
    /// Returns true if the window contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.as_non_mut().contains(entity)
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        unsafe {
            self.index_of(entity)
                .map(|index| self.data.get_unchecked(index))
        }
    }
    pub(crate) fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        match self.index_of(entity) {
            Some(mut index) => {
                if let Pack::Update(pack) = &mut self.metadata.pack {
                    // index of the first element non modified
                    let non_mod = pack.inserted + pack.modified;

                    if index >= non_mod {
                        unsafe {
                            // SAFE we checked the storage contains the entity
                            ptr::swap(
                                self.dense.get_unchecked_mut(non_mod),
                                self.dense.get_unchecked_mut(index),
                            );

                            ptr::swap(
                                self.data.get_unchecked_mut(non_mod),
                                self.data.get_unchecked_mut(index),
                            );

                            let dense = *self.dense.get_unchecked(non_mod);
                            self.sparse.set_sparse_index_unchecked(dense, non_mod);

                            let dense = *self.dense.get_unchecked(index);
                            self.sparse.set_sparse_index_unchecked(dense, index);
                        }

                        pack.modified += 1;
                        index = non_mod;
                    }
                }

                Some(unsafe { self.data.get_unchecked_mut(index) })
            }
            None => None,
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
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset == 0 && self.len() >= pack.inserted {
                Ok(Window {
                    sparse: self.sparse.as_non_mut(),
                    dense: &self.dense[0..pack.inserted],
                    data: &self.data[0..pack.inserted],
                    metadata: &self.metadata,
                    offset: 0,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn inserted(&self) -> Window<'_, T> {
        match self.try_inserted() {
            Ok(inserted) => inserted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *inserted* section of an update packed window mutably.
    pub fn try_inserted_mut(&mut self) -> Result<WindowMut<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset == 0 && self.len() >= pack.inserted {
                Ok(WindowMut {
                    sparse: self.sparse.reborrow(),
                    dense: &mut self.dense[0..pack.inserted],
                    data: &mut self.data[0..pack.inserted],
                    metadata: &mut self.metadata,
                    offset: 0,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn inserted_mut(&mut self) -> WindowMut<'_, T> {
        match self.try_inserted_mut() {
            Ok(inserted) => inserted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *modified* section of an update packed window.
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset <= pack.inserted && self.len() >= pack.modified {
                Ok(Window {
                    sparse: self.sparse.as_non_mut(),
                    dense: &self.dense[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    data: &self.data[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    metadata: &self.metadata,
                    offset: (pack.inserted - self.offset),
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn modified(&self) -> Window<'_, T> {
        match self.try_modified() {
            Ok(modified) => modified,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *modified* section of an update packed window mutably.
    pub fn try_modified_mut(&mut self) -> Result<WindowMut<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset <= pack.inserted && self.len() >= pack.modified {
                Ok(WindowMut {
                    sparse: self.sparse.reborrow(),
                    dense: &mut self.dense[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    data: &mut self.data[(pack.inserted - self.offset)
                        ..(pack.inserted + pack.modified - self.offset)],
                    offset: (pack.inserted - self.offset),
                    metadata: &mut self.metadata,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn modified_mut(&mut self) -> WindowMut<'_, T> {
        match self.try_modified_mut() {
            Ok(modified) => modified,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset == 0 && self.len() >= pack.inserted + pack.modified {
                Ok(Window {
                    sparse: self.sparse.as_non_mut(),
                    dense: &self.dense[0..pack.inserted + pack.modified],
                    data: &self.data[0..pack.inserted + pack.modified],
                    metadata: &self.metadata,
                    offset: 0,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        match self.try_inserted_or_modified() {
            Ok(inserted_or_modified) => inserted_or_modified,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window mutably.
    pub fn try_inserted_or_modified_mut(
        &mut self,
    ) -> Result<WindowMut<'_, T>, error::UpdateWindow> {
        if let Pack::Update(pack) = &self.metadata.pack {
            if self.offset == 0 && self.len() >= pack.inserted + pack.modified {
                Ok(WindowMut {
                    sparse: self.sparse.reborrow(),
                    dense: &mut self.dense[0..pack.inserted + pack.modified],
                    data: &mut self.data[0..pack.inserted + pack.modified],
                    metadata: &mut self.metadata,
                    offset: 0,
                })
            } else {
                Err(error::UpdateWindow::OutOfBounds)
            }
        } else {
            Err(error::UpdateWindow::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn inserted_or_modified_mut(&mut self) -> WindowMut<'_, T> {
        match self.try_inserted_or_modified_mut() {
            Ok(inserted_or_modified) => inserted_or_modified,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the *deleted* components of an update packed window.
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *deleted* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        match self.try_deleted() {
            Ok(deleted) => deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the ids of *removed* components of an update packed window.
    pub fn try_removed(&self) -> Result<&[EntityId], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(&pack.removed)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the ids of *removed* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn removed(&self) -> &[EntityId] {
        match self.try_removed() {
            Ok(removed) => removed,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Takes ownership of the *deleted* components of an update packed window.
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.metadata.pack {
            let mut vec = Vec::with_capacity(pack.deleted.capacity());
            core::mem::swap(&mut vec, &mut pack.deleted);
            Ok(vec)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Takes ownership of the *deleted* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        match self.try_take_deleted() {
            Ok(deleted) => deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Takes ownership of the ids of *removed* components of an update packed window.
    pub fn try_take_removed(&mut self) -> Result<Vec<EntityId>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.metadata.pack {
            let mut vec = Vec::with_capacity(pack.removed.capacity());
            core::mem::swap(&mut vec, &mut pack.removed);
            Ok(vec)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Takes ownership of the ids of *removed* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        match self.try_take_removed() {
            Ok(removed) => removed,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all component in the *inserted* section of an update packed window to the *neutral* section.
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.metadata.pack {
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
                        self.sparse.set_sparse_index_unchecked(dense, i);
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
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn clear_inserted(&mut self) {
        match self.try_clear_inserted() {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all component in the *modified* section of an update packed window to the *neutral* section.
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.metadata.pack {
            pack.modified = 0;
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *modified* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn clear_modified(&mut self) {
        match self.try_clear_modified() {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed window to the *neutral* section.
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.metadata.pack {
            pack.inserted = 0;
            pack.modified = 0;
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn clear_inserted_and_modified(&mut self) {
        match self.try_clear_inserted_and_modified() {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) {
        if self.contains(entity) {
            // SAFE we checked for OOB
            let dense_index = unsafe { self.index_of_owned_unchecked(entity) };
            match &mut self.metadata.pack {
                Pack::Tight(pack) => {
                    if dense_index < pack.len - self.offset {
                        pack.len -= 1;

                        self.dense.swap(dense_index, pack.len);
                        self.data.swap(dense_index, pack.len);

                        // swap index and last packed element (can be the same)
                        unsafe {
                            let last_pack = *self.dense.get_unchecked(dense_index);

                            self.sparse.set_sparse_index_unchecked(entity, pack.len);
                            self.sparse
                                .set_sparse_index_unchecked(last_pack, dense_index);
                        }
                    }
                }
                Pack::Loose(pack) => {
                    if dense_index < pack.len {
                        pack.len -= 1;

                        self.dense.swap(dense_index, pack.len);
                        self.data.swap(dense_index, pack.len);

                        // swap index and last packed element (can be the same)
                        unsafe {
                            // SAFE pack.len is valid
                            let last_pack = *self.dense.get_unchecked(pack.len);

                            self.sparse.set_sparse_index_unchecked(
                                entity,
                                self.sparse.sparse_index_unchecked(entity),
                            );
                            self.sparse.set_sparse_index_unchecked(
                                last_pack,
                                self.sparse.sparse_index_unchecked(last_pack),
                            );
                        }
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
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn id_at(&self, index: usize) -> EntityId {
        match self.try_id_at(index) {
            Some(id) => id,
            None => panic!(
                "Window contains {} components but trying to access the id at index {}.",
                self.len(),
                index
            ),
        }
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
    pub unsafe fn index_of_owned_unchecked(&self, entity: EntityId) -> usize {
        self.as_non_mut().index_of_owned_unchecked(entity)
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
                sparse: self.sparse.as_non_mut(),
                dense: &self.dense[range.clone()],
                data: &self.data[range],
                metadata: &self.metadata,
            })
        } else {
            Err(error::NotInbound::Window)
        }
    }
    /// Returns a window over `range`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn as_window<R: core::ops::RangeBounds<usize>>(&self, range: R) -> Window<'_, T> {
        match self.try_as_window(range) {
            Ok(window) => window,
            Err(err) => panic!("{:?}", err),
        }
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
            if let Pack::Update(update) = &self.metadata.pack {
                if !(range.start + self.offset..range.end + self.offset)
                    .contains(&(update.inserted + update.modified))
                {
                    return Err(error::NotInbound::UpdatePack);
                }
            }
            Ok(WindowMut {
                offset: range.start + self.offset,
                sparse: self.sparse.reborrow(),
                dense: &mut self.dense[range.clone()],
                data: &mut self.data[range],
                metadata: &mut self.metadata,
            })
        } else {
            Err(error::NotInbound::Window)
        }
    }
    /// Returns a mutable window over `range`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn as_window_mut<R: core::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> WindowMut<'_, T> {
        match self.try_as_window_mut(range) {
            Ok(window) => window,
            Err(err) => panic!("{:?}", err),
        }
    }
    pub(crate) fn metadata(&self) -> &Metadata<T> {
        self.metadata
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
}

impl<T> Index<EntityId> for WindowMut<'_, T> {
    type Output = T;
    #[track_caller]
    fn index(&self, entity: EntityId) -> &Self::Output {
        match self.get(entity) {
            Some(component) => component,
            None => panic!("Window does not contain entity {:?}.", entity),
        }
    }
}

impl<T> IndexMut<EntityId> for WindowMut<'_, T> {
    #[track_caller]
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        match self.get_mut(entity) {
            Some(component) => component,
            None => panic!("Window does not contain entity {:?}.", entity),
        }
    }
}

pub struct RawWindowMut<'a, T> {
    sparse: *mut *mut usize,
    sparse_len: usize,
    dense: *mut EntityId,
    dense_len: usize,
    data: *mut T,
    metadata: *mut Metadata<T>,
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
        match self.sparse_index(entity) {
            Some(dense_index)
                if dense_index != core::usize::MAX
                    && dense_index >= self.offset
                    && dense_index < self.offset + self.dense_len
                    && (dense_index - self.offset < self.dense_len
                        && unsafe { ptr::read(self.dense.add(dense_index - self.offset)) }
                            == entity) =>
            {
                Some(dense_index - self.offset)
            }
            _ => None,
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub(crate) unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        if let Some(index) = self.index_of(entity) {
            index
        } else {
            unreachable!()
        }
    }
    fn sparse_index(&self, entity: EntityId) -> Option<usize> {
        if entity.bucket() < self.sparse_len {
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };
            if !bucket.is_null() {
                Some(unsafe { ptr::read(bucket.add(entity.bucket_index())) })
            } else {
                None
            }
        } else {
            None
        }
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
        if let Pack::Update(pack) = &mut (*self.metadata).pack {
            let first_non_mod = pack.inserted + pack.modified;
            if self.index_of_unchecked(entity) >= first_non_mod {
                pack.modified += 1;
            }
        }
    }
    pub(crate) fn flag_all(&mut self) {
        // SAFE we have exclusive access
        if let Pack::Update(pack) = unsafe { &mut (*self.metadata).pack } {
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
        if let Pack::Update(pack) = &(*self.metadata).pack {
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
                (*bucket.add(entity.bucket_index())) = index;
                let entity = ptr::read(self.dense.add(last_non_mut));
                let bucket = ptr::read(self.sparse.add(entity.bucket()));
                (*bucket.add(entity.bucket_index())) = last_non_mut;
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
            metadata: self.metadata,
            offset: self.offset,
            _phantom: PhantomData,
        }
    }
}
