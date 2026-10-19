#[cfg(test)]
mod tests;
// Rustdoc doesn't set `test` when it collects doc tests, these checks need their own gate.
#[cfg(doctest)]
#[path = "group/tests/compile_fail.rs"]
mod compile_fail_tests;

use crate::all_storages::AllStorages;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
#[cfg(doc)]
use crate::world::World;
use alloc::vec;
use alloc::vec::Vec;

/// Seals the grouping traits, only built-in storages can be grouped.
///
/// The traits are public but can't be named outside the crate, so nobody else can implement them.
mod seal {
    /// Implemented by every [`GroupMember`](super::GroupMember).
    pub trait Member {}
    /// Implemented by every [`TupleGroup`](super::TupleGroup).
    pub trait Tuple {}
}

/// One element of a [`TupleGroup`].
///
/// Thread-local storages are written with the same wrappers used to borrow them, for example `NonSend<T>`.\
/// Only components and their thread-local wrappers are group members, it can't be implemented
/// outside of shipyard.
#[doc(hidden)]
pub trait GroupMember: seal::Member {
    /// Returns the id of the storage this element refers to.
    fn storage_id() -> StorageId;
    /// Creates the storage if it doesn't exist yet.
    fn get_or_insert(all_storages: &mut AllStorages);
}

impl<T: Component + Send + Sync> seal::Member for T {}

impl<T: Component + Send + Sync> GroupMember for T {
    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<SparseSet<T>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_storage_or_insert_mut(Self::storage_id(), SparseSet::<T>::new);
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> seal::Member for NonSend<T> {}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> GroupMember for NonSend<T> {
    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<NonSend<SparseSet<T>>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_storage_or_insert_non_send_mut(Self::storage_id(), || {
            NonSend(SparseSet::<T>::new())
        });
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> seal::Member for NonSync<T> {}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> GroupMember for NonSync<T> {
    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<NonSync<SparseSet<T>>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_storage_or_insert_non_sync_mut(Self::storage_id(), || {
            NonSync(SparseSet::<T>::new())
        });
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> seal::Member for NonSendSync<T> {}

#[cfg(feature = "thread_local")]
impl<T: Component> GroupMember for NonSendSync<T> {
    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<NonSendSync<SparseSet<T>>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_storage_or_insert_non_send_sync_mut(Self::storage_id(), || {
            NonSendSync(SparseSet::<T>::new())
        });
    }
}

/// Trait used as bound for [`World::group`] and [`AllStorages::group`].
///
/// Only implemented for tuples of two or more elements, since a group needs at least two storages.\
/// It can't be implemented outside of shipyard.
#[doc(hidden)]
pub trait TupleGroup: seal::Tuple {
    /// Returns the id of every member storage, in tuple order.
    fn storage_ids() -> Vec<StorageId>;
    /// Creates every member storage that doesn't exist yet.
    fn get_or_insert_storages(all_storages: &mut AllStorages);
}

macro_rules! impl_group {
    ($(($member: ident, $index: tt))+) => {
        impl<$($member: GroupMember),+> seal::Tuple for ($($member,)+) {}

        impl<$($member: GroupMember),+> TupleGroup for ($($member,)+) {
            #[inline]
            fn storage_ids() -> Vec<StorageId> {
                vec![$($member::storage_id()),+]
            }

            #[inline]
            fn get_or_insert_storages(all_storages: &mut AllStorages) {
                $(
                    $member::get_or_insert(all_storages);
                )+
            }
        }
    }
}

macro_rules! group {
    ($(($member: ident, $index: tt))+; ($member1: ident, $index1: tt) $(($queue_member: ident, $queue_index: tt))*) => {
        impl_group![$(($member, $index))*];
        group![$(($member, $index))* ($member1, $index1); $(($queue_member, $queue_index))*];
    };
    ($(($member: ident, $index: tt))+;) => {
        impl_group![$(($member, $index))*];
    }
}

// Starts at two elements, there is no single-storage group.
#[cfg(not(feature = "extended_tuple"))]
group![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
group![
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
