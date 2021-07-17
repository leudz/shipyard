use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::SparseSetDrain;

/// Determines what a storage should track.
pub struct Track<const INSERTION: bool, const MODIFIED: bool, const REMOVAL: bool>(());

#[allow(missing_docs)]
pub type Nothing = Track<false, false, false>;
#[allow(missing_docs)]
pub type Insertion = Track<true, false, false>;
#[allow(missing_docs)]
pub type Modification = Track<false, true, false>;
#[allow(missing_docs)]
pub type Removal = Track<false, false, true>;
#[allow(missing_docs)]
pub type All = Track<true, true, true>;

/// Trait implemented by all trackings.
pub trait Tracking: Sized + Sealed {
    #[doc(hidden)]
    type RemovalData: 'static + Send + Sync + Default;

    #[doc(hidden)]
    fn track_insertion() -> bool;
    #[doc(hidden)]
    fn track_modification() -> bool;
    #[doc(hidden)]
    fn track_removal() -> bool;

    #[doc(hidden)]
    fn used_memory<T: Component<Tracking = Self>>(_: &SparseSet<T, Self>) -> usize {
        0
    }

    #[doc(hidden)]
    fn reserved_memory<T: Component<Tracking = Self>>(_: &SparseSet<T, Self>) -> usize {
        0
    }

    #[doc(hidden)]
    fn remove<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
    ) -> Option<T>;

    #[doc(hidden)]
    fn clear<T: Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>);

    #[doc(hidden)]
    fn apply<T: Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
        sparse_set: &mut SparseSet<T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn apply_mut<T: Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
        sparse_set: &mut SparseSet<T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn drain<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
    ) -> SparseSetDrain<'_, T>;
}

mod nothing {
    use super::{Nothing, Tracking};
    use crate::{seal::Sealed, EntityId, SparseSet, SparseSetDrain};

    impl Sealed for Nothing {}

    impl Tracking for Nothing {
        type RemovalData = ();

        fn track_insertion() -> bool {
            false
        }

        fn track_modification() -> bool {
            false
        }

        fn track_removal() -> bool {
            false
        }

        fn remove<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
            entity: EntityId,
        ) -> Option<T> {
            sparse_set.actual_remove(entity)
        }

