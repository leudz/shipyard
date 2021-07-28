use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::SparseSetDrain;

/// Determines what a storage should track.
pub struct Track<
    const INSERTION: bool,
    const MODIFIED: bool,
    const DELETION: bool,
    const REMOVAL: bool,
>(());

#[allow(missing_docs)]
pub type Nothing = Track<false, false, false, false>;
#[allow(missing_docs)]
pub type Insertion = Track<true, false, false, false>;
#[allow(missing_docs)]
pub type Modification = Track<false, true, false, false>;
#[allow(missing_docs)]
pub type Deletion = Track<false, false, true, false>;
#[allow(missing_docs)]
pub type Removal = Track<false, false, false, true>;
#[allow(missing_docs)]
pub type All = Track<true, true, true, true>;

/// Trait implemented by all trackings.
pub trait Tracking<T: Component>: Sized + Sealed {
    #[doc(hidden)]
    type DeletionData: 'static + Default;
    #[doc(hidden)]
    type RemovalData: 'static + Send + Sync + Default;

    #[doc(hidden)]
    fn track_insertion() -> bool;
    #[doc(hidden)]
    fn track_modification() -> bool;
    #[doc(hidden)]
    fn track_removal() -> bool;

    #[doc(hidden)]
    fn used_memory(_: &SparseSet<T, Self>) -> usize {
        0
    }

    #[doc(hidden)]
    fn reserved_memory(_: &SparseSet<T, Self>) -> usize {
        0
    }

    #[doc(hidden)]
    fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> Option<T>;

