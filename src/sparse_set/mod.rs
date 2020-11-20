mod add_component;
mod bulk_add_entity;
mod delete_component;
mod metadata;
mod remove;
pub mod sort;
mod sparse_array;
mod window;
// #[cfg(feature = "serde1")]
// mod deser;

pub(crate) use add_component::AddComponent;
pub(crate) use bulk_add_entity::BulkAddEntity;
pub(crate) use delete_component::DeleteComponent;
pub(crate) use metadata::Metadata;
pub(crate) use remove::Remove;
pub(crate) use sparse_array::SparseArray;
pub(crate) use window::FullRawWindowMut;
// #[cfg(feature = "serde1")]
// pub(crate) use deser::SparseSetSerializer;
// #[cfg(feature = "serde1")]
// use hashbrown::HashMap;
// #[cfg(feature = "serde1")]
// pub(crate) use metadata::SerdeInfos;

use crate::error;
use crate::storage::AllStorages;
use crate::storage::EntityId;
use crate::unknown_storage::UnknownStorage;
#[cfg(all(not(feature = "std"), feature = "serde1"))]
use alloc::string::ToString;
use alloc::vec::Vec;
// #[cfg(feature = "serde1")]
// use alloc::borrow::Cow;
// #[cfg(feature = "serde1")]
// use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig, SerConfig};
// #[cfg(feature = "serde1")]
// use deser::SparseSetDeserializer;

pub(crate) const BUCKET_SIZE: usize = 256 / core::mem::size_of::<usize>();

/// `SparseSet` component storage.
// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.

// Inserted and modified info is only present in dense
pub struct SparseSet<T> {
    pub(crate) sparse: SparseArray<[EntityId; BUCKET_SIZE]>,
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) metadata: Metadata<T>,
}

impl<T> SparseSet<T> {
    #[inline]
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: SparseArray::new(),
            dense: Vec::new(),
            data: Vec::new(),
            metadata: Default::default(),
        }
    }
    #[inline]
    pub(crate) fn full_raw_window_mut(&mut self) -> FullRawWindowMut<'_, T> {
        FullRawWindowMut::new(self)
    }
    /// Returns a slice of all the components in this storage.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

impl<T> SparseSet<T> {
    /// Returns `true` if `entity` owns a component in this storage.
    #[inline]
    pub fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }
    /// Returns the length of the storage.
    #[inline]
    pub fn len(&self) -> usize {
        self.dense.len()
    }
    /// Returns true if the storage's length is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }
}

impl<T> SparseSet<T> {
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    #[inline]
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
        self.sparse.get(entity).and_then(|sparse_entity| {
            if entity.gen() == sparse_entity.gen() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        })
    }
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this storage and until a modification happens.
    ///
    /// # Safety
    ///
    /// `entity` has to own a component of this type.  
    /// The index is only valid until a modification occurs in the storage.
    #[inline]
    pub unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        self.sparse.get_unchecked(entity).uindex()
    }
    /// Returns the `EntityId` at a given `index`.
    #[inline]
    pub fn id_at(&self, index: usize) -> Option<EntityId> {
        self.dense.get(index).copied()
    }
    #[inline]
    pub(crate) fn private_get(&self, entity: EntityId) -> Option<&T> {
        self.index_of(entity)
            .map(|index| unsafe { self.data.get_unchecked(index) })
    }
    #[inline]
    pub(crate) fn private_get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        let index = self.index_of(entity)?;

        if self.metadata.update.is_some() {
            unsafe {
                let dense_entity = self.dense.get_unchecked_mut(index);

                if !dense_entity.is_inserted() {
                    dense_entity.set_modified();
                }
            }
        }

        Some(unsafe { self.data.get_unchecked_mut(index) })
    }
}

