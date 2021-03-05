mod add_component;
mod bulk_add_entity;
mod delete_component;
mod drain;
mod metadata;
mod remove;
mod sparse_array;
mod window;

pub use drain::SparseSetDrain;
pub use sparse_array::SparseArray;

pub(crate) use add_component::AddComponent;
pub(crate) use bulk_add_entity::BulkAddEntity;
pub(crate) use delete_component::DeleteComponent;
pub(crate) use metadata::Metadata;
pub(crate) use remove::Remove;
pub(crate) use window::FullRawWindowMut;

use crate::entity_id::EntityId;
use crate::memory_usage::StorageMemoryUsage;
use crate::storage::Storage;
use alloc::vec::Vec;
use core::cmp::{Ord, Ordering};

pub(crate) const BUCKET_SIZE: usize = 256 / core::mem::size_of::<EntityId>();

/// Default component storage.
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
            metadata: Metadata::new(),
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
    fn id_of(&self, entity: EntityId) -> Option<EntityId> {
        if let Some(index) = self.index_of(entity) {
            Some(unsafe { *self.dense.get_unchecked(index) })
        } else {
            None
        }
    }
    #[inline]
    pub(crate) fn private_get(&self, entity: EntityId) -> Option<&T> {
        self.index_of(entity)
            .map(|index| unsafe { self.data.get_unchecked(index) })
    }
    #[inline]
    pub(crate) fn private_get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        let index = self.index_of(entity)?;

        if self.metadata.track_modification {
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
    /// # Tracking
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

            if self.metadata.track_insertion {
                entity.set_inserted();
            } else {
                entity.clear_meta();
            }

            self.dense.push(entity);
            self.data.push(value);

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

            if self.metadata.track_modification && !dense_entity.is_inserted() {
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

        if component.is_some() {
            if let Some(removed) = &mut self.metadata.track_removal {
                removed.push(entity);
            }
        }

        component
    }
    #[inline]
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<T> {
        let sparse_entity = self.sparse.get(entity)?;

        if entity.gen() >= sparse_entity.gen() {
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
            if let Some(deleted) = &mut self.metadata.track_deletion {
                deleted.push((entity, component));
            }

            true
        } else {
            false
        }
    }
}