    #[doc(hidden)]
    fn delete(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> bool;

    #[doc(hidden)]
    fn clear(sparse_set: &mut SparseSet<T, Self>);

    #[doc(hidden)]
    fn apply<R, F: FnOnce(&mut T, &T) -> R>(
        sparse_set: &mut SparseSet<T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        sparse_set: &mut SparseSet<T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn drain(sparse_set: &mut SparseSet<T, Self>) -> SparseSetDrain<'_, T>;
}

mod nothing {
    use super::{Nothing, Tracking};
    use crate::{seal::Sealed, Component, EntityId, SparseSet, SparseSetDrain};

    impl Sealed for Nothing {}

    impl<T: Component<Tracking = Nothing>> Tracking<T> for Nothing {
        type DeletionData = ();
        type RemovalData = ();

        #[inline]
        fn track_insertion() -> bool {
            false
        }

        #[inline]
        fn track_modification() -> bool {
            false
        }

        #[inline]
        fn track_removal() -> bool {
            false
        }

        #[inline]
        fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> Option<T> {
            sparse_set.actual_remove(entity)
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

            sparse_set.dense.clear();
            sparse_set.data.clear();
        }

        #[track_caller]
        #[inline]
        fn apply<R, F: FnOnce(&mut T, &T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
            sparse_set: &mut SparseSet<T, Self>,
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
}

mod insertion {
    use super::{Insertion, Tracking};
    use crate::{seal::Sealed, Component, EntityId, SparseSet, SparseSetDrain};

    impl Sealed for Insertion {}

    impl<T: Component<Tracking = Insertion>> Tracking<T> for Insertion {
        type DeletionData = ();
        type RemovalData = ();

        #[inline]
        fn track_insertion() -> bool {
            true
        }

        #[inline]
        fn track_modification() -> bool {
            false
        }

        #[inline]
        fn track_removal() -> bool {
            false
        }

        #[inline]
        fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> Option<T> {
            sparse_set.actual_remove(entity)
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

            sparse_set.dense.clear();
            sparse_set.data.clear();
        }

        #[track_caller]
        #[inline]
        fn apply<R, F: FnOnce(&mut T, &T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
            sparse_set: &mut SparseSet<T, Self>,
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
}

mod modification {
    use super::{Modification, Tracking};
    use crate::{seal::Sealed, Component, EntityId, SparseSet, SparseSetDrain};

    impl Sealed for Modification {}

    impl<T: Component<Tracking = Modification>> Tracking<T> for Modification {
        type DeletionData = ();
        type RemovalData = ();

        #[inline]
        fn track_insertion() -> bool {
            false
        }

        #[inline]
        fn track_modification() -> bool {
            true
        }

        #[inline]
        fn track_removal() -> bool {
            false
        }

        #[inline]
        fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId) -> Option<T> {
            sparse_set.actual_remove(entity)
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

            sparse_set.dense.clear();
            sparse_set.data.clear();
        }

        #[track_caller]
        #[inline]
        fn apply<R, F: FnOnce(&mut T, &T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
                {
                    let a_dense = unsafe { sparse_set.dense.get_unchecked_mut(a_index) };
                    a_dense.set_modified();
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
        fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
                    let a_dense = sparse_set.dense.get_unchecked_mut(a_index);
                    a_dense.set_modified();

                    let b_dense = sparse_set.dense.get_unchecked_mut(b_index);
                    b_dense.set_modified();
                }

                let a = unsafe { &mut *sparse_set.data.as_mut_ptr().add(a_index) };
                let b = unsafe { &mut *sparse_set.data.as_mut_ptr().add(b_index) };

                f(a, b)
            } else {
                panic!("Cannot use apply with identical components.");
            }
        }

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
}

mod deletion {
    use super::{Deletion, Tracking};
    use crate::seal::Sealed;
    use crate::{Component, EntityId, SparseSet, SparseSetDrain};
    use alloc::vec::Vec;

    impl Sealed for Deletion {}

    impl<T: Component<Tracking = Deletion>> Tracking<T> for Deletion {
        type DeletionData = Vec<(EntityId, T)>;
        type RemovalData = ();

        #[inline]
        fn used_memory(sparse_set: &SparseSet<T, Self>) -> usize {
            sparse_set.deletion_data.len() * core::mem::size_of::<(EntityId, T)>()
        }

        #[inline]
        fn reserved_memory(sparse_set: &SparseSet<T, Self>) -> usize {
            sparse_set.deletion_data.capacity() * core::mem::size_of::<(EntityId, T)>()
        }

        #[inline]
        fn track_insertion() -> bool {
            false
        }

        #[inline]
        fn track_modification() -> bool {
            false
        }

        #[inline]
        fn track_removal() -> bool {
            false
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
            sparse_set: &mut SparseSet<T, Self>,
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
            sparse_set: &mut SparseSet<T, Self>,
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
}

mod removal {
    use super::{Removal, Tracking};
    use crate::entity_id::EntityId;
    use crate::seal::Sealed;
    use crate::{Component, SparseSet, SparseSetDrain};
    use alloc::vec::Vec;

    impl Sealed for Removal {}

    impl<T: Component<Tracking = Removal>> Tracking<T> for Removal {
        type DeletionData = ();
        type RemovalData = Vec<EntityId>;

        #[inline]
        fn used_memory(sparse_set: &SparseSet<T, Self>) -> usize {
            sparse_set.removal_data.len() * core::mem::size_of::<EntityId>()
        }

        #[inline]
        fn reserved_memory(sparse_set: &SparseSet<T, Self>) -> usize {
            sparse_set.removal_data.capacity() * core::mem::size_of::<EntityId>()
        }

        #[inline]
        fn track_insertion() -> bool {
            false
        }

        #[inline]
        fn track_modification() -> bool {
            false
        }

        #[inline]
        fn track_removal() -> bool {
            true
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

            sparse_set.removal_data.extend(sparse_set.dense.drain(..));
            sparse_set.data.clear();
        }

        #[track_caller]
        #[inline]
        fn apply<R, F: FnOnce(&mut T, &T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
            sparse_set: &mut SparseSet<T, Self>,
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
}

mod all {
    use super::{All, Tracking};
    use crate::entity_id::EntityId;
    use crate::seal::Sealed;
    use crate::{Component, SparseSet, SparseSetDrain};
    use alloc::vec::Vec;

    impl Sealed for All {}

    impl<T: Component<Tracking = All>> Tracking<T> for All {
        type DeletionData = Vec<(EntityId, T)>;
        type RemovalData = Vec<EntityId>;

        #[inline]
        fn used_memory(sparse_set: &SparseSet<T, Self>) -> usize {
            sparse_set.deletion_data.len() * core::mem::size_of::<(EntityId, T)>()
                + sparse_set.removal_data.len() * core::mem::size_of::<EntityId>()
        }

        #[inline]
        fn reserved_memory(sparse_set: &SparseSet<T, Self>) -> usize {
            sparse_set.deletion_data.capacity() * core::mem::size_of::<(EntityId, T)>()
                + sparse_set.removal_data.capacity() * core::mem::size_of::<EntityId>()
        }

        #[inline]
        fn track_insertion() -> bool {
            true
        }

        #[inline]
        fn track_modification() -> bool {
            true
        }

        #[inline]
        fn track_removal() -> bool {
            true
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
            if let Some(component) = sparse_set.actual_remove(entity) {
                sparse_set.deletion_data.push((entity, component));

                true
            } else {
                false
            }
        }

        fn clear(sparse_set: &mut SparseSet<T, Self>) {
            for &id in &sparse_set.dense {
                unsafe {
                    *sparse_set.sparse.get_mut_unchecked(id) = EntityId::dead();
                }
            }

            sparse_set.removal_data.extend(sparse_set.dense.drain(..));
            sparse_set.data.clear();
        }

        #[track_caller]
        #[inline]
        fn apply<R, F: FnOnce(&mut T, &T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
                {
                    let a_dense = unsafe { sparse_set.dense.get_unchecked_mut(a_index) };
                    if !a_dense.is_inserted() {
                        a_dense.set_modified();
                    }
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
        fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
            sparse_set: &mut SparseSet<T, Self>,
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
                    let a_dense = sparse_set.dense.get_unchecked_mut(a_index);
                    if !a_dense.is_inserted() {
                        a_dense.set_modified();
                    }

                    let b_dense = sparse_set.dense.get_unchecked_mut(b_index);
                    if !b_dense.is_inserted() {
                        b_dense.set_modified();
                    }
                }

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
}
