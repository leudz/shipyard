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

    fn is_removed(sparse_set: &SparseSet<T, Self>, entity: EntityId) -> bool {
        sparse_set.removal_data.iter().any(|id| *id == entity)
    }

    #[inline]
    fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> Option<T> {
        let component = sparse_set.actual_remove(entity);

        if component.is_some() {
            sparse_set.removal_data.push(entity);
        }

        component
    }

    #[inline]
    fn delete(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> bool {
        sparse_set.actual_remove(entity).is_some()
    }

    fn clear(sparse_set: &mut SparseSet<T, Self>) {
        for &id in &sparse_set.dense {
            unsafe {
                *sparse_set.sparse.get_mut_unchecked(id) = EntityId::dead();
            }
        }

        sparse_set.removal_data.append(&mut sparse_set.dense);
        sparse_set.data.clear();
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

    fn drain(sparse_set: &mut SparseSet<T, Self>) -> SparseSetDrain<'_, T> {
        sparse_set.removal_data.extend_from_slice(&sparse_set.dense);

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
}