impl<T> SparseSet<T> {
    /// Returns `true` if `entity`'s component was inserted since the last [`clear_inserted`] or [`clear_all_inserted`] call.  
    /// Returns `false` if `entity` does not have a component in this storage.
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    /// [`clear_all_inserted`]: Self::clear_all_inserted
    #[inline]
    pub fn is_inserted(&self, entity: EntityId) -> bool {
        if let Some(id) = self.id_of(entity) {
            id.is_inserted()
        } else {
            false
        }
    }
    /// Returns `true` if `entity`'s component was modified since the last [`clear_modified`] or [`clear_all_modified`] call.  
    /// Returns `false` if `entity` does not have a component in this storage.
    ///
    /// [`clear_modified`]: Self::clear_modified
    /// [`clear_all_modified`]: Self::clear_all_modified
    #[inline]
    pub fn is_modified(&self, entity: EntityId) -> bool {
        if let Some(id) = self.id_of(entity) {
            id.is_modified()
        } else {
            false
        }
    }
    /// Returns `true` if `entity`'s component was inserted or modified since the last clear call.  
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted_or_modified(&self, entity: EntityId) -> bool {
        if let Some(id) = self.id_of(entity) {
            id.is_inserted() || id.is_modified()
        } else {
            false
        }
    }
    /// Returns the *deleted* components of a storage tracking deletion.
    ///
    /// ### Panics
    ///
    /// - Storage does not track deletion. Start tracking by calling [`track_deletion`] or [`track_all`].
    ///
    /// [`track_deletion`]: Self::track_deletion
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.metadata
            .track_deletion
            .as_deref()
            .expect("The storage does not track component deletion. Use `view_mut.track_deletion()` or `view_mut.track_all()` to start tracking.")
    }
    /// Returns the ids of *removed* components of a storage tracking removal.
    ///
    /// ### Panics
    ///
    /// - Storage does not track removal. Start tracking by calling [`track_removal`] or [`track_all`].
    ///
    /// [`track_removal`]: Self::track_removal
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn removed(&self) -> &[EntityId] {
        self.metadata
            .track_removal
            .as_deref()
            .expect("The storage does not track component removal. Use `view_mut.track_removal()` or `view_mut.track_all()` to start tracking.")
    }
    /// Returns the ids of *removed* or *deleted* components of a storage tracking removal and/or deletion.
    ///
    /// ### Panics
    ///
    /// - Storage does not track removal nor deletion. Start tracking by calling [`track_removal`], [`track_deletion`] or [`track_all`].
    ///
    /// [`track_removal`]: Self::track_removal
    /// [`track_deletion`]: Self::track_deletion
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        fn map_id<T>((id, _): &(EntityId, T)) -> EntityId {
            *id
        }

        match (
            self.metadata.track_removal.as_ref(),
            self.metadata.track_deletion.as_ref(),
        ) {
            (Some(removed), Some(deleted)) => {
                removed.iter().cloned().chain(deleted.iter().map(map_id))
            }
            (Some(removed), None) => removed.iter().cloned().chain([].iter().map(map_id)),
            (None, Some(deleted)) => [].iter().cloned().chain(deleted.iter().map(map_id)),
            (None, None) => {
                panic!("The storage does not track component removal nor deletion. Use `view_mut.track_removal()`, `view_mut.track_deletion()` or `view_mut.track_all()` to start tracking.")
            }
        }
    }
    /// Takes ownership of the *deleted* components of a storage tracking deletion.
    ///
    /// ### Panics
    ///
    /// - Storage does not track deletion. Start tracking by calling [`track_deletion`] or [`track_all`].
    ///
    /// [`track_deletion`]: Self::track_deletion
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.metadata
            .track_deletion
            .as_mut()
            .expect("The storage does not track component deletion. Use `view_mut.track_deletion()` or `view_mut.track_all()` to start tracking.")
            .drain(..).collect()
    }
    /// Takes ownership of the ids of *removed* components of a storage tracking removal.
    ///
    /// ### Panics
    ///
    /// - Storage does not track removal. Start tracking by calling [`track_removal`] or [`track_all`].
    ///
    /// [`track_removal`]: Self::track_removal
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        self.metadata
            .track_deletion
            .as_mut()
            .expect("The storage does not track component removal. Use `view_mut.track_removal()` or `view_mut.track_all()` to start tracking.")
            .drain(..)
            .map(|(id, _)| id)
            .collect()
    }
    /// Takes ownership of the *removed* and *deleted* components of a storage tracking removal and/or deletion.
    ///
    /// ### Panics
    ///
    /// - Storage does not track removal nor deletion. Start tracking by calling [`track_removal`], [`track_deletion`] or [`track_all`].
    ///
    /// [`track_removal`]: Self::track_removal
    /// [`track_deletion`]: Self::track_deletion
    /// [`track_all`]: Self::track_all
    pub fn take_removed_and_deleted(&mut self) -> (Vec<EntityId>, Vec<(EntityId, T)>) {
        match (
            self.metadata.track_removal.as_mut(),
            self.metadata.track_deletion.as_mut(),
        ) {
            (Some(removed), Some(deleted)) => {
                (removed.drain(..).collect(), deleted.drain(..).collect())
            }
            (Some(removed), None) => (removed.drain(..).collect(), Vec::new()),
            (None, Some(deleted)) => (Vec::new(), deleted.drain(..).collect()),
            (None, None) => {
                panic!("The storage does not track component removal nor deletion. Use `view_mut.track_removal()`, `view_mut.track_deletion()` or `view_mut.track_all()` to start tracking.")
            }
        }
    }
    /// Removes the *inserted* flag on `entity`'s component.
    ///
    /// ### Panics
    ///
    /// - Storage does not track insertion. Start tracking by calling [`track_insertion`] or [`track_all`].
    ///
    /// [`track_insertion`]: Self::track_insertion
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn clear_inserted(&mut self, entity: EntityId) {
        if self.metadata.track_insertion {
            if let Some(id) = self.sparse.get(entity) {
                let id = unsafe { self.dense.get_unchecked_mut(id.uindex()) };

                if id.is_inserted() {
                    id.clear_meta();
                }
            }
        } else {
            panic!("The storage does not track component insertion. Use `view_mut.track_insertion()` or `view_mut.track_all()` to start tracking.");
        }
    }
    /// Removes the *inserted* flag on all components of this storage.
    ///
    /// ### Panics
    ///
    /// - Storage does not track insertion. Start tracking by calling [`track_insertion`] or [`track_all`].
    ///
    /// [`track_insertion`]: Self::track_insertion
    /// [`track_all`]: Self::track_all
    #[track_caller]
    pub fn clear_all_inserted(&mut self) {
        if self.metadata.track_insertion {
            for id in &mut *self.dense {
                if id.is_inserted() {
                    id.clear_meta();
                }
            }
        } else {
            panic!("The storage does not track component insertion. Use `view_mut.track_insertion()` or `view_mut.track_all()` to start tracking.");
        }
    }
    /// Removes the *modified* flag on `entity`'s component.
    ///
    /// ### Panics
    ///
    /// - Storage does not track modification. Start tracking by calling [`track_modification`] or [`track_all`].
    ///
    /// [`track_modification`]: Self::track_modification
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn clear_modified(&mut self, entity: EntityId) {
        if self.metadata.track_modification {
            if let Some(id) = self.sparse.get(entity) {
                let id = unsafe { self.dense.get_unchecked_mut(id.uindex()) };

                if id.is_modified() {
                    id.clear_meta();
                }
            }
        } else {
            panic!("The storage does not track component modification. Use `view_mut.track_modification()` or `view_mut.track_all()` to start tracking.");
        }
    }
    /// Removes the *modified* flag on all components of this storage.
    ///
    /// ### Panics
    ///
    /// - Storage does not track modification. Start tracking by calling [`track_modification`] or [`track_all`].
    ///
    /// [`track_modification`]: Self::track_modification
    /// [`track_all`]: Self::track_all
    #[track_caller]
    pub fn clear_all_modified(&mut self) {
        if self.metadata.track_modification {
            for id in &mut *self.dense {
                if id.is_modified() {
                    id.clear_meta();
                }
            }
        } else {
            panic!("The storage does not track component modification. Use `view_mut.track_modification()` or `view_mut.track_all()` to start tracking.");
        }
    }
    /// Removes the *inserted* and *modified* flags on `entity`'s component.
    ///
    /// ### Panics
    ///
    /// - Storage does not track insertion not modification. Start tracking by calling [`track_insertion`], [`track_modification`] or [`track_all`].
    ///
    /// [`track_insertion`]: Self::track_insertion
    /// [`track_modification`]: Self::track_modification
    /// [`track_all`]: Self::track_all
    #[track_caller]
    #[inline]
    pub fn clear_inserted_and_modified(&mut self, entity: EntityId) {
        if !self.is_tracking_insertion() && !self.is_tracking_modification() {
            panic!("The storage does not track component insertion not modification. Use `view_mut.track_insertion()`, `view_mut.track_modification()` or `view_mut.track_all()` to start tracking.");
        }

        if let Some(id) = self.sparse.get(entity) {
            unsafe {
                self.dense.get_unchecked_mut(id.uindex()).clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on all components of this storage.
    ///
    /// ### Panics
    ///
    /// - Storage does not track insertion not modification. Start tracking by calling [`track_insertion`], [`track_modification`] or [`track_all`].
    ///
    /// [`track_insertion`]: Self::track_insertion
    /// [`track_modification`]: Self::track_modification
    /// [`track_all`]: Self::track_all
    #[track_caller]
    pub fn clear_all_inserted_and_modified(&mut self) {
        if !self.is_tracking_insertion() && !self.is_tracking_modification() {
            panic!("The storage does not track component insertion not modification. Use `view_mut.track_insertion()`, `view_mut.track_modification()` or `view_mut.track_all()` to start tracking.");
        }

        for id in &mut self.dense {
            id.clear_meta();
        }
    }
    /// Flags components when they are inserted.  
    /// To check the flag use [`is_inserted`], [`is_inserted_or_modified`] or iterate over the storage after calling [`inserted`].  
    /// To clear the flag use [`clear_inserted`] or [`clear_all_inserted`].
    ///
    /// [`is_inserted`]: Self::is_inserted()
    /// [`is_inserted_or_modified`]: Self::is_inserted_or_modified()
    /// [`inserted`]: crate::view::View::inserted()
    /// [`clear_inserted`]: Self::clear_inserted()
    /// [`clear_all_inserted`]: Self::clear_all_inserted()
    pub fn track_insertion(&mut self) -> &mut Self {
        self.metadata.track_insertion = true;

        self
    }
    /// Flags components when they are modified. Will not flag components already flagged inserted.  
    /// To check the flag use [`is_modified`], [`is_inserted_or_modified`] or iterate over the storage after calling [`modified`].  
    /// To clear the flag use [`clear_modified`] or [`clear_all_modified`].
    ///
    /// [`is_modified`]: Self::is_modified()
    /// [`is_inserted_or_modified`]: Self::is_inserted_or_modified()
    /// [`modified`]: crate::view::View::modified()
    /// [`clear_modified`]: Self::clear_modified()
    /// [`clear_all_modified`]: Self::clear_all_modified()
    pub fn track_modification(&mut self) -> &mut Self {
        self.metadata.track_modification = true;

        self
    }
    /// Stores components and their [`EntityId`] when they are deleted.  
    /// You can access them with [`deleted`] or [`removed_or_deleted`].  
    /// You can clear them and get back a `Vec` with [`take_deleted`] or [`take_removed_and_deleted`].
    ///
    /// [`EntityId`]: crate::entity_id::EntityId
    /// [`deleted`]: Self::deleted()
    /// [`removed_or_deleted`]: Self::removed_or_deleted()
    /// [`take_deleted`]: Self::take_deleted()
    /// [`take_removed_and_deleted`]: Self::take_removed_and_deleted()
    pub fn track_deletion(&mut self) -> &mut Self {
        if self.metadata.track_deletion.is_none() {
            self.metadata.track_deletion = Some(Vec::new());
        }

        self
    }
    /// Stores [`EntityId`] of deleted components.  
    /// You can access them with [`removed`] or [`removed_or_deleted`].  
    /// You can clear them and get back a `Vec` with [`take_removed`] or [`take_removed_and_deleted`].
    ///
    /// [`EntityId`]: crate::entity_id::EntityId
    /// [`removed`]: Self::removed()
    /// [`removed_or_deleted`]: Self::removed_or_deleted()
    /// [`take_removed`]: Self::take_removed()
    /// [`take_removed_and_deleted`]: Self::take_removed_and_deleted()
    pub fn track_removal(&mut self) -> &mut Self {
        if self.metadata.track_removal.is_none() {
            self.metadata.track_removal = Some(Vec::new());
        }

        self
    }
    /// Flags component insertion, modification, deletion and removal.  
    /// Same as calling [`track_insertion`], [`track_modification`], [`track_deletion`] and [`track_removal`].
    ///
    /// [`track_insertion`]: Self::track_insertion
    /// [`track_modification`]: Self::track_modification
    /// [`track_deletion`]: Self::track_deletion
    /// [`track_removal`]: Self::track_removal
    pub fn track_all(&mut self) {
        self.track_insertion();
        self.track_modification();
        self.track_deletion();
        self.track_removal();
    }
    /// Returns `true` if the storage tracks insertion.
    pub fn is_tracking_insertion(&self) -> bool {
        self.metadata.track_insertion
    }
    /// Returns `true` if the storage tracks modification.
    pub fn is_tracking_modification(&self) -> bool {
        self.metadata.track_modification
    }
    /// Returns `true` if the storage tracks deletion.
    pub fn is_tracking_deletion(&self) -> bool {
        self.metadata.track_deletion.is_some()
    }
    /// Returns `true` if the storage tracks removal.
    pub fn is_tracking_removal(&self) -> bool {
        self.metadata.track_removal.is_some()
    }
    /// Returns `true` if the storage tracks insertion, modification, deletion or removal.
    pub fn is_tracking_any(&self) -> bool {
        self.is_tracking_insertion()
            || self.is_tracking_modification()
            || self.is_tracking_deletion()
            || self.is_tracking_removal()
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

        if let Some(deleted) = &mut self.metadata.track_deletion {
            deleted.extend(self.dense.drain(..).zip(self.data.drain(..)));
        } else {
            self.dense.clear();
            self.data.clear();
        }
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.  
    /// Unwraps errors.
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    #[inline]
    pub fn apply<R, F: FnOnce(&mut T, &T) -> R>(&mut self, a: EntityId, b: EntityId, f: F) -> R {
        let a_index = self.index_of(a).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                a
            )
        });
        let b_index = self.index_of(b).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                b
            )
        });

        if a_index != b_index {
            if self.metadata.track_modification {
                unsafe {
                    let a_dense = self.dense.get_unchecked_mut(a_index);

                    if !a_dense.is_inserted() {
                        a_dense.set_modified();
                    }
                }
            }

            let a = unsafe { &mut *self.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &*self.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.  
    /// Unwraps errors.
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    #[inline]
    pub fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R {
        let a_index = self.index_of(a).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                a
            )
        });
        let b_index = self.index_of(b).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                b
            )
        });

        if a_index != b_index {
            if self.metadata.track_modification {
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

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }
    /// Creates a draining iterator that empties the storage and yields the removed items.
    pub fn drain(&mut self) -> SparseSetDrain<'_, T> {
        if let Some(removed) = &mut self.metadata.track_removal {
            removed.extend_from_slice(&self.dense);
        }

        for id in &self.dense {
            // SAFE ids from self.dense are always valid
            unsafe {
                *self.sparse.get_mut_unchecked(*id) = EntityId::dead();
            }
        }

        let dense_ptr = self.dense.as_ptr();
        let dense_len = self.dense.len();

        self.dense.clear();

        SparseSetDrain {
            dense_ptr,
            dense_len,
            data: self.data.drain(..),
        }
    }
    /// Sorts the `SparseSet` with a comparator function, but may not preserve the order of equal elements.
    pub fn sort_unstable_by<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        let mut transform: Vec<usize> = (0..self.dense.len()).collect();

        transform.sort_unstable_by(|&i, &j| {
            // SAFE dense and data have the same length
            compare(unsafe { self.data.get_unchecked(i) }, unsafe {
                self.data.get_unchecked(j)
            })
        });

        let mut pos;
        for i in 0..transform.len() {
            // SAFE we're in bound
            pos = unsafe { *transform.get_unchecked(i) };
            while pos < i {
                // SAFE we're in bound
                pos = unsafe { *transform.get_unchecked(pos) };
            }
            self.dense.swap(i, pos);
            self.data.swap(i, pos);
        }

        for (i, id) in self.dense.iter().enumerate() {
            unsafe {
                self.sparse.get_mut_unchecked(*id).set_index(i as u64);
            }
        }
    }
}