impl<T> SparseSet<T> {
    /// Inserts `value` in the `SparseSet`.
    ///
    /// # Update pack
    ///
    /// In case `entity` had a component of this type, the new component will be considered `modified`.  
    /// In all other cases it'll be considered `inserted`.
    pub(crate) fn insert(&mut self, mut entity: EntityId, value: T) -> Option<T> {
        self.sparse.allocate_at(entity);

        // at this point there can't be nothing at the sparse index
        let sparse_entity = unsafe { self.sparse.get_mut_unchecked(entity) };

        let old_component;

        if sparse_entity.is_dead() {
            *sparse_entity =
                EntityId::new_from_parts(self.dense.len() as u64, entity.gen() as u16, 0);

            if self.metadata.update.is_some() {
                entity.set_inserted();
            } else {
                entity.clear_meta();
            }

            self.dense.push(entity);
            self.data.push(value);

            self.run_on_insert(entity);

            old_component = None;
        } else if entity.gen() >= sparse_entity.gen() {
            let old_data = unsafe {
                core::mem::replace(self.data.get_unchecked_mut(sparse_entity.uindex()), value)
            };

            if entity.gen() == sparse_entity.gen() {
                old_component = Some(old_data);
            } else {
                old_component = None;
            }

            sparse_entity.copy_gen(entity);

            let dense_entity = unsafe { self.dense.get_unchecked_mut(sparse_entity.uindex()) };

            if self.metadata.update.is_some() && !dense_entity.is_inserted() {
                dense_entity.set_modified();
            }

            dense_entity.copy_index_gen(entity);
        } else {
            old_component = None;
        }

        old_component
    }
}

impl<T> SparseSet<T> {
    /// Removes `entity`'s component from this storage.
    #[inline]
    pub fn remove(&mut self, entity: EntityId) -> Option<T>
    where
        T: 'static,
    {
        let component = self.actual_remove(entity);

        if let Some(update) = &mut self.metadata.update {
            if component.is_some() {
                update.removed.push(entity);
            }
        }

        component
    }
    #[inline]
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<T> {
        let sparse_entity = self.sparse.get(entity)?;

        if entity.gen() >= sparse_entity.gen() {
            self.run_on_remove(entity);

            let sparse_entity = self.sparse.get(entity)?;

            unsafe {
                *self.sparse.get_mut_unchecked(entity) = EntityId::dead();
            }

            self.dense.swap_remove(sparse_entity.uindex());
            let component = self.data.swap_remove(sparse_entity.uindex());

            unsafe {
                let last = *self.dense.get_unchecked(sparse_entity.uindex());
                self.sparse
                    .get_mut_unchecked(last)
                    .copy_index(sparse_entity);
            }

            if entity.gen() == sparse_entity.gen() {
                Some(component)
            } else {
                None
            }
        } else {
            None
        }
    }
    /// Deletes `entity`'s component from this storage.
    #[inline]
    pub fn delete(&mut self, entity: EntityId) -> bool
    where
        T: 'static,
    {
        if let Some(component) = self.actual_remove(entity) {
            if let Some(update) = &mut self.metadata.update {
                update.deleted.push((entity, component));
            }

            true
        } else {
            false
        }
    }
}

