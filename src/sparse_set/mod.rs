mod add_component;
mod bulk_add_entity;
mod delete_component;
mod metadata;
mod remove;
pub mod sort;
mod sparse_array;
mod window;

pub(crate) use add_component::AddComponent;
pub(crate) use bulk_add_entity::BulkAddEntity;
pub(crate) use delete_component::DeleteComponent;
pub(crate) use metadata::Metadata;
pub(crate) use remove::Remove;
pub(crate) use sparse_array::SparseArray;
pub(crate) use window::FullRawWindowMut;

use crate::storage::AllStorages;
use crate::storage::EntityId;
use crate::unknown_storage::UnknownStorage;
use alloc::vec::Vec;

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
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn deleted(&self) -> &[(EntityId, T)] {
        if let Some(update) = &self.metadata.update {
            &update.deleted
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Returns the ids of *removed* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn removed(&self) -> &[EntityId] {
        if let Some(update) = &self.metadata.update {
            &update.removed
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Returns the ids of *removed* or *deleted* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        if let Some(update) = &self.metadata.update {
            update
                .removed
                .iter()
                .copied()
                .chain(update.deleted.iter().map(|(id, _)| id).copied())
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Takes ownership of the *deleted* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        if let Some(update) = &mut self.metadata.update {
            let mut vec = Vec::with_capacity(update.deleted.capacity());

            core::mem::swap(&mut vec, &mut update.deleted);

            vec
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Takes ownership of the ids of *removed* components of an update packed storage.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        if let Some(update) = &mut self.metadata.update {
            let mut vec = Vec::with_capacity(update.removed.capacity());

            core::mem::swap(&mut vec, &mut update.removed);

            vec
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Takes ownership of the *removed* and *deleted* components of an update packed storage.  
    /// Unmraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    pub fn take_removed_and_deleted(&mut self) -> (Vec<EntityId>, Vec<(EntityId, T)>) {
        (self.take_removed(), self.take_deleted())
    }
    /// Moves all component in the *inserted* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn clear_inserted(&mut self) {
        if self.metadata.update.is_some() {
            for id in &mut *self.dense {
                if id.is_inserted() {
                    id.clear_meta();
                }
            }
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Moves all component in the *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn clear_modified(&mut self) {
        if self.metadata.update.is_some() {
            for id in &mut *self.dense {
                if id.is_modified() {
                    id.clear_meta();
                }
            }
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Moves all component in the *inserted* and *modified* section of an update packed storage to the *neutral* section.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - Storage isn't update packed.
    #[track_caller]
    #[inline]
    pub fn clear_inserted_and_modified(&mut self) {
        if self.metadata.update.is_some() {
            for id in &mut *self.dense {
                id.clear_meta();
            }
        } else {
            panic!("The storage isn't update packed. Use `view.update_pack()` to pack it.");
        }
    }
    /// Update packs this storage making it track *inserted*, *modified*, *removed* and *deleted* components.  
    /// Does nothing if the storage is already update packed.
    #[inline]
    pub fn update_pack(&mut self) {
        self.metadata.update.get_or_insert_with(Default::default);
    }

    /// Returns `true` if the `SparseSet` is update packed.
    #[inline]
    pub fn is_update_packed(&self) -> bool {
        self.metadata.update.is_some()
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
    /// Unwraps errors.
    ///
    /// ### Errors
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

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
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

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
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
    // /// Registers a callback triggered when a component is inserted and run immediately.
    // ///
    // /// Callbacks will run one after the other based on the order they were added.
    // /// They will run after the component is already in the `SparseSet`.
    // /// Inserting components to an entity that already owns a component in this storage will not trigger `on_insert` event.
    // #[inline]
    // fn on_insert(&mut self, f: fn(EntityId, &mut Self)) {
    //     self.metadata.local_on_insert.push(f);
    // }
    // /// Registers a callback triggered when a component is inserted and run when `ViewMut` is dropped.
    // ///
    // /// Callbacks will run one after the other based on the order they were added.
    // /// `on_insert_global` callbacks run before `on_remove_global`.
    // /// It is not possible to remove unique storages inside a global callback.
    // #[inline]
    // fn on_insert_global(&mut self, f: fn(EntityId, &mut Self, &AllStorages)) {
    //     self.metadata.global_on_insert.push(f);
    // }
    // /// Registers a callback triggered when a component is removed or deleted and run immediately.
    // ///
    // /// Callbacks will run one after the other based on the order they were added.
    // /// They will run before the component is removed from the `SparseSet`.
    // #[inline]
    // fn on_remove(&mut self, f: fn(EntityId, &mut Self)) {
    //     self.metadata.local_on_remove.push(f);
    // }
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
