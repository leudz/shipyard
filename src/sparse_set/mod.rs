mod add_component;
mod bulk_add_entity;
mod delete;
mod drain;
mod remove;
mod sparse_array;
mod window;

pub use drain::SparseSetDrain;
pub use sparse_array::SparseArray;

pub(crate) use add_component::AddComponent;
pub(crate) use bulk_add_entity::BulkAddEntity;
pub(crate) use delete::Delete;
pub(crate) use remove::Remove;
pub(crate) use window::FullRawWindowMut;

use crate::component::Component;
use crate::memory_usage::StorageMemoryUsage;
use crate::storage::Storage;
use crate::track;
use crate::{entity_id::EntityId, track::Tracking};
use alloc::vec::Vec;
use core::{
    cmp::{Ord, Ordering},
    fmt,
};

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
pub struct SparseSet<T: Component, Track: Tracking<T> = <T as Component>::Tracking> {
    pub(crate) sparse: SparseArray<EntityId, BUCKET_SIZE>,
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) deletion_data: Track::DeletionData,
    pub(crate) removal_data: Track::RemovalData,
}

impl<T: fmt::Debug + Component> fmt::Debug for SparseSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.dense.iter().zip(&self.data))
            .finish()
    }
}

impl<T: Component> SparseSet<T> {
    #[inline]
    pub(crate) fn new() -> Self {
        SparseSet {
            sparse: SparseArray::new(),
            dense: Vec::new(),
            data: Vec::new(),
            deletion_data: <T::Tracking as Tracking<T>>::DeletionData::default(),
            removal_data: <T::Tracking as Tracking<T>>::RemovalData::default(),
        }
    }
    #[inline]
    pub(crate) fn full_raw_window_mut(&mut self) -> FullRawWindowMut<'_, T, T::Tracking> {
        FullRawWindowMut::new(self)
    }
    /// Returns a slice of all the components in this storage.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

impl<T: Component> SparseSet<T> {
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

impl<T: Component> SparseSet<T> {
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
        self.index_of(entity)
            .map(|index| unsafe { *self.dense.get_unchecked(index) })
    }
    #[inline]
    pub(crate) fn private_get(&self, entity: EntityId) -> Option<&T> {
        self.index_of(entity)
            .map(|index| unsafe { self.data.get_unchecked(index) })
    }
    #[inline]
    pub(crate) fn private_get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        let index = self.index_of(entity)?;