impl<T: Ord> SparseSet<T> {
    /// Sorts the `SparseSet`, but may not preserve the order of equal elements.
    pub fn sort_unstable(&mut self) {
        self.sort_unstable_by(Ord::cmp)
    }
}

impl<T> core::ops::Index<EntityId> for SparseSet<T> {
    type Output = T;
    #[track_caller]
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.private_get(entity).unwrap()
    }
}

impl<T> core::ops::IndexMut<EntityId> for SparseSet<T> {
    #[track_caller]
    #[inline]
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.private_get_mut(entity).unwrap()
    }
}

impl<T: 'static> Storage for SparseSet<T> {
    #[inline]
    fn delete(&mut self, entity: EntityId) {
        SparseSet::delete(self, entity);
    }
    #[inline]
    fn clear(&mut self) {
        <Self>::clear(self);
    }
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(StorageMemoryUsage {
            storage_name: core::any::type_name::<Self>().into(),
            allocated_memory_bytes: self.sparse.reserved_memory()
                + (self.dense.capacity() * core::mem::size_of::<EntityId>())
                + (self.data.capacity() * core::mem::size_of::<T>())
                + self.metadata.used_memory()
                + core::mem::size_of::<Self>(),
            used_memory_bytes: self.sparse.used_memory()
                + (self.dense.len() * core::mem::size_of::<EntityId>())
                + (self.data.len() * core::mem::size_of::<T>())
                + self.metadata.reserved_memory()
                + core::mem::size_of::<Self>(),
            component_count: self.len(),
        })
    }
    fn sparse_array(&self) -> Option<&SparseArray<[EntityId; 32]>> {
        Some(&self.sparse)
    }
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

