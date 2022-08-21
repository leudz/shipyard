use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::track::{
    map_deletion_data, All, DeletionTracking, InsertionOrModificationTracking, InsertionTracking,
    ModificationTracking, RemovalOrDeletionTracking, RemovalTracking, Tracking,
};
use crate::view::ViewMut;
use crate::{Component, SparseSet, SparseSetDrain};

impl Sealed for All {}

impl Tracking for All {
    #[inline]
    fn track_insertion() -> bool {
        true
    }

    #[inline]
    fn track_modification() -> bool {
        true
    }

    #[inline]
    fn track_deletion() -> bool {
        true
    }

    #[inline]
    fn track_removal() -> bool {
        true
    }

    fn is_inserted<T: Component<Tracking = Self>>(
        sparse_set: &SparseSet<T, Self>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        if let Some(dense) = sparse_set.index_of(entity) {
            super::is_track_within_bounds(sparse_set.insertion_data[dense], last, current)
        } else {
            false
        }
    }
    fn is_modified<T: Component<Tracking = Self>>(
        sparse_set: &SparseSet<T, Self>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        if let Some(dense) = sparse_set.index_of(entity) {
            super::is_track_within_bounds(sparse_set.modification_data[dense], last, current)
        } else {
            false
        }
    }
    fn is_deleted<T: Component<Tracking = Self>>(
        sparse_set: &SparseSet<T, Self>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        sparse_set.deletion_data.iter().any(|(id, timestamp, _)| {
            *id == entity && super::is_track_within_bounds(*timestamp, last, current)
        })
    }
    fn is_removed<T: Component<Tracking = Self>>(
        sparse_set: &SparseSet<T, Self>,
        entity: EntityId,
        last: u32,
        current: u32,
    ) -> bool {
        sparse_set.removal_data.iter().any(|(id, timestamp)| {
            *id == entity && super::is_track_within_bounds(*timestamp, last, current)
        })
    }

    #[inline]
    fn remove<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
        current: u32,
    ) -> Option<T> {
        let component = sparse_set.actual_remove(entity);

        if component.is_some() {
            sparse_set.removal_data.push((entity, current));
        }

        component
    }

    #[inline]
    fn delete<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
        current: u32,
    ) -> bool {
        if let Some(component) = sparse_set.actual_remove(entity) {
            sparse_set.deletion_data.push((entity, current, component));

            true
        } else {
            false
        }
    }

    fn clear<T: Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>, current: u32) {
        for &id in &sparse_set.dense {
            unsafe {
                *sparse_set.sparse.get_mut_unchecked(id) = EntityId::dead();
            }
        }

        sparse_set.deletion_data.extend(
            sparse_set
                .dense
                .drain(..)
                .zip(sparse_set.data.drain(..))
                .map(|(entity, component)| (entity, current, component)),
        );
        sparse_set.insertion_data.clear();
        sparse_set.modification_data.clear();
    }

    #[track_caller]
    #[inline]
    fn apply<T: Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
        sparse_set: &mut ViewMut<'_, T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R {
        let a_index = sparse_set.index_of(a).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                a
            )
        });
        let b_index = sparse_set.index_of(b).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                b
            )
        });

        if a_index != b_index {
            unsafe {
                *sparse_set.modification_data.get_unchecked_mut(a_index) = sparse_set.current;
            }

            let a = unsafe { &mut *sparse_set.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &*sparse_set.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }

    #[track_caller]
    #[inline]
    fn apply_mut<T: Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
        sparse_set: &mut ViewMut<'_, T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R {
        let a_index = sparse_set.index_of(a).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                a
            )
        });
        let b_index = sparse_set.index_of(b).unwrap_or_else(move || {
            panic!(
                "Entity {:?} does not have any component in this storage.",
                b
            )
        });

        if a_index != b_index {
            unsafe {
                *sparse_set.modification_data.get_unchecked_mut(a_index) = sparse_set.current;
                *sparse_set.modification_data.get_unchecked_mut(b_index) = sparse_set.current;
            }

            let a = unsafe { &mut *sparse_set.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &mut *sparse_set.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }

    fn drain<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        current: u32,
    ) -> SparseSetDrain<'_, T> {
        sparse_set
            .removal_data
            .extend(sparse_set.dense.drain(..).map(|entity| (entity, current)));

        for id in &sparse_set.dense {
            // SAFE ids from sparse_set.dense are always valid
            unsafe {
                *sparse_set.sparse.get_mut_unchecked(*id) = EntityId::dead();
            }
        }

        let dense_ptr = sparse_set.dense.as_ptr();
        let dense_len = sparse_set.dense.len();

        unsafe {
            sparse_set.dense.set_len(0);
        }

        SparseSetDrain {
            dense_ptr,
            dense_len,
            data: sparse_set.data.drain(..),
        }
    }

    fn clear_all_removed_and_deleted<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
    ) {
        sparse_set.deletion_data.clear();
        sparse_set.removal_data.clear();
    }
    fn clear_all_removed_and_deleted_older_than_timestamp<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        timestamp: crate::TrackingTimestamp,
    ) {
        sparse_set.deletion_data.retain(|(_, t, _)| {
            super::is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t)
        });
        sparse_set.removal_data.retain(|(_, t)| {
            super::is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t)
        });
    }
}

impl InsertionTracking for All {}
impl ModificationTracking for All {}
impl InsertionOrModificationTracking for All {}
impl RemovalTracking for All {}
impl DeletionTracking for All {}
impl RemovalOrDeletionTracking for All {
    #[allow(trivial_casts)]
    fn removed_or_deleted<T: Component<Tracking = Self>>(
        sparse_set: &SparseSet<T, Self>,
    ) -> core::iter::Chain<
        core::iter::Map<
            core::slice::Iter<'_, (EntityId, u32, T)>,
            for<'r> fn(&'r (EntityId, u32, T)) -> (EntityId, u32),
        >,
        core::iter::Copied<core::slice::Iter<'_, (EntityId, u32)>>,
    > {
        sparse_set
            .deletion_data
            .iter()
            .map(map_deletion_data as _)
            .chain(sparse_set.removal_data.iter().copied())
    }
}
