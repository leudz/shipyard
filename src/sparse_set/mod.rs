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

#[derive(Copy, Clone)]
pub(crate) union SparseIndex {
    owned: usize,
    shared: EntityId,
}

/// Component storage.
// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.
//
// An entity is shared is self.shared > 0, the sparse index isn't usize::MAX and dense doesn't point back
// Shared components don't qualify for packs
pub struct SparseSet<T> {
    pub(crate) sparse: Vec<Option<Box<[SparseIndex; BUCKET_SIZE]>>>,
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) pack_info: PackInfo<T>,
    shared: usize,
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            pack_info: Default::default(),
            shared: 0,
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
                *self.sparse.get_unchecked_mut(entity.bucket()) = Some(Box::new(
                    [SparseIndex {
                        owned: core::usize::MAX,
                    }; BUCKET_SIZE],
                ));
            }
        }
    }
    pub(crate) fn insert(&mut self, mut value: T, entity: EntityId) -> Option<OldComponent<T>> {
        self.allocate_at(entity);

        let (old_component, index) = unsafe {
            match self.sparse_index(entity).unwrap() {
                SparseIndex { owned }
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || *self.dense.get_unchecked(owned) == entity =>
                {
                    // SAFE entity.bucket() exists and contains at least bucket_index elements
                    let (result, index) = match &mut self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index())
                        .owned
                    {
                        i if *i == core::usize::MAX => {
                            *i = self.dense.len();
                            self.dense.push(entity);
                            self.data.push(value);
                            (None, self.dense.len() - 1)
                        }
                        &mut i => {
                            // SAFE sparse index are always valid
                            core::mem::swap(self.data.get_unchecked_mut(i), &mut value);
                            (Some(value), i)
                        }
                    };

                    (result.map(OldComponent::Owned), index)
                }
                SparseIndex { shared: _ } => {
                    *self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index()) = SparseIndex {
                        owned: self.dense.len(),
                    };

                    self.dense.push(entity);
                    self.data.push(value);

                    (Some(OldComponent::Shared), self.dense.len() - 1)
                }
            }
        };

        if let Pack::Update(pack) = &mut self.pack_info.pack {
            match old_component {
                Some(OldComponent::Owned(_)) if index >= pack.inserted + pack.modified => {
                    self.dense.swap(pack.inserted + pack.modified, index);
                    self.data.swap(pack.inserted + pack.modified, index);

                    let entity = self.dense[index];
                    // SAFE entity.bucket() exists and contains at least bucket_index elements
                    unsafe {
                        self.sparse
                            .get_unchecked_mut(entity.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(entity.bucket_index())
                            .owned = index;
                    }

                    let entity = self.dense[pack.inserted + pack.modified];
                    // SAFE entity.bucket() exists and contains at least bucket_index elements
                    unsafe {
                        self.sparse
                            .get_unchecked_mut(entity.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(entity.bucket_index())
                            .owned = pack.inserted + pack.modified;
                    }

                    pack.modified += 1;
                }
                Some(OldComponent::Shared) | None => {
                    self.dense.swap(pack.inserted + pack.modified, index);
                    self.data.swap(pack.inserted + pack.modified, index);
                    self.dense
                        .swap(pack.inserted, pack.inserted + pack.modified);
                    self.data.swap(pack.inserted, pack.inserted + pack.modified);

                    let entity = self.dense[pack.inserted];
                    // SAFE entity.bucket() exists and contains at least bucket_index elements
                    unsafe {
                        self.sparse
                            .get_unchecked_mut(entity.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(entity.bucket_index())
                            .owned = pack.inserted;
                    }

                    let entity = self.dense[pack.inserted + pack.modified];
                    // SAFE entity.bucket() exists and contains at least bucket_index elements
                    unsafe {
                        self.sparse
                            .get_unchecked_mut(entity.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(entity.bucket_index())
                            .owned = pack.inserted + pack.modified;
                    }

                    let entity = self.dense[index];
                    // SAFE entity.bucket() exists and contains at least bucket_index elements
                    unsafe {
                        self.sparse
                            .get_unchecked_mut(entity.bucket())
                            .as_mut()
                            .unwrap()
                            .get_unchecked_mut(entity.bucket_index())
                            .owned = index;
                    }

                    pack.inserted += 1;
                }
                _ => {}
            }
        }

        old_component
    }
    /// Removes `entity`'s component from this storage.
    pub fn try_remove(&mut self, entity: EntityId) -> Result<Option<OldComponent<T>>, error::Remove>
    where
        T: 'static,
    {
        if self.pack_info.observer_types.is_empty() {
            match self.pack_info.pack {
                Pack::Tight(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                Pack::Loose(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                Pack::Update(_) => {
                    let component = self.actual_remove(entity);

                    if let Some(OldComponent::Owned(_)) = &component {
                        if let Pack::Update(update) = &mut self.pack_info.pack {
                            update.removed.push(entity);
                        } else {
                            unreachable!()
                        }
                    }

                    Ok(component)
                }
                Pack::NoPack => Ok(self.actual_remove(entity)),
            }
        } else {
            Err(error::Remove::MissingPackStorage(type_name::<T>()))
        }
    }
    /// Removes `entity`'s component from this storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn remove(&mut self, entity: EntityId) -> Option<OldComponent<T>>
    where
        T: 'static,
    {
        self.try_remove(entity).unwrap()
    }
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<OldComponent<T>> {
        unsafe {
            match self.sparse_index(entity) {
                Some(SparseIndex { owned })
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || self.dense.get(owned).copied() == Some(entity) =>
                {
                    if owned != core::usize::MAX {
                        // SAFE we're inbound
                        let mut dense_index = self
                            .sparse
                            .get_unchecked(entity.bucket())
                            .as_ref()
                            .unwrap()
                            .get_unchecked(entity.bucket_index())
                            .owned;
                        if dense_index < self.dense.len() {
                            // SAFE we're inbound
                            let dense_id = *self.dense.get_unchecked(dense_index);
                            if dense_id.index() == entity.index() && dense_id.gen() <= entity.gen()
                            {
                                match &mut self.pack_info.pack {
                                    Pack::Tight(pack_info) => {
                                        if dense_index < pack_info.len {
                                            pack_info.len -= 1;
                                            // swap index and last packed element (can be the same)
                                            let last_packed =
                                                *self.dense.get_unchecked(pack_info.len);
                                            self.sparse
                                                .get_unchecked_mut(last_packed.bucket())
                                                .as_mut()
                                                .unwrap()
                                                .get_unchecked_mut(last_packed.bucket_index())
                                                .owned = dense_index;

                                            self.dense.swap(dense_index, pack_info.len);
                                            self.data.swap(dense_index, pack_info.len);
                                            dense_index = pack_info.len;
                                        }
                                    }
                                    Pack::Loose(pack_info) => {
                                        if dense_index < pack_info.len - 1 {
                                            pack_info.len -= 1;
                                            // swap index and last packed element (can be the same)
                                            let dense = self.dense.get_unchecked(pack_info.len);
                                            self.sparse
                                                .get_unchecked_mut(dense.bucket())
                                                .as_mut()
                                                .unwrap()
                                                .get_unchecked_mut(dense.bucket_index())
                                                .owned = dense_index;

                                            self.dense.swap(dense_index, pack_info.len);
                                            self.data.swap(dense_index, pack_info.len);
                                            dense_index = pack_info.len;
                                        }
                                    }
                                    Pack::Update(pack) => {
                                        if dense_index < pack.inserted {
                                            pack.inserted -= 1;
                                            // SAFE pack.inserted is a valid index
                                            let dense = *self.dense.get_unchecked(pack.inserted);
                                            // SAFE dense can always index into sparse
                                            self.sparse
                                                .get_unchecked_mut(dense.bucket())
                                                .as_mut()
                                                .unwrap()
                                                .get_unchecked_mut(dense.bucket_index())
                                                .owned = dense_index;

                                            self.dense.swap(dense_index, pack.inserted);
                                            self.data.swap(dense_index, pack.inserted);
                                            dense_index = pack.inserted;
                                        }
                                        if dense_index < pack.inserted + pack.modified {
                                            pack.modified -= 1;
                                            // SAFE pack.inserted + pack.modified is a valid index
                                            let dense = *self
                                                .dense
                                                .get_unchecked(pack.inserted + pack.modified);
                                            // SAFE dense can always index into sparse
                                            self.sparse
                                                .get_unchecked_mut(dense.bucket())
                                                .as_mut()
                                                .unwrap()
                                                .get_unchecked_mut(dense.bucket_index())
                                                .owned = dense_index;

                                            self.dense
                                                .swap(dense_index, pack.inserted + pack.modified);
                                            self.data
                                                .swap(dense_index, pack.inserted + pack.modified);
                                            dense_index = pack.inserted + pack.modified;
                                        }
                                    }
                                    Pack::NoPack => {}
                                }
                                // SAFE we're in bound
                                let last = *self.dense.get_unchecked(self.dense.len() - 1);
                                // SAFE dense can always index into sparse
                                self.sparse
                                    .get_unchecked_mut(last.bucket())
                                    .as_mut()
                                    .unwrap()
                                    .get_unchecked_mut(last.bucket_index())
                                    .owned = dense_index;
                                // SAFE we checked for OOB
                                self.sparse
                                    .get_unchecked_mut(entity.bucket())
                                    .as_mut()
                                    .unwrap()
                                    .get_unchecked_mut(entity.bucket_index())
                                    .owned = core::usize::MAX;

                                self.dense.swap_remove(dense_index);
                                if dense_id.gen() == entity.gen() {
                                    Some(OldComponent::Owned(self.data.swap_remove(dense_index)))
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
                Some(SparseIndex { shared: _ }) => {
                    self.unshare(entity);
                    Some(OldComponent::Shared)
                }
                None => None,
            }
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
        if let Some(OldComponent::Owned(component)) = self.actual_remove(entity) {
            if let Pack::Update(pack) = &mut self.pack_info.pack {
                pack.deleted.push((entity, component));
            }
        }
    }
    /// Returns true if the storage contains `entity`.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }
    pub fn contains_owned(&self, entity: EntityId) -> bool {
        unsafe {
            match self.sparse_index(entity) {
                Some(SparseIndex { owned })
                    if self.shared == 0 || self.dense.get(owned).copied() == Some(entity) =>
                {
                    true
                }
                _ => false,
            }
        }
    }
    pub fn contains_shared(&self, entity: EntityId) -> bool {
        unsafe {
            match self.sparse_index(entity) {
                Some(SparseIndex { owned })
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || self.dense.get(owned).copied() == Some(entity) =>
                {
                    false
                }
                Some(_) => true,
                None => false,
            }
        }
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        unsafe {
            self.index_of(entity)
                .map(|index| self.data.get_unchecked(index))
        }
    }
    pub(crate) fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        unsafe {
            match self.sparse_index(entity) {
                Some(SparseIndex { owned: mut index })
                    if self.shared == 0
                        || index == core::usize::MAX
                        || self.dense.get(index).copied() == Some(entity) =>
                {
                    if index != core::usize::MAX {
                        if let Pack::Update(pack) = &mut self.pack_info.pack {
                            // index of the first element non modified
                            let non_mod = pack.inserted + pack.modified;
                            if index >= non_mod {
                                // SAFE we checked the storage contains the entity
                                ptr::swap(
                                    self.dense.get_unchecked_mut(non_mod),
                                    self.dense.get_unchecked_mut(index),
                                );
                                ptr::swap(
                                    self.data.get_unchecked_mut(non_mod),
                                    self.data.get_unchecked_mut(index),
                                );
                                let dense = self.dense.get_unchecked(non_mod);
                                self.sparse
                                    .get_unchecked_mut(dense.bucket())
                                    .as_mut()
                                    .unwrap()
                                    .get_unchecked_mut(dense.bucket_index())
                                    .owned = non_mod;
                                let dense = *self.dense.get_unchecked(index);
                                self.sparse
                                    .get_unchecked_mut(dense.bucket())
                                    .as_mut()
                                    .unwrap()
                                    .get_unchecked_mut(dense.bucket_index())
                                    .owned = index;

                                pack.modified += 1;
                                index = non_mod;
                            }
                        }

                        Some(self.data.get_unchecked_mut(index))
                    } else {
                        None
                    }
                }
                Some(SparseIndex { shared }) => self.get_mut(shared),
                None => None,
            }
        }
    }
    /// Returns the length of the storage.
    pub fn len(&self) -> usize {
        self.dense.len()
    }
    /// Returns true if the storage's length is 0.
    pub fn is_empty(&self) -> bool {
        self.window().is_empty()
    }
    /// Returns the *inserted* section of an update packed storage.
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window::new(self, 0..pack.inserted))
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted(&self) -> Window<'_, T> {
        self.try_inserted().unwrap()
    }
    /// Returns the *inserted* section of an update packed storage mutably.
    pub fn try_inserted_mut(&mut self) -> Result<WindowMut<'_, T>, error::Inserted> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            let range = 0..pack.inserted;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::Inserted::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* section of an update packed storage mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_mut().unwrap()
    }
    /// Returns the *modified* section of an update packed storage.
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
    /// Returns the *modified* section of an update packed storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn modified(&self) -> Window<'_, T> {
        self.try_modified().unwrap()
    }
    /// Returns the *modified* section of an update packed storage mutably.
    pub fn try_modified_mut(&mut self) -> Result<WindowMut<'_, T>, error::Modified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            let range = pack.inserted..pack.inserted + pack.modified;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::Modified::NotUpdatePacked)
        }
    }
    /// Returns the *modified* section of an update packed storage mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_modified_mut().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed storage.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::InsertedOrModified> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window::new(self, 0..pack.inserted + pack.modified))
        } else {
            Err(error::InsertedOrModified::NotUpdatePacked)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        self.try_inserted_or_modified().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed storage mutably.
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
    /// Returns the *inserted* and *modified* section of an update packed wistoragenstoragedow mutably.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_or_modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_or_modified_mut().unwrap()
    }
    /// Returns the *deleted* components of an update packed storage.
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *deleted* components of an update packed storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.try_deleted().unwrap()
    }
    /// Returns the ids of *removed* components of an update packed storage.
    pub fn try_removed(&self) -> Result<&[EntityId], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(&pack.removed)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the ids of *removed* components of an update packed storage.
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn removed(&self) -> &[EntityId] {
        self.try_removed().unwrap()
    }
    /// Takes ownership of the *deleted* components of an update packed storage.
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        self.window_mut().try_take_deleted()
    }
    /// Takes ownership of the *deleted* components of an update packed storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.try_take_deleted().unwrap()
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.
    pub fn try_take_removed(&mut self) -> Result<Vec<EntityId>, error::NotUpdatePack> {
        self.window_mut().try_take_removed()
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        self.try_take_removed().unwrap()
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_inserted()
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_inserted(&mut self) {
        self.try_clear_inserted().unwrap()
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_modified()
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_modified(&mut self) {
        self.try_clear_modified().unwrap()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_inserted_and_modified()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.  
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
                            self.sparse
                                .get_unchecked_mut(entity.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(entity.bucket_index())
                                .owned = pack.len;
                            self.sparse
                                .get_unchecked_mut(first_non_packed.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(first_non_packed.bucket_index())
                                .owned = dense_index;
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
                            self.sparse
                                .get_unchecked_mut(entity.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(entity.bucket_index())
                                .owned = pack.len;
                            self.sparse
                                .get_unchecked_mut(first_non_packed.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(first_non_packed.bucket_index())
                                .owned = dense_index;
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
                    removed: Vec::new(),
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
            self.sparse[id.bucket()].as_mut().unwrap()[id.bucket_index()].owned = core::usize::MAX;
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
        unsafe {
            match self.sparse_index(entity) {
                Some(SparseIndex { owned })
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || self.dense.get(owned).copied() == Some(entity) =>
                {
                    if owned != core::usize::MAX {
                        Some(owned)
                    } else {
                        None
                    }
                }
                Some(SparseIndex { shared }) => self.index_of(shared),
                None => None,
            }
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        match self.sparse_index(entity) {
            Some(SparseIndex { owned })
                if self.shared == 0
                    || owned == core::usize::MAX
                    || self.dense.get(owned).copied() == Some(entity) =>
            {
                owned
            }
            Some(SparseIndex { shared }) => self.index_of_unchecked(shared),
            None => unreachable!(),
        }
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    pub unsafe fn index_of_owned_unchecked(&self, entity: EntityId) -> usize {
        if let Some(bucket) = self.sparse.get_unchecked(entity.bucket()) {
            bucket.get_unchecked(entity.bucket_index()).owned
        } else {
            core::hint::unreachable_unchecked()
        }
    }
    fn sparse_index(&self, entity: EntityId) -> Option<SparseIndex> {
        // SAFE bucket_index always returns a valid bucket index
        self.sparse
            .get(entity.bucket())
            .and_then(Option::as_ref)
            .map(|bucket| unsafe { *bucket.get_unchecked(entity.bucket_index()) })
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
    pub fn swap(&mut self, mut a: EntityId, mut b: EntityId) {
        unsafe {
            match self.sparse_index(a) {
                Some(SparseIndex { owned })
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || self.dense.get(owned).copied() == Some(a) => {}
                Some(SparseIndex { shared }) => {
                    a = shared;
                }
                None => {}
            }

            match self.sparse_index(b) {
                Some(SparseIndex { owned })
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || self.dense.get(owned).copied() == Some(b) => {}
                Some(SparseIndex { shared }) => {
                    b = shared;
                }
                None => {}
            }
        }

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

                            self.sparse
                                .get_unchecked_mut(a.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(a.bucket_index())
                                .owned = non_mut;

                            self.sparse
                                .get_unchecked_mut(non_mut_id.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(non_mut_id.bucket_index())
                                .owned = a_index;

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

                            self.sparse
                                .get_unchecked_mut(b.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(b.bucket_index())
                                .owned = non_mut;

                            self.sparse
                                .get_unchecked_mut(non_mut_id.bucket())
                                .as_mut()
                                .unwrap()
                                .get_unchecked_mut(non_mut_id.bucket_index())
                                .owned = b_index;

                            pack.modified += 1;
                        }
                    }
                }
            }
        }
    }
    /// Shares `entity`'s component wuth `with` entity.  
    /// Deleting `entity`'s component won't stop the sahring.
    pub fn share(&mut self, entity: EntityId, with: EntityId) {
        self.allocate_at(with);

        unsafe {
            *self
                .sparse
                .get_unchecked_mut(with.bucket())
                .as_mut()
                .unwrap()
                .get_unchecked_mut(with.bucket_index()) = SparseIndex { shared: entity };
        }

        self.shared += 1;
    }
    /// Makes `entity` stop observing another entity.
    pub fn unshare(&mut self, entity: EntityId) {
        unsafe {
            match self.sparse_index(entity) {
                Some(SparseIndex { owned })
                    if self.shared == 0
                        || owned == core::usize::MAX
                        || self.dense.get(owned).copied() == Some(entity) => {}
                Some(SparseIndex { shared: _ }) => {
                    *self
                        .sparse
                        .get_unchecked_mut(entity.bucket())
                        .as_mut()
                        .unwrap()
                        .get_unchecked_mut(entity.bucket_index()) = SparseIndex {
                        owned: core::usize::MAX,
                    }
                }
                None => {}
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum OldComponent<T> {
    Owned(T),
    Shared,
}

impl<T> OldComponent<T> {
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn unwrap_owned(self) -> T {
        match self {
            Self::Owned(component) => component,
            Self::Shared => panic!("Called `OldComponent::unwrap_owned` on a `Shared` value"),
        }
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
    assert_eq!(
        array.actual_remove(entity_id),
        Some(OldComponent::Owned("0"))
    );
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"10"));
    assert_eq!(
        array.actual_remove(entity_id),
        Some(OldComponent::Owned("10"))
    );
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
    assert_eq!(
        array.actual_remove(entity_id),
        Some(OldComponent::Owned("3"))
    );
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    assert_eq!(
        array.actual_remove(entity_id),
        Some(OldComponent::Owned("100"))
    );
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(
        array.actual_remove(entity_id),
        Some(OldComponent::Owned("5"))
    );
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
