mod add_component;
mod contains;
#[cfg(feature = "serde1")]
mod deser;
mod metadata;
pub mod sort;
mod sparse_array;
mod view_add_entity;
mod windows;

pub use add_component::AddComponentUnchecked;
pub use contains::Contains;
pub use windows::{Window, WindowMut, WindowSort1};

#[cfg(feature = "serde1")]
pub(crate) use deser::SparseSetSerializer;
#[cfg(feature = "serde1")]
pub(crate) use metadata::SerdeInfos;
pub(crate) use metadata::{
    LoosePack, Metadata, Pack, TightPack, UpdatePack, BUCKET_SIZE as SHARED_BUCKET_SIZE,
};
pub(crate) use view_add_entity::ViewAddEntity;
pub(crate) use windows::RawWindowMut;

use crate::error;
#[cfg(feature = "serde1")]
use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig, SerConfig};
use crate::storage::EntityId;
use crate::type_id::TypeId;
use crate::unknown_storage::UnknownStorage;
#[cfg(all(not(feature = "std"), feature = "serde1"))]
use alloc::string::ToString;
use alloc::vec::Vec;
use core::any::{type_name, Any};
use core::ptr;
#[cfg(feature = "serde1")]
use deser::SparseSetDeserializer;
use sparse_array::{SparseArray, SparseSlice, SparseSliceMut};

pub(crate) const BUCKET_SIZE: usize = 256 / core::mem::size_of::<usize>();

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
    pub(crate) sparse: SparseArray<[usize; BUCKET_SIZE]>,
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) metadata: Metadata<T>,
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: SparseArray::new(),
            dense: Vec::new(),
            data: Vec::new(),
            metadata: Default::default(),
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
    /// Returns a slice of all the components in this storage.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    /// Returns a window over `range`.
    ///
    /// ### Errors
    ///
    /// - `range` was out of bounds.
    pub fn try_as_window<R: core::ops::RangeBounds<usize>>(
        &self,
        range: R,
    ) -> Result<Window<'_, T>, error::NotInbound> {
        use core::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => start.checked_add(1).unwrap_or(core::usize::MAX),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => end.checked_add(1).unwrap_or(core::usize::MAX),
            Bound::Excluded(end) => *end,
            Bound::Unbounded => self.len(),
        };
        let range = start..end;

        let full_range = 0..self.len();
        if full_range.contains(&start) && full_range.contains(&end.saturating_sub(1)) {
            Ok(Window::new(self, range))
        } else {
            Err(error::NotInbound::View(type_name::<T>()))
        }
    }
    /// Returns a window over `range`.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - `range` was out of bounds.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn as_window<R: core::ops::RangeBounds<usize>>(&self, range: R) -> Window<'_, T> {
        self.try_as_window(range).unwrap()
    }
    /// Returns a mutable window over `range`.
    ///
    /// ### Errors
    ///
    /// - `range` was out of bounds.
    pub fn try_as_window_mut<R: core::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> Result<WindowMut<'_, T>, error::NotInbound> {
        use core::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => start.checked_add(1).unwrap_or(core::usize::MAX),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(end) => end.checked_add(1).unwrap_or(core::usize::MAX),
            Bound::Excluded(end) => *end,
            Bound::Unbounded => self.len(),
        };
        let range = start..end;

        let full_range = 0..self.len();
        if full_range.contains(&start) && full_range.contains(&end.saturating_sub(1)) {
            if let Pack::Update(update) = &self.metadata.pack {
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
    ///
    /// ### Errors
    ///
    /// - `range` was out of bounds.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn as_window_mut<R: core::ops::RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> WindowMut<'_, T> {
        self.try_as_window_mut(range).unwrap()
    }
    pub(crate) fn clone_indices(&self) -> Vec<EntityId> {
        self.dense.clone()
    }
}