#[test]
fn drain() {
    let mut sparse_set = SparseSet::new();

    sparse_set.insert(EntityId::new(0), 0);
    sparse_set.insert(EntityId::new(1), 1);

    let mut drain = sparse_set.drain();

    assert_eq!(drain.next(), Some(0));
    assert_eq!(drain.next(), Some(1));
    assert_eq!(drain.next(), None);

    drop(drain);

    assert_eq!(sparse_set.len(), 0);
    assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
}

#[test]
fn drain_with_id() {
    let mut sparse_set = SparseSet::new();

    sparse_set.insert(EntityId::new(0), 0);
    sparse_set.insert(EntityId::new(1), 1);

    let mut drain = sparse_set.drain().with_id();

    assert_eq!(drain.next(), Some((EntityId::new(0), 0)));
    assert_eq!(drain.next(), Some((EntityId::new(1), 1)));
    assert_eq!(drain.next(), None);

    drop(drain);

    assert_eq!(sparse_set.len(), 0);
    assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
}

#[test]
fn drain_empty() {
    let mut sparse_set = SparseSet::<u32>::new();

    assert_eq!(sparse_set.drain().next(), None);
    assert_eq!(sparse_set.drain().with_id().next(), None);

    assert_eq!(sparse_set.len(), 0);
}

#[test]
fn unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::new();

    for i in (0..100).rev() {
        let mut entity_id = crate::entity_id::EntityId::zero();
        entity_id.set_index(100 - i);
        array.insert(entity_id, i);
    }

    array.sort_unstable();

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..100 {
        let mut entity_id = crate::entity_id::EntityId::zero();
        entity_id.set_index(100 - i);
        assert_eq!(array.private_get(entity_id), Some(&i));
    }
}

#[test]
fn partially_sorted_unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::new();

    for i in 0..20 {
        let mut entity_id = crate::entity_id::EntityId::zero();
        entity_id.set_index(i);
        assert!(array.insert(entity_id, i).is_none());
    }
    for i in (20..100).rev() {
        let mut entity_id = crate::entity_id::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert!(array.insert(entity_id, i).is_none());
    }

    array.sort_unstable();

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..20 {
        let mut entity_id = crate::entity_id::EntityId::zero();
        entity_id.set_index(i);
        assert_eq!(array.private_get(entity_id), Some(&i));
    }
    for i in 20..100 {
        let mut entity_id = crate::entity_id::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert_eq!(array.private_get(entity_id), Some(&i));
    }
}