        if T::Tracking::track_modification() {
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

impl<T: Component> SparseSet<T> {
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
                EntityId::new_from_index_and_gen(self.dense.len() as u64, entity.gen());

            if T::Tracking::track_insertion() {
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

            if T::Tracking::track_modification() && !dense_entity.is_inserted() {
                dense_entity.set_modified();
            }

            dense_entity.copy_index_gen(entity);
        } else {
            old_component = None;
        }

        old_component
    }
}

impl<T: Component> SparseSet<T> {
    /// Removes `entity`'s component from this storage.
    #[inline]
    pub fn remove(&mut self, entity: EntityId) -> Option<T> {
        T::Tracking::remove(self, entity)
    }
    /// Deletes `entity`'s component from this storage.
    #[inline]
    pub fn delete(&mut self, entity: EntityId) -> bool {
        T::Tracking::delete(self, entity)
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
}

impl<T: Component<Tracking = track::Insertion>> SparseSet<T, track::Insertion> {
    /// Removes the *inserted* flag on `entity`'s component.
    pub fn clear_inserted(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            let id = unsafe { self.dense.get_unchecked_mut(id.uindex()) };

            if id.is_inserted() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *inserted* flag on all components of this storage.
    pub fn clear_all_inserted(&mut self) {
        for id in &mut *self.dense {
            if id.is_inserted() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on `entity`'s component.
    #[inline]
    pub fn clear_inserted_and_modified(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            unsafe {
                self.dense.get_unchecked_mut(id.uindex()).clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on all components of this storage.
    pub fn clear_all_inserted_and_modified(&mut self) {
        for id in &mut self.dense {
            id.clear_meta();
        }
    }
}

impl<T: Component<Tracking = track::Modification>> SparseSet<T, track::Modification> {
    /// Removes the *modified* flag on `entity`'s component.
    #[inline]
    pub fn clear_modified(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            let id = unsafe { self.dense.get_unchecked_mut(id.uindex()) };

            if id.is_modified() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *modified* flag on all components of this storage.
    pub fn clear_all_modified(&mut self) {
        for id in &mut *self.dense {
            if id.is_modified() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on `entity`'s component.
    #[inline]
    pub fn clear_inserted_and_modified(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            unsafe {
                self.dense.get_unchecked_mut(id.uindex()).clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on all components of this storage.
    pub fn clear_all_inserted_and_modified(&mut self) {
        for id in &mut self.dense {
            id.clear_meta();
        }
    }
}

impl<T: Component<Tracking = track::Deletion>> SparseSet<T, track::Deletion> {
    /// Returns the *deleted* components of a storage tracking deletion.
    pub fn deleted(&self) -> &[(EntityId, T)] {
        &self.deletion_data
    }
    /// Returns the ids of *removed* or *deleted* components of a storage tracking removal and/or deletion.
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.deletion_data.iter().map(|(id, _)| *id)
    }
    /// Takes ownership of the *deleted* components of a storage tracking deletion.
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.deletion_data.drain(..).collect()
    }
    /// Takes ownership of the *removed* and *deleted* components of a storage tracking removal and/or deletion.
    pub fn take_removed_and_deleted(&mut self) -> (Vec<EntityId>, Vec<(EntityId, T)>) {
        (Vec::new(), self.deletion_data.drain(..).collect())
    }
}

impl<T: Component<Tracking = track::Removal>> SparseSet<T, track::Removal> {
    /// Returns the ids of *removed* components of a storage tracking removal.
    pub fn removed(&self) -> &[EntityId] {
        &self.removal_data
    }
    /// Returns the ids of *removed* or *deleted* components of a storage tracking removal and/or deletion.
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.removal_data.iter().copied()
    }
    /// Takes ownership of the ids of *removed* components of a storage tracking removal.
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        self.removal_data.drain(..).collect()
    }
    /// Takes ownership of the *removed* and *deleted* components of a storage tracking removal and/or deletion.
    pub fn take_removed_and_deleted(&mut self) -> (Vec<EntityId>, Vec<(EntityId, T)>) {
        (self.removal_data.drain(..).collect(), Vec::new())
    }
}

impl<T: Component<Tracking = track::All>> SparseSet<T, track::All> {
    /// Returns the *deleted* components of a storage tracking deletion.
    pub fn deleted(&self) -> &[(EntityId, T)] {
        &self.deletion_data
    }
    /// Returns the ids of *removed* components of a storage tracking removal.
    pub fn removed(&self) -> &[EntityId] {
        &self.removal_data
    }
    /// Returns the ids of *removed* or *deleted* components of a storage tracking removal and/or deletion.
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.deletion_data
            .iter()
            .map(|(id, _)| *id)
            .chain(self.removal_data.iter().copied())
    }
    /// Takes ownership of the *deleted* components of a storage tracking deletion.
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.deletion_data.drain(..).collect()
    }
    /// Takes ownership of the ids of *removed* components of a storage tracking removal.
    pub fn take_removed(&mut self) -> Vec<EntityId> {
        self.removal_data.drain(..).collect()
    }
    /// Takes ownership of the *removed* and *deleted* components of a storage tracking removal and/or deletion.
    pub fn take_removed_and_deleted(&mut self) -> (Vec<EntityId>, Vec<(EntityId, T)>) {
        (
            self.removal_data.drain(..).collect(),
            self.deletion_data.drain(..).collect(),
        )
    }
    /// Removes the *inserted* flag on `entity`'s component.
    pub fn clear_inserted(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            let id = unsafe { self.dense.get_unchecked_mut(id.uindex()) };

            if id.is_inserted() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *inserted* flag on all components of this storage.
    pub fn clear_all_inserted(&mut self) {
        for id in &mut *self.dense {
            if id.is_inserted() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *modified* flag on `entity`'s component.
    #[inline]
    pub fn clear_modified(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            let id = unsafe { self.dense.get_unchecked_mut(id.uindex()) };

            if id.is_modified() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *modified* flag on all components of this storage.
    pub fn clear_all_modified(&mut self) {
        for id in &mut *self.dense {
            if id.is_modified() {
                id.clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on `entity`'s component.
    #[inline]
    pub fn clear_inserted_and_modified(&mut self, entity: EntityId) {
        if let Some(id) = self.sparse.get(entity) {
            unsafe {
                self.dense.get_unchecked_mut(id.uindex()).clear_meta();
            }
        }
    }
    /// Removes the *inserted* and *modified* flags on all components of this storage.
    pub fn clear_all_inserted_and_modified(&mut self) {
        for id in &mut self.dense {
            id.clear_meta();
        }
    }
}

impl<T: Component> SparseSet<T> {
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
    /// Returns `true` if the storage tracks insertion.
    pub fn is_tracking_insertion(&self) -> bool {
        T::Tracking::track_insertion()
    }
    /// Returns `true` if the storage tracks modification.
    pub fn is_tracking_modification(&self) -> bool {
        T::Tracking::track_modification()
    }
    /// Returns `true` if the storage tracks removal.
    pub fn is_tracking_removal(&self) -> bool {
        T::Tracking::track_removal()
    }
    /// Returns `true` if the storage tracks insertion, modification, deletion or removal.
    pub fn is_tracking_any(&self) -> bool {
        self.is_tracking_insertion()
            || self.is_tracking_modification()
            || self.is_tracking_removal()
    }
}

impl<T: Component> SparseSet<T> {
    /// Reserves memory for at least `additional` components. Adding components can still allocate though.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.dense.reserve(additional);
        self.data.reserve(additional);
    }
    /// Deletes all components in this storage.
    pub fn clear(&mut self) {
        T::Tracking::clear(self);
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.  
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    #[inline]
    pub fn apply<R, F: FnOnce(&mut T, &T) -> R>(&mut self, a: EntityId, b: EntityId, f: F) -> R {
        T::Tracking::apply(self, a, b, f)
    }
    /// Applies the given function `f` to the entities `a` and `b`.  
    /// The two entities shouldn't point to the same component.  
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
        T::Tracking::apply_mut(self, a, b, f)
    }
    /// Creates a draining iterator that empties the storage and yields the removed items.
    pub fn drain(&mut self) -> SparseSetDrain<'_, T> {
        T::Tracking::drain(self)
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

impl<T: Ord + Component> SparseSet<T> {
    /// Sorts the `SparseSet`, but may not preserve the order of equal elements.
    pub fn sort_unstable(&mut self) {
        self.sort_unstable_by(Ord::cmp)
    }
}

impl<T: Component> core::ops::Index<EntityId> for SparseSet<T> {
    type Output = T;
    #[track_caller]
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.private_get(entity).unwrap()
    }
}

impl<T: Component> core::ops::IndexMut<EntityId> for SparseSet<T> {
    #[track_caller]
    #[inline]
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.private_get_mut(entity).unwrap()
    }
}

impl<T: 'static + Component> Storage for SparseSet<T> {
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
                + T::Tracking::used_memory(self)
                + core::mem::size_of::<Self>(),
            used_memory_bytes: self.sparse.used_memory()
                + (self.dense.len() * core::mem::size_of::<EntityId>())
                + (self.data.len() * core::mem::size_of::<T>())
                + T::Tracking::reserved_memory(self)
                + core::mem::size_of::<Self>(),
            component_count: self.len(),
        })
    }
    fn sparse_array(&self) -> Option<&SparseArray<EntityId, BUCKET_SIZE>> {
        Some(&self.sparse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{track, Component};

    #[derive(PartialEq, Eq, Debug)]
    struct STR(&'static str);

    impl Component for STR {
        type Tracking = track::Nothing;
    }

    #[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
    struct I32(i32);

    impl Component for I32 {
        type Tracking = track::Nothing;
    }

    #[test]
    fn insert() {
        let mut array = SparseSet::new();

        assert!(array
            .insert(EntityId::new_from_parts(0, 0, 0), STR("0"))
            .is_none());
        assert_eq!(array.dense, &[EntityId::new_from_parts(0, 0, 0)]);
        assert_eq!(array.data, &[STR("0")]);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(0, 0, 0)),
            Some(&STR("0"))
        );

        assert!(array
            .insert(EntityId::new_from_parts(1, 0, 0), STR("1"))
            .is_none());
        assert_eq!(
            array.dense,
            &[
                EntityId::new_from_parts(0, 0, 0),
                EntityId::new_from_parts(1, 0, 0)
            ]
        );
        assert_eq!(array.data, &[STR("0"), STR("1")]);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(0, 0, 0)),
            Some(&STR("0"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(1, 0, 0)),
            Some(&STR("1"))
        );

        assert!(array
            .insert(EntityId::new_from_parts(5, 0, 0), STR("5"))
            .is_none());
        assert_eq!(
            array.dense,
            &[
                EntityId::new_from_parts(0, 0, 0),
                EntityId::new_from_parts(1, 0, 0),
                EntityId::new_from_parts(5, 0, 0)
            ]
        );
        assert_eq!(array.data, &[STR("0"), STR("1"), STR("5")]);
        assert_eq!(
            array.private_get_mut(EntityId::new_from_parts(5, 0, 0)),
            Some(&mut STR("5"))
        );

        assert_eq!(array.private_get(EntityId::new_from_parts(4, 0, 0)), None);
    }

    #[test]
    fn remove() {
        let mut array = SparseSet::new();
        array.insert(EntityId::new_from_parts(0, 0, 0), STR("0"));
        array.insert(EntityId::new_from_parts(5, 0, 0), STR("5"));
        array.insert(EntityId::new_from_parts(10, 0, 0), STR("10"));

        assert_eq!(
            array.remove(EntityId::new_from_parts(0, 0, 0)),
            Some(STR("0"))
        );
        assert_eq!(
            array.dense,
            &[
                EntityId::new_from_parts(10, 0, 0),
                EntityId::new_from_parts(5, 0, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5")]);
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0, 0)),
            Some(&STR("10"))
        );

        array.insert(EntityId::new_from_parts(3, 0, 0), STR("3"));
        array.insert(EntityId::new_from_parts(100, 0, 0), STR("100"));
        assert_eq!(
            array.dense,
            &[
                EntityId::new_from_parts(10, 0, 0),
                EntityId::new_from_parts(5, 0, 0),
                EntityId::new_from_parts(3, 0, 0),
                EntityId::new_from_parts(100, 0, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5"), STR("3"), STR("100")]);
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(3, 0, 0)),
            Some(&STR("3"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0, 0)),
            Some(&STR("10"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(100, 0, 0)),
            Some(&STR("100"))
        );

        assert_eq!(
            array.remove(EntityId::new_from_parts(3, 0, 0)),
            Some(STR("3"))
        );
        assert_eq!(
            array.dense,
            &[
                EntityId::new_from_parts(10, 0, 0),
                EntityId::new_from_parts(5, 0, 0),
                EntityId::new_from_parts(100, 0, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5"), STR("100")]);
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
        assert_eq!(array.private_get(EntityId::new_from_parts(3, 0, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0, 0)),
            Some(&STR("10"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(100, 0, 0)),
            Some(&STR("100"))
        );

        assert_eq!(
            array.remove(EntityId::new_from_parts(100, 0, 0)),
            Some(STR("100"))
        );
        assert_eq!(
            array.dense,
            &[
                EntityId::new_from_parts(10, 0, 0),
                EntityId::new_from_parts(5, 0, 0)
            ]
        );
        assert_eq!(array.data, &[STR("10"), STR("5")]);
        assert_eq!(array.private_get(EntityId::new_from_parts(0, 0, 0)), None);
        assert_eq!(array.private_get(EntityId::new_from_parts(3, 0, 0)), None);
        assert_eq!(
            array.private_get(EntityId::new_from_parts(5, 0, 0)),
            Some(&STR("5"))
        );
        assert_eq!(
            array.private_get(EntityId::new_from_parts(10, 0, 0)),
            Some(&STR("10"))
        );
        assert_eq!(array.private_get(EntityId::new_from_parts(100, 0, 0)), None);
    }

    #[test]
    fn drain() {
        let mut sparse_set = SparseSet::new();

        sparse_set.insert(EntityId::new(0), I32(0));
        sparse_set.insert(EntityId::new(1), I32(1));

        let mut drain = sparse_set.drain();

        assert_eq!(drain.next(), Some(I32(0)));
        assert_eq!(drain.next(), Some(I32(1)));
        assert_eq!(drain.next(), None);

        drop(drain);

        assert_eq!(sparse_set.len(), 0);
        assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
    }

    #[test]
    fn drain_with_id() {
        let mut sparse_set = SparseSet::new();

        sparse_set.insert(EntityId::new(0), I32(0));
        sparse_set.insert(EntityId::new(1), I32(1));

        let mut drain = sparse_set.drain().with_id();

        assert_eq!(drain.next(), Some((EntityId::new(0), I32(0))));
        assert_eq!(drain.next(), Some((EntityId::new(1), I32(1))));
        assert_eq!(drain.next(), None);

        drop(drain);

        assert_eq!(sparse_set.len(), 0);
        assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
    }

    #[test]
    fn drain_empty() {
        let mut sparse_set = SparseSet::<I32>::new();

        assert_eq!(sparse_set.drain().next(), None);
        assert_eq!(sparse_set.drain().with_id().next(), None);

        assert_eq!(sparse_set.len(), 0);
    }

    #[test]
    fn unstable_sort() {
        let mut array = SparseSet::new();

        for i in (0..100).rev() {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(100 - i);
            array.insert(entity_id, I32(i as i32));
        }

        array.sort_unstable();

        for window in array.data.windows(2) {
            assert!(window[0] < window[1]);
        }
        for i in 0..100 {
            let mut entity_id = crate::entity_id::EntityId::zero();
            entity_id.set_index(100 - i);
            assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
        }
    }

    #[test]
    fn partially_sorted_unstable_sort() {
        let mut array = SparseSet::new();

        for i in 0..20 {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(i);
            assert!(array.insert(entity_id, I32(i as i32)).is_none());
        }
        for i in (20..100).rev() {
            let mut entity_id = EntityId::zero();
            entity_id.set_index(100 - i + 20);
            assert!(array.insert(entity_id, I32(i as i32)).is_none());
        }

        array.sort_unstable();

        for window in array.data.windows(2) {
            assert!(window[0] < window[1]);
        }
        for i in 0..20 {
            let mut entity_id = crate::entity_id::EntityId::zero();
            entity_id.set_index(i);
            assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
        }
        for i in 20..100 {
            let mut entity_id = crate::entity_id::EntityId::zero();
            entity_id.set_index(100 - i + 20);
            assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
        }
    }

    #[test]
    fn debug() {
        let mut sparse_set = SparseSet::new();

        sparse_set.insert(EntityId::new(0), STR("0"));
        sparse_set.insert(EntityId::new(5), STR("5"));
        sparse_set.insert(EntityId::new(10), STR("10"));

        println!("{:#?}", sparse_set);
    }
}
