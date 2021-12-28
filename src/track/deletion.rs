use super::{Deletion, Tracking};
use crate::seal::Sealed;
use crate::view::ViewMut;
use crate::{Component, EntityId, SparseSet, SparseSetDrain};

impl Sealed for Deletion {}

impl<T: Component<Tracking = Deletion>> Tracking<T> for Deletion {
    #[inline]
    fn track_deletion() -> bool {
        true
    }

    fn is_deleted(sparse_set: &SparseSet<T, Self>, entity: EntityId) -> bool {
        sparse_set.deletion_data.iter().any(|(id, _)| *id == entity)
    }

    #[inline]
    fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> Option<T> {
        sparse_set.actual_remove(entity)
    }

    #[inline]
    fn delete(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> bool {
        if let Some(component) = sparse_set.actual_remove(entity) {
            sparse_set.deletion_data.push((entity, component));

            true
        } else {
            false
        }
    }

    #[inline]
    fn clear(sparse_set: &mut SparseSet<T, Self>) {
        for &id in &sparse_set.dense {
            unsafe {
                *sparse_set.sparse.get_mut_unchecked(id) = EntityId::dead();
            }
        }

        sparse_set
            .deletion_data
            .extend(sparse_set.dense.drain(..).zip(sparse_set.data.drain(..)));
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

    #[inline]
    fn drain(sparse_set: &mut SparseSet<T, Self>) -> SparseSetDrain<'_, T> {
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