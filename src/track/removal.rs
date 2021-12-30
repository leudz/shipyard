use super::{Removal, Tracking};
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::view::ViewMut;
use crate::{Component, SparseSet, SparseSetDrain};

impl Sealed for Removal {}

impl<T: Component<Tracking = Removal>> Tracking<T> for Removal {
    #[inline]
    fn track_removal() -> bool {
        true
    }

    fn is_removed(
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
    fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId, current: u32) -> Option<T> {
        let component = sparse_set.actual_remove(entity);

        if component.is_some() {
            sparse_set.removal_data.push((entity, current));
        }

        component
    }

    #[inline]
    fn delete(sparse_set: &mut SparseSet<T, Self>, entity: EntityId, _current: u32) -> bool {
        sparse_set.actual_remove(entity).is_some()
    }

    fn clear(sparse_set: &mut SparseSet<T, Self>, current: u32) {
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
    fn apply<R, F: FnOnce(&mut T, &T) -> R>(
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
    fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
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

    fn drain(sparse_set: &mut SparseSet<T, Self>, current: u32) -> SparseSetDrain<'_, T> {
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

    fn clear_all_removed_or_deleted(sparse_set: &mut SparseSet<T, Self>) {
        sparse_set.removal_data.clear();
    }
    fn clear_all_removed_or_deleted_older_than_timestamp(
        sparse_set: &mut SparseSet<T, Self>,
        timestamp: crate::TrackingTimestamp,
    ) {
        sparse_set.removal_data.retain(|(_, t)| {
            super::is_track_within_bounds(timestamp.0, t.wrapping_sub(u32::MAX / 2), *t)
        });
    }
}