impl<T> SparseSet<T> {
    /// Returns the *deleted* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Some(update) = &self.metadata.update {
            Ok(&update.deleted)
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
    #[track_caller]
    #[inline]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        match self.try_deleted() {
            Ok(deleted) => deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the ids of *removed* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_removed(&self) -> Result<&[EntityId], error::NotUpdatePack> {
        if let Some(update) = &self.metadata.update {
            Ok(&update.removed)
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
    #[track_caller]
    #[inline]
    pub fn removed(&self) -> &[EntityId] {
        match self.try_removed() {
            Ok(removed) => removed,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Returns the ids of *removed* or *deleted* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_removed_or_deleted(
        &self,
    ) -> Result<impl Iterator<Item = EntityId> + '_, error::NotUpdatePack> {
        if let Some(update) = &self.metadata.update {
            Ok(update
                .removed
                .iter()
                .copied()
                .chain(update.deleted.iter().map(|(id, _)| id).copied()))
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Returns the ids of *removed* or *deleted* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        match self.try_removed_or_deleted() {
            Ok(removed_or_deleted) => removed_or_deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Takes ownership of the *deleted* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        if let Some(update) = &mut self.metadata.update {
            let mut vec = Vec::with_capacity(update.deleted.capacity());

            core::mem::swap(&mut vec, &mut update.deleted);

            Ok(vec)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Takes ownership of the *deleted* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        match self.try_take_deleted() {
            Ok(deleted) => deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_take_removed(&mut self) -> Result<Vec<EntityId>, error::NotUpdatePack> {
        if let Some(update) = &mut self.metadata.update {
            let mut vec = Vec::with_capacity(update.removed.capacity());

            core::mem::swap(&mut vec, &mut update.removed);

            Ok(vec)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        match self.try_take_removed() {
            Ok(removed) => removed,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Takes ownership of the *removed* and *deleted* components of an update packed storage.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[allow(clippy::type_complexity)]
    pub fn try_take_removed_and_deleted(
        &mut self,
    ) -> Result<(Vec<EntityId>, Vec<(EntityId, T)>), error::NotUpdatePack> {
        Ok((self.try_take_removed()?, self.try_take_deleted().unwrap()))
    }
    /// Takes ownership of the *removed* and *deleted* components of an update packed storage.  
    /// Unmraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn take_removed_and_deleted(&mut self) -> (Vec<EntityId>, Vec<(EntityId, T)>) {
        match self.try_take_removed_and_deleted() {
            Ok(removed_and_deleted) => removed_and_deleted,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        if self.metadata.update.is_some() {
            for id in &mut *self.dense {
                if id.is_inserted() {
                    id.clear_meta();
                }
            }

            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn clear_inserted(&mut self) {
        match self.try_clear_inserted() {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if self.metadata.update.is_some() {
            for id in &mut *self.dense {
                if id.is_modified() {
                    id.clear_meta();
                }
            }

            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn clear_modified(&mut self) {
        match self.try_clear_modified() {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[inline]
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if self.metadata.update.is_some() {
            for id in &mut *self.dense {
                id.clear_meta();
            }

            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn clear_inserted_and_modified(&mut self) {
        match self.try_clear_inserted_and_modified() {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Update packs this storage making it track *inserted*, *modified*, *removed* and *deleted* components.  
    /// Does nothing if the storage is already update packed.
    #[inline]
    pub fn update_pack(&mut self) {
        self.metadata.update.get_or_insert_with(Default::default);
    }
}

impl<T> SparseSet<T> {
    /// Reserves memory for at least `additional` components. Adding components can still allocate though.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.dense.reserve(additional);
        self.data.reserve(additional);
    }
    /// Deletes all components in this storage.
    pub fn clear(&mut self) {
        for &id in &self.dense {
            unsafe {
                *self.sparse.get_mut_unchecked(id) = EntityId::dead();
            }
        }

        if let Some(update) = &mut self.metadata.update {
            update
                .deleted
                .extend(self.dense.drain(..).zip(self.data.drain(..)));
        }

        self.dense.clear();
        self.data.clear();
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.
    ///
    /// ### Errors
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[inline]
    pub fn try_apply<R, F: FnOnce(&mut T, &T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> Result<R, error::Apply> {
        let a_index = self
            .index_of(a)
            .ok_or(error::Apply::MissingComponent(a))?;
        let b_index = self
            .index_of(b)
            .ok_or(error::Apply::MissingComponent(b))?;

        if a_index != b_index {
            if self.metadata.update.is_some() {
                unsafe {
                    let a_dense = self.dense.get_unchecked_mut(a_index);

                    if !a_dense.is_inserted() {
                        a_dense.set_modified();
                    }
                }
            }

            let a = unsafe { &mut *self.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &*self.data.as_mut_ptr().add(b_index) };

            Ok(f(a, b))
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
    #[inline]
    pub fn apply<R, F: FnOnce(&mut T, &T) -> R>(&mut self, a: EntityId, b: EntityId, f: F) -> R {
        match self.try_apply(a, b, f) {
            Ok(result) => result,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.
    ///
    /// ### Errors
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[inline]
    pub fn try_apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> Result<R, error::Apply> {
        let a_index = self
            .index_of(a)
            .ok_or(error::Apply::MissingComponent(a))?;
        let b_index = self
            .index_of(b)
            .ok_or(error::Apply::MissingComponent(b))?;

        if a_index != b_index {
            if self.metadata.update.is_some() {
                unsafe {
                    let a_dense = self.dense.get_unchecked_mut(a_index);

                    if !a_dense.is_inserted() {
                        a_dense.set_modified();
                    }

                    let b_dense = self.dense.get_unchecked_mut(b_index);
                    if !b_dense.is_inserted() {
                        b_dense.set_modified();
                    }
                }
            }

            let a = unsafe { &mut *self.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &mut *self.data.as_mut_ptr().add(b_index) };

            Ok(f(a, b))
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
    #[inline]
    pub fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R {
        match self.try_apply_mut(a, b, f) {
            Ok(result) => result,
            Err(err) => panic!("{:?}", err),
        }
    }
}

impl<T> SparseSet<T> {
    #[inline]
    fn run_on_insert(&mut self, entity: EntityId) {
        self.schedule_insert_global(entity);
        let mut i = 0;

        if i < self.metadata.local_on_insert.len() {
            let f = unsafe { self.metadata.local_on_insert.get_unchecked(i) };
            (f)(entity, self);
            i += 1;
        }

        let _ = i;
    }
    pub(crate) fn run_on_insert_global(&mut self, all_storages: &AllStorages) {
        let mut i = 0;
        let mut current = 0;
        let end = self.metadata.on_insert_ids_dense.len();

        while i < self.metadata.global_on_insert.len() {
            let f = unsafe { *self.metadata.global_on_insert.get_unchecked(i) };

            while current < end {
                let entity = unsafe { *self.metadata.on_insert_ids_dense.get_unchecked(current) };

                f(entity, self, all_storages);
                current += 1;
            }

            current = 0;
            i += 1;
        }

        for entity in self.metadata.on_insert_ids_dense.drain(0..end) {
            unsafe {
                *self.metadata.on_insert_ids_sparse.get_mut_unchecked(entity) = EntityId::dead();
            }
        }
    }
    #[inline]
    fn run_on_remove(&mut self, entity: EntityId) {
        self.schedule_remove_global(entity);

        let mut i = 0;

        if i < self.metadata.local_on_remove.len() {
            let f = unsafe { self.metadata.local_on_remove.get_unchecked(i) };
            (f)(entity, self);
            i += 1;
        }

        let _ = i;
    }
    pub(crate) fn run_on_remove_global(&mut self, all_storages: &AllStorages) {
        let mut i = 0;
        let mut current = 0;
        let end = self.metadata.on_remove_ids_dense.len();

        while i < self.metadata.global_on_remove.len() {
            let f = unsafe { *self.metadata.global_on_remove.get_unchecked(i) };

            while current < end {
                let entity = unsafe { *self.metadata.on_remove_ids_dense.get_unchecked(current) };

                f(entity, self, all_storages);
                current += 1;
            }

            current = 0;
            i += 1;
        }

        for entity in self.metadata.on_remove_ids_dense.drain(0..end) {
            unsafe {
                *self.metadata.on_remove_ids_sparse.get_mut_unchecked(entity) = EntityId::dead();
            }
        }
    }
    /// Registers a callback triggered when a component is inserted and run immediately.
    ///
    /// Callbacks will run one after the other based on the order they were added.  
    /// They will run after the component is already in the `SparseSet`.  
    /// Inserting components to an entity that already owns a component in this storage will not trigger `on_insert` event.
    #[inline]
    pub fn on_insert(&mut self, f: fn(EntityId, &mut Self)) {
        self.metadata.local_on_insert.push(f);
    }
    // /// Registers a callback triggered when a component is inserted and run when `ViewMut` is dropped.
    // ///
    // /// Callbacks will run one after the other based on the order they were added.
    // /// `on_insert_global` callbacks run before `on_remove_global`.
    // /// It is not possible to remove unique storages inside a global callback.
    // #[inline]
    // fn on_insert_global(&mut self, f: fn(EntityId, &mut Self, &AllStorages)) {
    //     self.metadata.global_on_insert.push(f);
    // }
    /// Registers a callback triggered when a component is removed or deleted and run immediately.
    ///
    /// Callbacks will run one after the other based on the order they were added.  
    /// They will run before the component is removed from the `SparseSet`.  
    #[inline]
    pub fn on_remove(&mut self, f: fn(EntityId, &mut Self)) {
        self.metadata.local_on_remove.push(f);
    }
    // /// Registers a callback triggered when a component is removed or deleted and run when `ViewMut` is dropped or when deleting components using `AllStorages`.
    // ///
    // /// Callbacks will run one after the other based on the order they were added.
    // /// `on_remove_global` callbacks run after `on_insert_global`.
    // /// It is not possible to remove unique storages inside a global callback.
    // #[inline]
    // fn on_remove_global(&mut self, f: fn(EntityId, &mut Self, &AllStorages)) {
    //     self.metadata.global_on_remove.push(f);
    // }
    /// Schedules a `on_insert_global` event for `entity`.
    #[inline]
    fn schedule_insert_global(&mut self, entity: EntityId) {
        if !self.metadata.global_on_insert.is_empty() {
            self.metadata.on_insert_ids_sparse.allocate_at(entity);

            let id = unsafe { self.metadata.on_insert_ids_sparse.get_mut_unchecked(entity) };

            if id.is_dead() || entity.gen() > id.gen() {
                *id = EntityId::new_from_parts(
                    self.metadata.on_insert_ids_dense.len() as u64,
                    entity.gen() as u16,
                    0,
                );
                self.metadata.on_insert_ids_dense.push(entity);
            }
        }
    }
    /// Schedules a `on_remove_global` event for `entity`.
    #[inline]
    fn schedule_remove_global(&mut self, entity: EntityId) {
        if !self.metadata.global_on_remove.is_empty() {
            self.metadata.on_remove_ids_sparse.allocate_at(entity);

            let id = unsafe { self.metadata.on_remove_ids_sparse.get_mut_unchecked(entity) };

            if id.is_dead() || entity.gen() > id.gen() {
                *id = EntityId::new_from_parts(
                    self.metadata.on_remove_ids_dense.len() as u64,
                    entity.gen() as u16,
                    0,
                );
                self.metadata.on_remove_ids_dense.push(entity);
            }
        }
    }
}

// #[cfg(feature = "serde1")]
// impl<T: serde::Serialize + for<'de> serde::Deserialize<'de> + 'static> SparseSet<T> {
//     /// Setup serialization for this storage.
//     /// Needs to be called for a storage to be serialized.
//     #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
//     pub fn setup_serde(&mut self, ser_config: SerConfig) {
//         self.metadata.serde = Some(SerdeInfos::new(ser_config));
//     }
// }

impl<T> core::ops::Index<EntityId> for SparseSet<T> {
    type Output = T;
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.private_get(entity).unwrap()
    }
}

impl<T> core::ops::IndexMut<EntityId> for SparseSet<T> {
    #[inline]
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.private_get_mut(entity).unwrap()
    }
}

impl<T: 'static> UnknownStorage for SparseSet<T> {
    #[inline]
    fn delete(&mut self, entity: EntityId) {
        SparseSet::delete(self, entity);
    }
    #[inline]
    fn clear(&mut self) {
        <Self>::clear(self);
    }
    #[inline]
    fn has_remove_event_to_dispatch(&self) -> bool {
        !self.metadata.on_remove_ids_dense.is_empty()
    }
    #[inline]
    fn run_on_remove_global(&mut self, all_storages: &AllStorages) {
        self.run_on_remove_global(all_storages);
    }
    //     #[cfg(feature = "serde1")]
    //     fn should_serialize(&self, _: GlobalSerConfig) -> bool {
    //         self.metadata.serde.is_some()
    //     }
    //     #[cfg(feature = "serde1")]
    //     fn serialize_identifier(&self) -> Cow<'static, str> {
    //         self.metadata
    //             .serde
    //             .as_ref()
    //             .and_then(|serde| serde.identifier.as_ref())
    //             .map(|identifier| identifier.0.clone())
    //             .unwrap_or("".into())
    //     }
    //     #[cfg(feature = "serde1")]
    //     fn serialize(
    //         &self,
    //         ser_config: GlobalSerConfig,
    //         serializer: &mut dyn crate::erased_serde::Serializer,
    //     ) -> crate::erased_serde::Result<crate::erased_serde::Ok> {
    //         (self.metadata.serde.as_ref().unwrap().serialization)(self, ser_config, serializer)
    //     }
    //     #[cfg(feature = "serde1")]
    //     fn deserialize(
    //         &self,
    //     ) -> Option<
    //         fn(
    //             GlobalDeConfig,
    //             &HashMap<EntityId, EntityId>,
    //             &mut dyn crate::erased_serde::Deserializer<'_>,
    //         ) -> Result<crate::storage::Storage, crate::erased_serde::Error>,
    //     > {
    //         Some(self.metadata.serde.as_ref()?.deserialization)
    //     }
}

#[test]
fn insert() {
    let mut array = SparseSet::new();

    assert!(array
        .insert(EntityId::new_from_parts(0, 0, 0), "0")
        .is_none());
    assert_eq!(array.dense, &[EntityId::new_from_parts(0, 0, 0)]);
    assert_eq!(array.data, &["0"]);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(0, 0, 0)),
        Some(&"0")
    );

    assert!(array
        .insert(EntityId::new_from_parts(1, 0, 0), "1")
        .is_none());
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(0, 0, 0),
            EntityId::new_from_parts(1, 0, 0)
        ]
    );
    assert_eq!(array.data, &["0", "1"]);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(0, 0, 0)),
        Some(&"0")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(1, 0, 0)),
        Some(&"1")
    );

    assert!(array
        .insert(EntityId::new_from_parts(5, 0, 0), "5")
        .is_none());
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(0, 0, 0),
            EntityId::new_from_parts(1, 0, 0),
            EntityId::new_from_parts(5, 0, 0)
        ]
    );
    assert_eq!(array.data, &["0", "1", "5"]);
    assert_eq!(
        array.private_get_mut(EntityId::new_from_parts(5, 0, 0)),
        Some(&mut "5")
    );

    assert_eq!(array.private_get(EntityId::new_from_parts(4, 0, 0)), None);
}

#[test]
fn remove() {
    let mut array = SparseSet::new();
    array.insert(EntityId::new_from_parts(0, 0, 0), "0");
    array.insert(EntityId::new_from_parts(5, 0, 0), "5");
    array.insert(EntityId::new_from_parts(10, 0, 0), "10");

    assert_eq!(array.remove(EntityId::new_from_parts(0, 0, 0)), Some("0"));
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0, 0),
            EntityId::new_from_parts(5, 0, 0)
        ]
    );
    assert_eq!(array.data, &["10", "5"]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0, 0)),
        Some(&"5")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0, 0)),
        Some(&"10")
    );

    array.insert(EntityId::new_from_parts(3, 0, 0), "3");
    array.insert(EntityId::new_from_parts(100, 0, 0), "100");
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0, 0),
            EntityId::new_from_parts(5, 0, 0),
            EntityId::new_from_parts(3, 0, 0),
            EntityId::new_from_parts(100, 0, 0)
        ]
    );
    assert_eq!(array.data, &["10", "5", "3", "100"]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(3, 0, 0)),
        Some(&"3")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0, 0)),
        Some(&"5")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0, 0)),
        Some(&"10")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(100, 0, 0)),
        Some(&"100")
    );

    assert_eq!(array.remove(EntityId::new_from_parts(3, 0, 0)), Some("3"));
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0, 0),
            EntityId::new_from_parts(5, 0, 0),
            EntityId::new_from_parts(100, 0, 0)
        ]
    );
    assert_eq!(array.data, &["10", "5", "100"]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
    assert_eq!(array.private_get(EntityId::new_from_parts(3, 0, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0, 0)),
        Some(&"5")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0, 0)),
        Some(&"10")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(100, 0, 0)),
        Some(&"100")
    );

    assert_eq!(
        array.remove(EntityId::new_from_parts(100, 0, 0)),
        Some("100")
    );
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0, 0),
            EntityId::new_from_parts(5, 0, 0)
        ]
    );
    assert_eq!(array.data, &["10", "5"]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
    assert_eq!(array.private_get(EntityId::new_from_parts(3, 0, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0, 0)),
        Some(&"5")
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0, 0)),
        Some(&"10")
    );
    assert_eq!(array.private_get(EntityId::new_from_parts(100, 0, 0)), None);
}
