mod add_component;
mod contains;
mod pack_info;
pub mod sort;
mod view_add_entity;
mod windows;

pub use add_component::AddComponentUnchecked;
pub use contains::Contains;
pub use windows::{Window, WindowMut, WindowSort1};

pub(crate) use pack_info::{LoosePack, Pack, PackInfo, TightPack, UpdatePack};
pub(crate) use view_add_entity::ViewAddEntity;
pub(crate) use windows::RawWindowMut;

use crate::error;
use crate::storage::EntityId;
use crate::unknown_storage::UnknownStorage;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::{type_name, Any, TypeId};
use core::ptr;

pub(crate) const BUCKET_SIZE: usize = 128 / core::mem::size_of::<usize>();

/// Component storage.
// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.
pub struct SparseSet<T> {
    pub(crate) sparse: Vec<Option<Box<[usize; BUCKET_SIZE]>>>,
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) pack_info: PackInfo<T>,
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            pack_info: Default::default(),
        }
    }
    pub(crate) fn window(&self) -> Window<'_, T> {
        Window::new(self, 0..self.len())
    }
    pub(crate) fn window_mut(&mut self) -> WindowMut<'_, T> {
        WindowMut::new(self, 0..self.len())
    }
    pub(crate) fn raw_window_mut(&mut self) -> RawWindowMut<'_, T> {
        self.window_mut().into_raw()
    }
    pub(crate) fn allocate_at(&mut self, entity: EntityId) {
        if entity.bucket() >= self.sparse.len() {
            self.sparse.resize(entity.bucket() + 1, None);
        }
        unsafe {
            // SAFE we just allocated at least entity.bucket()
            if self.sparse.get_unchecked(entity.bucket()).is_none() {
                *self.sparse.get_unchecked_mut(entity.bucket()) =
                    Some(Box::new([core::usize::MAX; BUCKET_SIZE]));
            }
        }
    }
    pub(crate) fn insert(&mut self, mut value: T, entity: EntityId) -> Option<T> {
        self.allocate_at(entity);

        // SAFE entity.bucket() exists and contains at least bucket_index elements
        let (result, mut index) = match unsafe {
            self.sparse
                .get_unchecked_mut(entity.bucket())
                .as_mut()
                .unwrap()
                .get_unchecked_mut(entity.bucket_index())
        } {
            i if *i == core::usize::MAX => {
                *i = self.dense.len();
                self.dense.push(entity);
                self.data.push(value);
                (None, self.dense.len() - 1)
            }
            &mut i => {
                // SAFE sparse index are always valid
                unsafe {
                    core::mem::swap(self.data.get_unchecked_mut(i), &mut value);
                }
                (Some(value), i)
            }
        };

        if let Pack::Update(pack) = &mut self.pack_info.pack {
            if result.is_none() {
                self.dense.swap(pack.inserted + pack.modified, index);
                self.data.swap(pack.inserted + pack.modified, index);

                let entity = self.dense[index];
                // SAFE entity.bucket() exists and contains at least bucket_index elements
                unsafe {
                    *self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index()) = index;
                }

                index = pack.inserted + pack.modified;

                self.dense.swap(pack.inserted, index);
                self.data.swap(pack.inserted, index);

                let entity = self.dense[index];
                // SAFE entity.bucket() exists and contains at least bucket_index elements
                unsafe {
                    *self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index()) = index;
                }

                let entity = self.dense[pack.inserted];
                // SAFE entity.bucket() exists and contains at least bucket_index elements
                unsafe {
                    *self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index()) = pack.inserted;
                }

                pack.inserted += 1;
            } else if index >= pack.inserted + pack.modified {
                self.dense.swap(pack.inserted + pack.modified, index);
                self.data.swap(pack.inserted + pack.modified, index);

                let entity = self.dense[index];
                // SAFE entity.bucket() exists and contains at least bucket_index elements
                unsafe {
                    *self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index()) = index;
                }

                pack.modified += 1;
            }
        }

        result
    }
    /// Removes `entity`'s component from this storage.
    pub fn try_remove(&mut self, entity: EntityId) -> Result<Option<T>, error::Remove>
    where
        T: 'static,
    {
        if self.pack_info.observer_types.is_empty() {
            match self.pack_info.pack {
                Pack::Tight(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                Pack::Loose(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                _ => Ok(self.actual_remove(entity)),
            }
        } else {
            Err(error::Remove::MissingPackStorage(type_name::<T>()))
        }
    }
    /// Removes `entity`'s component from this storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn remove(&mut self, entity: EntityId) -> Option<T>
    where
        T: 'static,
    {
        self.try_remove(entity).unwrap()
    }
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<T> {
        if self.contains(entity) {
            // SAFE we're inbound
            let mut dense_index = unsafe {
                *self
                    .sparse
                    .get_unchecked(entity.bucket())
                    .as_ref()
                    .unwrap()
                    .get_unchecked(entity.bucket_index())
            };
            if dense_index < self.dense.len() {
                // SAFE we're inbound
                let dense_id = unsafe { *self.dense.get_unchecked(dense_index) };
                if dense_id.index() == entity.index() && dense_id.version() <= entity.version() {
                    match &mut self.pack_info.pack {
                        Pack::Tight(pack_info) => {
                            if dense_index < pack_info.len {
                                pack_info.len -= 1;
                                unsafe {
                                    // swap index and last packed element (can be the same)
                                    let last_packed = *self.dense.get_unchecked(pack_info.len);
                                    *self
                                        .sparse
                                        .get_unchecked_mut(last_packed.bucket())
                                        .as_mut()
                                        .unwrap()
                                        .get_unchecked_mut(last_packed.bucket_index()) =
                                        dense_index;
                                }
                                self.dense.swap(dense_index, pack_info.len);
                                self.data.swap(dense_index, pack_info.len);
                                dense_index = pack_info.len;
                            }
                        }
                        Pack::Loose(pack_info) => {
                            if dense_index < pack_info.len - 1 {
                                pack_info.len -= 1;
                                unsafe {
                                    // swap index and last packed element (can be the same)
                                    let dense = self.dense.get_unchecked(pack_info.len);
                                    *self
                                        .sparse
                                        .get_unchecked_mut(dense.bucket())
                                        .as_mut()
                                        .unwrap()
                                        .get_unchecked_mut(dense.bucket_index()) = dense_index;
                                }
                                self.dense.swap(dense_index, pack_info.len);
                                self.data.swap(dense_index, pack_info.len);
                                dense_index = pack_info.len;
                            }
                        }
                        Pack::Update(pack) => {
                            if dense_index < pack.inserted {
                                pack.inserted -= 1;
                                unsafe {
                                    // SAFE pack.inserted is a valid index
                                    let dense = *self.dense.get_unchecked(pack.inserted);
                                    // SAFE dense can always index into sparse
                                    *self
                                        .sparse
                                        .get_unchecked_mut(dense.bucket())
                                        .as_mut()
                                        .unwrap()
                                        .get_unchecked_mut(dense.bucket_index()) = dense_index;
                                }
                                self.dense.swap(dense_index, pack.inserted);
                                self.data.swap(dense_index, pack.inserted);
                                dense_index = pack.inserted;
                            }
                            if dense_index < pack.inserted + pack.modified {
                                pack.modified -= 1;
                                unsafe {
                                    // SAFE pack.inserted + pack.modified is a valid index
                                    let dense =
                                        *self.dense.get_unchecked(pack.inserted + pack.modified);
                                    // SAFE dense can always index into sparse
                                    *self
                                        .sparse
                                        .get_unchecked_mut(dense.bucket())
                                        .as_mut()
                                        .unwrap()
                                        .get_unchecked_mut(dense.bucket_index()) = dense_index;
                                }
                                self.dense.swap(dense_index, pack.inserted + pack.modified);
                                self.data.swap(dense_index, pack.inserted + pack.modified);
                                dense_index = pack.inserted + pack.modified;
                            }
                        }
                        Pack::NoPack => {}
                    }
                    unsafe {
                        // SAFE we're in bound
                        let last = *self.dense.get_unchecked(self.dense.len() - 1);
                        // SAFE dense can always index into sparse
                        *self
                            .sparse
                            .get_unchecked_mut(last.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(last.bucket_index()) = dense_index;
                        // SAFE we checked for OOB
                        *self
                            .sparse
                            .get_unchecked_mut(entity.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(entity.bucket_index()) = core::usize::MAX;
                    }
                    self.dense.swap_remove(dense_index);
                    if dense_id.version() == entity.version() {
                        Some(self.data.swap_remove(dense_index))
                    } else {
                        self.data.swap_remove(dense_index);
                        None
                    }
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
    /// Deletes `entity`'s component from this storage.
    pub fn try_delete(&mut self, entity: EntityId) -> Result<(), error::Remove>
    where
        T: 'static,
    {
        if self.pack_info.observer_types.is_empty() {
            match self.pack_info.pack {
                Pack::Tight(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                Pack::Loose(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                _ => {
                    self.actual_delete(entity);
                    Ok(())
                }
            }
        } else {
            Err(error::Remove::MissingPackStorage(type_name::<T>()))
        }
    }
    /// Deletes `entity`'s component from this storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn delete(&mut self, entity: EntityId)
    where
        T: 'static,
    {
        self.try_delete(entity).unwrap()
    }
    pub(crate) fn actual_delete(&mut self, entity: EntityId) {
        if let Some(component) = self.actual_remove(entity) {
            if let Pack::Update(pack) = &mut self.pack_info.pack {
                pack.deleted.push((entity, component));
            }
        }
    }
    /// Returns true if the storage contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        // we're not delegating to window since we know the index is in range
        if let Some(bucket) = self.sparse.get(entity.bucket()).and_then(Option::as_ref) {
            unsafe {
                // SAFE bucket_index is always a valid bucket index
                *bucket.get_unchecked(entity.bucket_index()) != core::usize::MAX
            }
        } else {
            false
        }
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
                            .get_unchecked(entity.bucket_index()),
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
                    .get_unchecked(entity.bucket_index())
            };
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
                            let dense = self.dense.get_unchecked(non_mod);
                            *self
                                .sparse
                                .get_unchecked_mut(dense.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(dense.bucket_index()) = non_mod;
                            let dense = *self.dense.get_unchecked(index);
                            *self
                                .sparse
                                .get_unchecked_mut(dense.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(dense.bucket_index()) = index;
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
        self.dense.len()
    }
    /// Returns true if the window's length is 0.
    pub fn is_empty(&self) -> bool {
        self.window().is_empty()
    }
    /// Returns the *inserted* section of an update packed window.
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window::new(self, 0..pack.inserted))
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted(&self) -> Window<'_, T> {
        self.try_inserted().unwrap()
    }
    /// Returns the *inserted* section of an update packed window mutably.
    pub fn try_inserted_mut(&mut self) -> Result<WindowMut<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            let range = 0..pack.inserted;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed window mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_mut().unwrap()
    }
    /// Returns the *modified* section of an update packed window.
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::Modified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window::new(
                self,
                pack.inserted..pack.inserted + pack.modified,
            ))
        } else {
            Err(error::Modified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn modified(&self) -> Window<'_, T> {
        self.try_modified().unwrap()
    }
    /// Returns the *modified* section of an update packed window mutably.
    pub fn try_modified_mut(&mut self) -> Result<WindowMut<'_, T>, error::Modified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            let range = pack.inserted..pack.inserted + pack.modified;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::Modified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed window mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_modified_mut().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed window.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::InsertedOrModified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window::new(self, 0..pack.inserted + pack.modified))
        } else {
            Err(error::InsertedOrModified::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        self.try_inserted_or_modified().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed window mutably.
    pub fn try_inserted_or_modified_mut(
        &mut self,
    ) -> Result<WindowMut<'_, T>, error::InsertedOrModified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            let range = 0..pack.inserted + pack.modified;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::InsertedOrModified::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed window mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
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
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.try_deleted().unwrap()
    }
    /// Takes ownership of the *deleted* components of an update packed window.
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        self.window_mut().try_take_deleted()
    }
    /// Takes ownership of the *deleted* components of an update packed window.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.try_take_deleted().unwrap()
    }
    /// Moves all component in the *inserted* section of an update packed window to the *neutral* section.
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_inserted()
    }
    /// Moves all component in the *inserted* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_inserted(&mut self) {
        self.try_clear_inserted().unwrap()
    }
    /// Moves all component in the *modified* section of an update packed window to the *neutral* section.
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_modified()
    }
    /// Moves all component in the *modified* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_modified(&mut self) {
        self.try_clear_modified().unwrap()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed window to the *neutral* section.
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_inserted_and_modified()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed window to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_inserted_and_modified(&mut self) {
        self.try_clear_inserted_and_modified().unwrap()
    }
    //          ▼ old end of pack
    //              ▼ new end of pack
    // [_ _ _ _ | _ | _ _ _ _ _]
    //            ▲       ▼
    //            ---------
    //              pack
    pub(crate) fn pack(&mut self, entity: EntityId) {
        if let Some(dense_index) = self.index_of(entity) {
            match &mut self.pack_info.pack {
                Pack::Tight(pack) => {
                    if dense_index >= pack.len {
                        unsafe {
                            // SAFE pack.len is in bound
                            let first_non_packed = *self.dense.get_unchecked(pack.len);
                            // SAFE we checked the entity has a component and bucket_index is alwyas in bound
                            *self
                                .sparse
                                .get_unchecked_mut(entity.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(entity.bucket_index()) = pack.len;
                            *self
                                .sparse
                                .get_unchecked_mut(first_non_packed.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(first_non_packed.bucket_index()) = dense_index;
                        }
                        self.dense.swap(pack.len, dense_index);
                        self.data.swap(pack.len, dense_index);
                        pack.len += 1;
                    }
                }
                Pack::Loose(pack) => {
                    if dense_index >= pack.len {
                        unsafe {
                            // SAFE pack.len is in bound
                            let first_non_packed = *self.dense.get_unchecked(pack.len);
                            // SAFE we checked the entity has a component and bucket_index is alwyas in bound
                            *self
                                .sparse
                                .get_unchecked_mut(entity.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(entity.bucket_index()) = pack.len;
                            *self
                                .sparse
                                .get_unchecked_mut(first_non_packed.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(first_non_packed.bucket_index()) = dense_index;
                        }
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
        self.window_mut().unpack(entity)
    }
    pub(crate) fn clone_indices(&self) -> Vec<EntityId> {
        self.dense.clone()
    }
    /// Update packs this storage making it track *inserted*, *modified* and *deleted* components.
    pub fn try_update_pack(&mut self) -> Result<(), error::Pack>
    where
        T: 'static,
    {
        match self.pack_info.pack {
            Pack::NoPack => {
                self.pack_info.pack = Pack::Update(UpdatePack {
                    inserted: self.len(),
                    modified: 0,
                    deleted: Vec::new(),
                });
                Ok(())
            }
            Pack::Tight(_) => Err(error::Pack::AlreadyTightPack(type_name::<T>())),
            Pack::Loose(_) => Err(error::Pack::AlreadyLoosePack(type_name::<T>())),
            Pack::Update(_) => Err(error::Pack::AlreadyUpdatePack(type_name::<T>())),
        }
    }
    /// Update packs this storage making it track *inserted*, *modified* and *deleted* components.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn update_pack(&mut self)
    where
        T: 'static,
    {
        self.try_update_pack().unwrap()
    }
    /// Reserves memory for at least `additional` components. Adding components can still allocate though.
    pub fn reserve(&mut self, additional: usize) {
        self.dense.reserve(additional);
        self.data.reserve(additional);
    }
    /// Deletes all components in this storage.
    pub fn clear(&mut self) {
        for id in &self.dense {
            self.sparse[id.bucket()].as_mut().unwrap()[id.bucket_index()] = core::usize::MAX;
        }
        match &mut self.pack_info.pack {
            Pack::Tight(tight) => tight.len = 0,
            Pack::Loose(loose) => loose.len = 0,
            Pack::Update(update) => update
                .deleted
                .extend(self.dense.drain(..).zip(self.data.drain(..))),
            Pack::NoPack => {}
        }
        self.dense.clear();
        self.data.clear();
    }
    /// Returns the `EntityId` at a given `index`.
    pub fn try_id_at(&self, index: usize) -> Option<EntityId> {
        self.dense.get(index).copied()
    }
    /// Returns the `EntityId` at a given `index`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn id_at(&self, index: usize) -> EntityId {
        self.try_id_at(index).unwrap()
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        if let Some(bucket) = self.sparse.get(entity.bucket()).and_then(Option::as_ref) {
            let index = unsafe { *bucket.get_unchecked(entity.bucket_index()) };
            if index != core::usize::MAX {
                Some(index)
            } else {
                None
            }
        } else {
            None
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        if let Some(bucket) = self.sparse.get_unchecked(entity.bucket()) {
            *bucket.get_unchecked(entity.bucket_index())
        } else {
            core::hint::unreachable_unchecked()
        }
    }
    /// Returns a slice of all the components in this storage.
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
            Ok(Window::new(self, range))
        } else {
            Err(error::NotInbound::View(type_name::<T>()))
        }
    }
    /// Returns a window over `range`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
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
                if !range.contains(&(update.inserted + update.modified)) {
                    return Err(error::NotInbound::UpdatePack);
                }
            }
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::NotInbound::View(type_name::<T>()))
        }
    }
    /// Returns a mutable window over `range`.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn as_window_mut<R: core::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> WindowMut<'_, T> {
        self.try_as_window_mut(range).unwrap()
    }
    /// Swaps `a` and `b`'s components.
    pub fn swap(&mut self, a: EntityId, b: EntityId) {
        if let Some(a_index) = self.index_of(a) {
            if let Some(b_index) = self.index_of(b) {
                self.data.swap(a_index, b_index);

                if let Pack::Update(pack) = &mut self.pack_info.pack {
                    let mut non_mut = pack.inserted + pack.modified;
                    if a_index >= non_mut {
                        self.dense.swap(a_index, non_mut);
                        self.data.swap(a_index, non_mut);

                        // SAFE non_mut exists
                        // a.bucket() exists and contains at least bucket_index elements
                        unsafe {
                            let non_mut_id = *self.dense.get_unchecked(non_mut);

                            *self
                                .sparse
                                .get_unchecked_mut(a.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(a.bucket_index()) = non_mut;

                            *self
                                .sparse
                                .get_unchecked_mut(non_mut_id.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(non_mut_id.bucket_index()) = a_index;

                            pack.modified += 1;
                            non_mut += 1;
                        }
                    }

                    if b_index >= non_mut {
                        self.dense.swap(b_index, non_mut);
                        self.data.swap(b_index, non_mut);

                        // SAFE non_mut exists
                        // a.bucket() exists and contains at least bucket_index elements
                        unsafe {
                            let non_mut_id = *self.dense.get_unchecked(non_mut);

                            *self
                                .sparse
                                .get_unchecked_mut(b.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(b.bucket_index()) = non_mut;

                            *self
                                .sparse
                                .get_unchecked_mut(non_mut_id.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(non_mut_id.bucket_index()) = b_index;

                            pack.modified += 1;
                        }
                    }
                }
            }
        }
    }
}

impl<T> core::ops::Index<EntityId> for SparseSet<T> {
    type Output = T;
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

impl<T> core::ops::IndexMut<EntityId> for SparseSet<T> {
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.get_mut(entity).unwrap()
    }
}

impl<T: 'static> UnknownStorage for SparseSet<T> {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<TypeId>) {
        self.actual_delete(entity);

        storage_to_unpack.reserve(self.pack_info.observer_types.len());

        let mut i = 0;
        for observer in self.pack_info.observer_types.iter().copied() {
            while i < storage_to_unpack.len() && observer < storage_to_unpack[i] {
                i += 1;
            }
            if storage_to_unpack.is_empty() || observer != storage_to_unpack[i] {
                storage_to_unpack.insert(i, observer);
            }
        }
    }
    fn clear(&mut self) {
        <Self>::clear(self)
    }
    fn unpack(&mut self, entity: EntityId) {
        Self::unpack(self, entity);
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[test]
fn insert() {
    let mut array = SparseSet::new();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(0);
    assert!(array.insert("0", entity_id).is_none());
    entity_id.set_index(1);
    assert!(array.insert("1", entity_id).is_none());
    assert_eq!(array.len(), 2);
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), Some(&"0"));
    entity_id.set_index(1);
    assert_eq!(array.get(entity_id), Some(&"1"));
    entity_id.set_index(5);
    assert!(array.insert("5", entity_id).is_none());
    assert_eq!(array.get_mut(entity_id), Some(&mut "5"));
    entity_id.set_index(4);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(6);
    assert_eq!(array.get(entity_id), None);
    assert!(array.insert("6", entity_id).is_none());
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(6);
    assert_eq!(array.get_mut(entity_id), Some(&mut "6"));
    entity_id.set_index(4);
    assert_eq!(array.get(entity_id), None);
}
#[test]
fn remove() {
    let mut array = SparseSet::new();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(0);
    array.insert("0", entity_id);
    entity_id.set_index(5);
    array.insert("5", entity_id);
    entity_id.set_index(10);
    array.insert("10", entity_id);
    entity_id.set_index(0);
    assert_eq!(array.actual_remove(entity_id), Some("0"));
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"10"));
    assert_eq!(array.actual_remove(entity_id), Some("10"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    assert_eq!(array.len(), 1);
    entity_id.set_index(3);
    array.insert("3", entity_id);
    entity_id.set_index(10);
    array.insert("100", entity_id);
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), Some(&"3"));
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    entity_id.set_index(3);
    assert_eq!(array.actual_remove(entity_id), Some("3"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    assert_eq!(array.actual_remove(entity_id), Some("100"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.actual_remove(entity_id), Some("5"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    assert_eq!(array.len(), 0);
}
