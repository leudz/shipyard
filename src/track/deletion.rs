use super::{Deletion, Tracking};
use crate::seal::Sealed;
use crate::view::ViewMut;
use crate::{Component, EntityId, SparseSet, SparseSetDrain};

impl Sealed for Deletion {}

impl Tracking for Deletion {
    #[inline]
    fn track_deletion() -> bool {
        true
    }

    fn is_deleted<T: Component<Tracking = Self>>(
        sparse_set: &SparseSet<T, Self>,
        entity: EntityId,
        last: u64,
        current: u64,
    ) -> bool {
        sparse_set.deletion_data.iter().any(|(id, timestamp, _)| {
            *id == entity && super::is_track_within_bounds(*timestamp, last, current)
        })
    }

    #[inline]
    fn remove<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
        _current: u64,
    ) -> Option<T> {
        sparse_set.actual_remove(entity)
    }

    #[inline]
    fn delete<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
        current: u64,
    ) -> bool {
        if let Some(component) = sparse_set.actual_remove(entity) {
            sparse_set.deletion_data.push((entity, current, component));

            true
        } else {
            false
        }
    }

    fn clear<T: Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>, current: u64) {
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
            let a = unsafe { &mut *sparse_set.data.as_mut_ptr().add(a_index) };
            let b = unsafe { &mut *sparse_set.data.as_mut_ptr().add(b_index) };

            f(a, b)
        } else {
            panic!("Cannot use apply with identical components.");
        }
    }

    #[inline]
    fn drain<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        _current: u64,
    ) -> SparseSetDrain<'_, T> {
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

    fn clear_all_removed_or_deleted<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
    ) {
        sparse_set.deletion_data.clear();
    }
    fn clear_all_removed_or_deleted_older_than_timestamp<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        timestamp: crate::TrackingTimestamp,
    ) {
        sparse_set.deletion_data.retain(|(_, t, _)| {
            super::is_track_within_bounds(timestamp.0, t.wrapping_sub(u64::MAX / 2), *t)
        });
    }
}