        fn clear<T: crate::Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>) {
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
        fn apply<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
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
        fn apply_mut<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
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

        fn drain<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
        ) -> SparseSetDrain<'_, T> {
            for id in &sparse_set.dense {
                // SAFE ids from sparse_set.dense are always valid
                unsafe {
                    *sparse_set.sparse.get_mut_unchecked(*id) = EntityId::dead();
                }
            }

            let dense_ptr = sparse_set.dense.as_ptr();
            let dense_len = sparse_set.dense.len();

            sparse_set.dense.clear();

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
    use crate::{seal::Sealed, EntityId, SparseSet, SparseSetDrain};

    impl Sealed for Insertion {}

    impl Tracking for Insertion {
        type RemovalData = ();

        fn track_insertion() -> bool {
            true
        }

        fn track_modification() -> bool {
            false
        }

        fn track_removal() -> bool {
            false
        }

        fn remove<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
            entity: EntityId,
        ) -> Option<T> {
            sparse_set.actual_remove(entity)
        }

        fn clear<T: crate::Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>) {
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
        fn apply<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
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
        fn apply_mut<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
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

        fn drain<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
        ) -> SparseSetDrain<'_, T> {
            for id in &sparse_set.dense {
                // SAFE ids from sparse_set.dense are always valid
                unsafe {
                    *sparse_set.sparse.get_mut_unchecked(*id) = EntityId::dead();
                }
            }

            let dense_ptr = sparse_set.dense.as_ptr();
            let dense_len = sparse_set.dense.len();

            sparse_set.dense.clear();

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
    use crate::{seal::Sealed, EntityId, SparseSet, SparseSetDrain};

    impl Sealed for Modification {}

    impl Tracking for Modification {
        type RemovalData = ();

        fn track_insertion() -> bool {
            false
        }

        fn track_modification() -> bool {
            true
        }

        fn track_removal() -> bool {
            false
        }

        fn remove<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
            entity: EntityId,
        ) -> Option<T> {
            sparse_set.actual_remove(entity)
        }

        fn clear<T: crate::Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>) {
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
        fn apply<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
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
        fn apply_mut<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
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

        fn drain<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
        ) -> SparseSetDrain<'_, T> {
            for id in &sparse_set.dense {
                // SAFE ids from sparse_set.dense are always valid
                unsafe {
                    *sparse_set.sparse.get_mut_unchecked(*id) = EntityId::dead();
                }
            }

            let dense_ptr = sparse_set.dense.as_ptr();
            let dense_len = sparse_set.dense.len();

            sparse_set.dense.clear();

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
    use crate::{SparseSet, SparseSetDrain};
    use alloc::vec::Vec;

    impl Sealed for Removal {}

    impl Tracking for Removal {
        type RemovalData = Vec<EntityId>;

        fn used_memory<T: crate::Component<Tracking = Self>>(
            sparse_set: &SparseSet<T, Self>,
        ) -> usize {
            sparse_set.removal_data.len() * core::mem::size_of::<EntityId>()
        }

        fn reserved_memory<T: crate::Component<Tracking = Self>>(
            sparse_set: &SparseSet<T, Self>,
        ) -> usize {
            sparse_set.removal_data.capacity() * core::mem::size_of::<EntityId>()
        }

        fn track_insertion() -> bool {
            false
        }

        fn track_modification() -> bool {
            false
        }

        fn track_removal() -> bool {
            true
        }

        fn remove<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
            entity: EntityId,
        ) -> Option<T> {
            let component = sparse_set.actual_remove(entity);

            if component.is_some() {
                sparse_set.removal_data.push(entity);
            }

            component
        }

        fn clear<T: crate::Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>) {
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
        fn apply<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
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
        fn apply_mut<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
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

        fn drain<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
        ) -> SparseSetDrain<'_, T> {
            sparse_set.removal_data.extend_from_slice(&sparse_set.dense);

            for id in &sparse_set.dense {
                // SAFE ids from sparse_set.dense are always valid
                unsafe {
                    *sparse_set.sparse.get_mut_unchecked(*id) = EntityId::dead();
                }
            }

            let dense_ptr = sparse_set.dense.as_ptr();
            let dense_len = sparse_set.dense.len();

            sparse_set.dense.clear();

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
    use crate::{SparseSet, SparseSetDrain};
    use alloc::vec::Vec;

    impl Sealed for All {}

    impl Tracking for All {
        type RemovalData = Vec<EntityId>;

        fn used_memory<T: crate::Component<Tracking = Self>>(
            sparse_set: &SparseSet<T, Self>,
        ) -> usize {
            sparse_set.removal_data.len() * core::mem::size_of::<EntityId>()
        }

        fn reserved_memory<T: crate::Component<Tracking = Self>>(
            sparse_set: &SparseSet<T, Self>,
        ) -> usize {
            sparse_set.removal_data.capacity() * core::mem::size_of::<EntityId>()
        }

        fn track_insertion() -> bool {
            true
        }

        fn track_modification() -> bool {
            true
        }

        fn track_removal() -> bool {
            true
        }

        fn remove<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
            entity: EntityId,
        ) -> Option<T> {
            let component = sparse_set.actual_remove(entity);

            if component.is_some() {
                sparse_set.removal_data.push(entity);
            }

            component
        }

        fn clear<T: crate::Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>) {
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
        fn apply<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
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
        fn apply_mut<T: crate::Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
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

        fn drain<T: crate::Component<Tracking = Self>>(
            sparse_set: &mut SparseSet<T, Self>,
        ) -> SparseSetDrain<'_, T> {
            sparse_set.removal_data.extend_from_slice(&sparse_set.dense);

            for id in &sparse_set.dense {
                // SAFE ids from sparse_set.dense are always valid
                unsafe {
                    *sparse_set.sparse.get_mut_unchecked(*id) = EntityId::dead();
                }
            }

            let dense_ptr = sparse_set.dense.as_ptr();
            let dense_len = sparse_set.dense.len();

            sparse_set.dense.clear();

            SparseSetDrain {
                dense_ptr,
                dense_len,
                data: sparse_set.data.drain(..),
            }
        }
    }
}
