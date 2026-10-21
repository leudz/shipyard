#[cfg(test)]
mod tests;
// Rustdoc doesn't set `test` when it collects doc tests, these checks need their own gate.
#[cfg(doctest)]
#[path = "group/tests/compile_fail.rs"]
mod compile_fail_tests;

use crate::all_storages::{AllStorages, member_groups};
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
#[cfg(doc)]
use crate::world::{World, WorldBuilder};
use alloc::sync::Arc;
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

/// Creates a group member's storage if it doesn't exist yet.
type CreateStorage = fn(&mut AllStorages);

/// Trait used as bound for [`WorldBuilder::with_group`], [`World::group`] and [`AllStorages::group`].
///
/// Only implemented for tuples of two or more elements, since a group needs at least two storages.\
/// It can't be implemented outside of shipyard.
#[doc(hidden)]
pub trait TupleGroup: seal::Tuple {
    /// Returns every member's storage id and the function creating its storage, in tuple order.
    fn members() -> Vec<(StorageId, CreateStorage)>;
}

macro_rules! impl_group {
    ($(($member: ident, $index: tt))+) => {
        impl<$($member: GroupMember),+> seal::Tuple for ($($member,)+) {}

        impl<$($member: GroupMember),+> TupleGroup for ($($member,)+) {
            #[inline]
            fn members() -> Vec<(StorageId, CreateStorage)> {
                vec![$({
                    let create: CreateStorage = $member::get_or_insert;
                    ($member::storage_id(), create)
                }),+]
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

/// The groups declared when building a [`World`].
///
/// This is configuration only: each group's members and how to create each member storage.\
/// It never changes once the world is built. Ranges, unions and trust are storage state
/// and live in each member's [`Groups`](crate::sparse_set::Groups).
#[derive(Clone, Default)]
pub(crate) struct Grouping {
    /// Every declared group's sorted member ids, in declaration order, without duplicates.
    pub(crate) groups: Vec<Arc<[StorageId]>>,
    /// The function creating each member storage, a single entry per storage.
    members: Vec<(StorageId, CreateStorage)>,
}

impl Grouping {
    /// Adds the group made of `G`'s storages, unless an equal group was already declared.
    ///
    /// ### Panics
    ///
    /// - The same storage is listed twice.
    #[track_caller]
    pub(crate) fn declare<G: TupleGroup>(&mut self) {
        let members = G::members();
        let ids = sorted_ids(members.iter().map(|&(storage_id, _)| storage_id).collect());

        if self.groups.iter().any(|group| group[..] == ids[..]) {
            return;
        }

        for (storage_id, create) in members {
            if !self.members.iter().any(|&(known, _)| known == storage_id) {
                self.members.push((storage_id, create));
            }
        }

        self.groups.push(Arc::from(ids));
    }

    /// Creates the member storages, attaches the groups and keeps `self` as `all_storages`'s configuration.
    ///
    /// Each member gets an empty range per group, in declaration order.\
    /// This is meant for a world without components, nothing is regrouped.
    pub(crate) fn install(self, all_storages: &mut AllStorages) {
        for &(_, create) in &self.members {
            create(all_storages);
        }

        let storages = all_storages.storages.get_mut();
        for group in &self.groups {
            for &storage_id in group.iter() {
                member_groups(storages, storage_id).attach(group.clone(), storage_id);
            }
        }

        all_storages.grouping = self;
    }
}

/// Returns `ids` sorted, the identity of the group they make. `(A, B)` and `(B, A)` are the same group.
///
/// ### Panics
///
/// - The same storage is listed twice.
#[track_caller]
pub(crate) fn sorted_ids(mut ids: Vec<StorageId>) -> Vec<StorageId> {
    ids.sort_unstable();

    assert!(
        ids.windows(2).all(|pair| pair[0] != pair[1]),
        "a group cannot contain identical storages"
    );

    ids
}