impl<T> SparseSet<T> {
    /// Returns `true` if `entity` owns or shares a component in this storage.
    ///
    /// In case it shares a component, returns `true` even if there is no owned component at the end of the shared chain.
    pub fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }
    /// Returns `true` if `entity` owns a component in this storage.
    pub fn contains_owned(&self, entity: EntityId) -> bool {
        self.index_of_owned(entity).is_some()
    }
    /// Returns `true` if `entity` shares a component in this storage.  
    ///
    /// Returns `true` even if there is no owned component at the end of the shared chain.
    pub fn contains_shared(&self, entity: EntityId) -> bool {
        self.shared_id(entity).is_some()
    }
    /// Returns the length of the storage.
    pub fn len(&self) -> usize {
        self.dense.len()
    }
    /// Returns true if the storage's length is 0.
    pub fn is_empty(&self) -> bool {
        self.window().is_empty()
    }
}

impl<T> SparseSet<T> {
    /// Returns the index of `entity`'s owned component in the `dense` and `data` vectors.
    ///
    /// In case `entity` is shared `index_of` will follow the shared chain to find the owned one at the end.  
    /// This index is only valid for this storage and until a modification happens.
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
    /// Returns the index of `entity`'s owned component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    pub fn index_of_owned(&self, entity: EntityId) -> Option<usize> {
        self.sparse.sparse_index(entity).and_then(|dense_index| {
            if dense_index != core::usize::MAX
                && self
                    .dense
                    .get(dense_index)
                    .map_or(false, |&dense| dense == entity)
            {
                Some(dense_index)
            } else {
                None
            }
        })
    }
    /// Returns the index of `entity`'s owned component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    ///
    /// # Safety
    ///
    /// `entity` has to own a component of this type.  
    /// The index is only valid until a modification occurs in the storage.
    pub unsafe fn index_of_owned_unchecked(&self, entity: EntityId) -> usize {
        match self.sparse.sparse_index(entity) {
            Some(dense_index) => dense_index,
            None => core::hint::unreachable_unchecked(),
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
    pub fn id_at(&self, index: usize) -> EntityId {
        self.try_id_at(index).unwrap()
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        self.index_of(entity)
            .map(|index| unsafe { self.data.get_unchecked(index) })
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
    /// Returns the `EntityId` `shared` entity points to.
    ///
    /// Returns `None` if the entity isn't shared.
    pub fn shared_id(&self, shared: EntityId) -> Option<EntityId> {
        if !self.contains_owned(shared) {
            match self.sparse.sparse_index(shared) {
                Some(gen) if gen as u64 == shared.gen() => {
                    self.metadata.shared.shared_index(shared)
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

impl<T> SparseSet<T> {
    /// Inserts `value` in the `SparseSet`.
    ///
    /// If an `entity` with the same index but a greater generation already has a component of this type, does nothing and returns `None`.
    ///
    /// Returns what was present at its place, one of the following:
    /// - None - no value present, either `entity` never had this component or it was removed/deleted
    /// - Some(OldComponent::Owned) - `entity` already had this component, it is no replaced
    /// - Some(OldComponent::OldGenOwned) - `entity` didn't have a component but an entity with the same index did and it wasn't removed with the entity
    /// - Some(OldComponent::Shared) - `entity` shared a component
    /// - Some(OldComponent::OldShared) - `entity` didn't have a component but an entity with the same index shared one and it wasn't removed with the entity
    ///
    /// # Update pack
    ///
    /// In case `entity` had a component of this type, the new component will be considered `modified`.
    /// In all other cases it'll be considered `inserted`.
    pub(crate) fn insert(&mut self, value: T, entity: EntityId) -> Option<OldComponent<T>> {
        self.sparse.allocate_at(entity);

        // at this point there can't be nothing at the sparse index
        let old_index = self.sparse.sparse_index(entity).unwrap();

        let (old_component, dense_index) = match old_index {
            core::usize::MAX => {
                unsafe {
                    self.sparse
                        .set_sparse_index_unchecked(entity, self.dense.len());
                }

                self.dense.push(entity);
                self.data.push(value);

                (None, self.dense.len() - 1)
            }
            dense_index => {
                if let Some(dense_id) = self.dense.get(dense_index).copied() {
                    if entity.gen() >= dense_id.gen() {
                        unsafe {
                            self.sparse.set_sparse_index_unchecked(entity, dense_index);

                            *self.dense.get_unchecked_mut(dense_index) = entity;

                            if entity.gen() == dense_id.gen() {
                                (
                                    Some(OldComponent::Owned(core::mem::replace(
                                        self.data.get_unchecked_mut(dense_index),
                                        value,
                                    ))),
                                    dense_index,
                                )
                            } else {
                                (
                                    Some(OldComponent::OldGenOwned(core::mem::replace(
                                        self.data.get_unchecked_mut(dense_index),
                                        value,
                                    ))),
                                    dense_index,
                                )
                            }
                        }
                    } else {
                        return None;
                    }
                } else if entity.gen() >= dense_index as u64 {
                    unsafe {
                        self.metadata
                            .shared
                            .set_sparse_index_unchecked(entity, EntityId::dead());

                        self.sparse.set_sparse_index_unchecked(entity, self.len());

                        self.dense.push(entity);
                        self.data.push(value);

                        if entity.gen() == dense_index as u64 {
                            (Some(OldComponent::Shared), self.len() - 1)
                        } else {
                            (Some(OldComponent::OldGenShared), self.len() - 1)
                        }
                    }
                } else {
                    return None;
                }
            }
        };

        if let Pack::Update(pack) = &mut self.metadata.pack {
            if dense_index >= pack.inserted + pack.modified {
                self.dense.swap(pack.inserted + pack.modified, dense_index);
                self.data.swap(pack.inserted + pack.modified, dense_index);

                unsafe {
                    let entity = *self.dense.get_unchecked(dense_index);
                    self.sparse.set_sparse_index_unchecked(entity, dense_index);
                }

                unsafe {
                    let entity = *self.dense.get_unchecked(pack.inserted + pack.modified);
                    self.sparse
                        .set_sparse_index_unchecked(entity, pack.inserted + pack.modified);
                }

                match old_component {
                    Some(OldComponent::Owned(_)) => pack.modified += 1,
                    _ => {
                        self.dense
                            .swap(pack.inserted + pack.modified, pack.inserted);
                        self.data.swap(pack.inserted + pack.modified, pack.inserted);

                        unsafe {
                            let entity = *self.dense.get_unchecked(pack.inserted + pack.modified);
                            self.sparse
                                .set_sparse_index_unchecked(entity, pack.inserted + pack.modified);
                        }

                        unsafe {
                            let entity = *self.dense.get_unchecked(pack.inserted);
                            self.sparse
                                .set_sparse_index_unchecked(entity, pack.inserted);
                        }

                        pack.inserted += 1;
                    }
                }
            }
        }

        old_component
    }
}

impl<T> SparseSet<T> {
    /// Removes `entity`'s component from this storage.
    ///
    /// ### Errors
    ///
    /// - Storage is tightly or loosly packed.
    pub fn try_remove(&mut self, entity: EntityId) -> Result<Option<OldComponent<T>>, error::Remove>
    where
        T: 'static,
    {
        if self.metadata.observer_types.is_empty() {
            match self.metadata.pack {
                Pack::Tight(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                Pack::Loose(_) => Err(error::Remove::MissingPackStorage(type_name::<T>())),
                Pack::Update(_) => {
                    let component = self.actual_remove(entity);

                    if let Some(OldComponent::Owned(_)) = &component {
                        if let Pack::Update(update) = &mut self.metadata.pack {
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
    ///
    /// ### Errors
    ///
    /// - Storage is tightly or loosly packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn remove(&mut self, entity: EntityId) -> Option<OldComponent<T>>
    where
        T: 'static,
    {
        self.try_remove(entity).unwrap()
    }
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<OldComponent<T>> {
        match self.sparse.sparse_index(entity) {
            Some(mut dense_index) if dense_index != core::usize::MAX => {
                let dense_id = unsafe { *self.dense.get_unchecked(dense_index) };

                if dense_id.index() == entity.index() && entity.gen() >= dense_id.gen() {
                    // moves the component out of the pack if in one
                    match &mut self.metadata.pack {
                        Pack::Tight(tight) => {
                            if dense_index < tight.len {
                                tight.len -= 1;

                                unsafe {
                                    // swap index and last packed element (can be the same)
                                    let last_packed = *self.dense.get_unchecked(tight.len);
                                    self.sparse
                                        .set_sparse_index_unchecked(last_packed, dense_index);
                                }

                                self.dense.swap(dense_index, tight.len);
                                self.data.swap(dense_index, tight.len);
                                dense_index = tight.len;
                            }
                        }
                        Pack::Loose(loose) => {
                            if dense_index < loose.len {
                                loose.len -= 1;

                                unsafe {
                                    // swap index and last packed element (can be the same)
                                    let last_packed = *self.dense.get_unchecked(loose.len);
                                    self.sparse
                                        .set_sparse_index_unchecked(last_packed, dense_index);
                                }

                                self.dense.swap(dense_index, loose.len);
                                self.data.swap(dense_index, loose.len);
                                dense_index = loose.len;
                            }
                        }
                        Pack::Update(update) => {
                            if dense_index < update.inserted {
                                update.inserted -= 1;

                                unsafe {
                                    // SAFE pack.inserted is a valid index
                                    let last_inserted = *self.dense.get_unchecked(update.inserted);
                                    // SAFE dense can always index into sparse
                                    self.sparse
                                        .set_sparse_index_unchecked(last_inserted, dense_index);
                                }

                                self.dense.swap(dense_index, update.inserted);
                                self.data.swap(dense_index, update.inserted);
                                dense_index = update.inserted;
                            }

                            if dense_index < update.inserted + update.modified {
                                update.modified -= 1;

                                unsafe {
                                    // SAFE pack.inserted + pack.modified is a valid index
                                    let last_modified = *self
                                        .dense
                                        .get_unchecked(update.inserted + update.modified);
                                    self.sparse
                                        .set_sparse_index_unchecked(last_modified, dense_index);
                                }

                                self.dense
                                    .swap(dense_index, update.inserted + update.modified);
                                self.data
                                    .swap(dense_index, update.inserted + update.modified);
                                dense_index = update.inserted + update.modified;
                            }
                        }
                        Pack::NoPack => {}
                    }

                    unsafe {
                        // SAFE we're in bound
                        let last = *self.dense.get_unchecked(self.dense.len() - 1);
                        // SAFE dense can always index into sparse
                        self.sparse.set_sparse_index_unchecked(last, dense_index);
                        // SAFE we checked for OOB
                        self.sparse
                            .set_sparse_index_unchecked(entity, core::usize::MAX);
                    }

                    self.dense.swap_remove(dense_index);
                    let old_component = self.data.swap_remove(dense_index);

                    if dense_id == entity {
                        Some(OldComponent::Owned(old_component))
                    } else {
                        Some(OldComponent::OldGenOwned(old_component))
                    }
                } else if self.metadata.shared.shared_index(entity).is_some() {
                    unsafe {
                        self.sparse
                            .set_sparse_index_unchecked(entity, core::usize::MAX);
                        self.metadata
                            .shared
                            .set_sparse_index_unchecked(entity, EntityId::dead());
                    }

                    if dense_index == entity.gen() as usize {
                        Some(OldComponent::Shared)
                    } else {
                        Some(OldComponent::OldGenShared)
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    /// Deletes `entity`'s component from this storage.
    ///
    /// ### Errors
    ///
    /// - Storage is tightly or loosly packed.
    pub fn try_delete(&mut self, entity: EntityId) -> Result<(), error::Remove>
    where
        T: 'static,
    {
        if self.metadata.observer_types.is_empty() {
            match self.metadata.pack {
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
    ///
    /// ### Errors
    ///
    /// - Storage is tightly or loosly packed.
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
            if let Pack::Update(pack) = &mut self.metadata.pack {
                pack.deleted.push((entity, component));
            }
        }
    }
}

impl<T> SparseSet<T> {
    /// Returns the *inserted* section of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(Window::new(self, 0..pack.inserted))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *inserted* section of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted(&self) -> Window<'_, T> {
        self.try_inserted().unwrap()
    }
    /// Returns the *inserted* section of an update packed storage mutably.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_inserted_mut(&mut self) -> Result<WindowMut<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            let range = 0..pack.inserted;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *inserted* section of an update packed storage mutably.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_mut().unwrap()
    }
    /// Returns the *modified* section of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(Window::new(
                self,
                pack.inserted..pack.inserted + pack.modified,
            ))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *modified* section of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn modified(&self) -> Window<'_, T> {
        self.try_modified().unwrap()
    }
    /// Returns the *modified* section of an update packed storage mutably.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_modified_mut(&mut self) -> Result<WindowMut<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            let range = pack.inserted..pack.inserted + pack.modified;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *modified* section of an update packed storage mutably.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_modified_mut().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(Window::new(self, 0..pack.inserted + pack.modified))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        self.try_inserted_or_modified().unwrap()
    }
    /// Returns the *inserted* and *modified* section of an update packed storage mutably.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_inserted_or_modified_mut(
        &mut self,
    ) -> Result<WindowMut<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            let range = 0..pack.inserted + pack.modified;
            Ok(WindowMut::new(self, range))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *inserted* and *modified* section of an update packed wistoragenstoragedow mutably.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn inserted_or_modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_or_modified_mut().unwrap()
    }
    /// Returns the *deleted* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the *deleted* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.try_deleted().unwrap()
    }
    /// Returns the ids of *removed* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_removed(&self) -> Result<&[EntityId], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.metadata.pack {
            Ok(&pack.removed)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the ids of *removed* components of an update packed storage.
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn removed(&self) -> &[EntityId] {
        self.try_removed().unwrap()
    }
    /// Takes ownership of the *deleted* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        self.window_mut().try_take_deleted()
    }
    /// Takes ownership of the *deleted* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.try_take_deleted().unwrap()
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_take_removed(&mut self) -> Result<Vec<EntityId>, error::NotUpdatePack> {
        self.window_mut().try_take_removed()
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        self.try_take_removed().unwrap()
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_inserted()
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_inserted(&mut self) {
        self.try_clear_inserted().unwrap()
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_modified()
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn clear_modified(&mut self) {
        self.try_clear_modified().unwrap()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        self.window_mut().try_clear_inserted_and_modified()
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
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
        if let Some(dense_index) = self.index_of_owned(entity) {
            match &mut self.metadata.pack {
                Pack::Tight(pack) => {
                    if dense_index >= pack.len {
                        unsafe {
                            // SAFE pack.len is in bound
                            let first_non_packed = *self.dense.get_unchecked(pack.len);
                            // SAFE we checked the entity has a component and bucket_index is alwyas in bound
                            self.sparse.set_sparse_index_unchecked(entity, pack.len);
                            self.sparse
                                .set_sparse_index_unchecked(first_non_packed, dense_index);
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
                            self.sparse.set_sparse_index_unchecked(entity, pack.len);
                            self.sparse
                                .set_sparse_index_unchecked(first_non_packed, dense_index);
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
    /// Update packs this storage making it track *inserted*, *modified* and *deleted* components.  
    /// Does nothing if the storage is already update packed.
    ///
    /// ### Errors
    ///
    /// - Storage is already tightly or loosly packed.
    pub fn try_update_pack(&mut self) -> Result<(), error::Pack>
    where
        T: 'static,
    {
        match self.metadata.pack {
            Pack::NoPack => {
                self.metadata.pack = Pack::Update(UpdatePack {
                    inserted: self.len(),
                    modified: 0,
                    removed: Vec::new(),
                    deleted: Vec::new(),
                });
                Ok(())
            }
            Pack::Tight(_) => Err(error::Pack::AlreadyTightPack(type_name::<T>())),
            Pack::Loose(_) => Err(error::Pack::AlreadyLoosePack(type_name::<T>())),
            Pack::Update(_) => Ok(()),
        }
    }
    /// Update packs this storage making it track *inserted*, *modified* and *deleted* components.  
    /// Does nothing if the storage is already update packed.
    /// Unwraps errors.  
    ///
    /// ### Errors
    ///
    /// - Storage is already tightly or loosly packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn update_pack(&mut self)
    where
        T: 'static,
    {
        self.try_update_pack().unwrap()
    }
}

impl<T> SparseSet<T> {
    /// Reserves memory for at least `additional` components. Adding components can still allocate though.
    pub fn reserve(&mut self, additional: usize) {
        self.dense.reserve(additional);
        self.data.reserve(additional);
    }
    /// Deletes all components in this storage.
    pub fn clear(&mut self) {
        for &id in &self.dense {
            unsafe {
                self.sparse.set_sparse_index_unchecked(id, core::usize::MAX);
            }
        }
        match &mut self.metadata.pack {
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
    /// Shares `owned`'s component with `shared` entity.  
    /// Deleting `owned`'s component won't stop the sharing.  
    /// Trying to share an entity with itself won't do anything.
    ///
    /// ### Errors
    ///
    /// - `entity` already had a owned component of this type.
    pub fn try_share(&mut self, owned: EntityId, shared: EntityId) -> Result<(), error::Share> {
        self.sparse.allocate_at(shared);

        if owned != shared {
            if !self.contains_owned(shared) {
                unsafe {
                    self.sparse
                        .set_sparse_index_unchecked(shared, shared.gen() as usize);

                    self.metadata
                        .shared
                        .set_sparse_index_unchecked(shared, owned);
                }

                Ok(())
            } else {
                Err(error::Share)
            }
        } else {
            Ok(())
        }
    }
    /// Shares `owned`'s component with `shared` entity.  
    /// Deleting `owned`'s component won't stop the sharing.  
    /// Trying to share an entity with itself won't do anything.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - `entity` already had a owned component of this type.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn share(&mut self, owned: EntityId, shared: EntityId) {
        self.try_share(owned, shared).unwrap()
    }
    /// Makes `entity` stop observing another entity.
    ///
    /// ### Errors
    ///
    /// - `entity` was not observing any entity.
    pub fn try_unshare(&mut self, entity: EntityId) -> Result<(), error::Unshare> {
        if self.contains_shared(entity) {
            unsafe {
                self.sparse
                    .set_sparse_index_unchecked(entity, core::usize::MAX);
                self.metadata
                    .shared
                    .set_sparse_index_unchecked(entity, EntityId::dead());
            }

            Ok(())
        } else {
            Err(error::Unshare)
        }
    }
    /// Makes `entity` stop observing another entity.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - `entity` was not observing any entity.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn unshare(&mut self, entity: EntityId) {
        self.try_unshare(entity).unwrap()
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.
    ///
    /// ### Errors
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    pub fn try_apply<R, F: FnOnce(&mut T, &T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> Result<R, error::Apply> {
        let mut a_index = self
            .index_of(a)
            .ok_or_else(|| error::Apply::MissingComponent(a))?;
        let b_index = self
            .index_of(b)
            .ok_or_else(|| error::Apply::MissingComponent(b))?;

        if a_index != b_index {
            if let Pack::Update(update) = &mut self.metadata.pack {
                let non_mut = update.first_non_mut();

                if a_index >= non_mut {
                    let non_mut_id = unsafe { *self.dense.get_unchecked(non_mut) };

                    self.dense.swap(a_index, non_mut);
                    self.data.swap(a_index, non_mut);

                    unsafe {
                        self.sparse.set_sparse_index_unchecked(a, non_mut);
                        self.sparse.set_sparse_index_unchecked(non_mut_id, a_index);
                    }

                    update.modified += 1;
                    a_index = non_mut;
                }
            }

            if a_index < b_index {
                let (first, second) = self.data.split_at_mut(a_index + 1);

                Ok(f(unsafe { first.get_unchecked_mut(a_index) }, unsafe {
                    second.get_unchecked(b_index - a_index - 1)
                }))
            } else {
                let (first, second) = self.data.split_at_mut(b_index + 1);

                Ok(f(unsafe { first.get_unchecked_mut(b_index) }, unsafe {
                    second.get_unchecked(a_index - b_index - 1)
                }))
            }
        } else {
            Err(error::Apply::IdenticalIds)
        }
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn apply<R, F: FnOnce(&mut T, &T) -> R>(&mut self, a: EntityId, b: EntityId, f: F) -> R {
        self.try_apply(a, b, f).unwrap()
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.
    ///
    /// ### Errors
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    pub fn try_apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> Result<R, error::Apply> {
        let mut a_index = self
            .index_of(a)
            .ok_or_else(|| error::Apply::MissingComponent(a))?;
        let mut b_index = self
            .index_of(b)
            .ok_or_else(|| error::Apply::MissingComponent(b))?;

        if a_index != b_index {
            if let Pack::Update(update) = &mut self.metadata.pack {
                let mut non_mut = update.first_non_mut();

                if a_index >= non_mut {
                    let non_mut_id = unsafe { *self.dense.get_unchecked(non_mut) };

                    self.dense.swap(a_index, non_mut);
                    self.data.swap(a_index, non_mut);

                    unsafe {
                        self.sparse.set_sparse_index_unchecked(a, non_mut);
                        self.sparse.set_sparse_index_unchecked(non_mut_id, a_index);
                    }

                    update.modified += 1;
                    a_index = non_mut;
                    non_mut += 1;
                }

                if b_index >= non_mut {
                    let non_mut_id = unsafe { *self.dense.get_unchecked(non_mut) };

                    self.dense.swap(b_index, non_mut);
                    self.data.swap(b_index, non_mut);

                    unsafe {
                        self.sparse.set_sparse_index_unchecked(b, non_mut);
                        self.sparse.set_sparse_index_unchecked(non_mut_id, b_index);
                    }

                    update.modified += 1;
                    b_index = non_mut;
                }
            }

            if a_index < b_index {
                let (first, second) = self.data.split_at_mut(a_index + 1);

                Ok(f(unsafe { first.get_unchecked_mut(a_index) }, unsafe {
                    second.get_unchecked_mut(b_index - a_index - 1)
                }))
            } else {
                let (first, second) = self.data.split_at_mut(b_index + 1);

                Ok(f(unsafe { first.get_unchecked_mut(b_index) }, unsafe {
                    second.get_unchecked_mut(a_index - b_index - 1)
                }))
            }
        } else {
            Err(error::Apply::IdenticalIds)
        }
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R {
        self.try_apply_mut(a, b, f).unwrap()
    }
}

#[cfg(feature = "serde1")]
impl<T: serde::Serialize + for<'de> serde::Deserialize<'de> + 'static> SparseSet<T> {
    /// Setup serialization for this storage.  
    /// Needs to be called for a storage to be serialized.
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn setup_serde(&mut self, _ser_config: SerConfig) {
        self.metadata.serde = Some(SerdeInfos::new());
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

        storage_to_unpack.reserve(self.metadata.observer_types.len());

        let mut i = 0;
        for observer in self.metadata.observer_types.iter().copied() {
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
    #[cfg(feature = "serde1")]
    fn is_serializable(&self) -> bool {
        self.metadata.serde.is_some()
    }
    #[cfg(feature = "serde1")]
    fn skip_serialization(&self, _: GlobalSerConfig) -> bool {
        false
    }
    #[cfg(feature = "serde1")]
    fn serialize(
        &self,
        ser_config: GlobalSerConfig,
        serializer: &mut dyn crate::erased_serde::Serializer,
    ) -> crate::erased_serde::Result<crate::erased_serde::Ok> {
        (self.metadata.serde.as_ref().unwrap().serialization)(self, ser_config, serializer)
    }
    #[cfg(feature = "serde1")]
    fn deserialize(
        &self,
    ) -> Option<
        fn(
            GlobalDeConfig,
            &mut dyn crate::erased_serde::Deserializer<'_>,
        ) -> Result<crate::storage::Storage, crate::erased_serde::Error>,
    > {
        Some(self.metadata.serde.as_ref()?.deserialization)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum OldComponent<T> {
    Owned(T),
    OldGenOwned(T),
    Shared,
    OldGenShared,
}

impl<T> OldComponent<T> {
    /// Extracts the value inside `OldComponent`.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    pub fn unwrap_owned(self) -> T {
        match self {
            Self::Owned(component) => component,
            Self::OldGenOwned(_) => {
                panic!("Called `OldComponent::unwrap_owned` on a `OldGenOwned` variant")
            }
            Self::Shared => panic!("Called `OldComponent::unwrap_owned` on a `Shared` variant"),
            Self::OldGenShared => {
                panic!("Called `OldComponent::unwrap_owned` on a `OldShared` variant")
            }
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
