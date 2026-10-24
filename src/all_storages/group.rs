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
use core::any::type_name;

/// Seals the grouping traits, only built-in storages can be grouped.
///
/// The traits are public but can't be named outside the crate, so nobody else can implement them.
mod seal {
    /// Implemented by every [`GroupMember`](super::GroupMember).
    pub trait Member {}
    /// Implemented by every [`TupleGroup`](super::TupleGroup).
    pub trait Tuple {}
}

/// A storage that can be part of a group, see [`WorldBuilder::with_group`].
///
/// Thread-local storages are written with the same wrappers used to borrow them, for example `NonSend<T>`.\
/// Only components and their thread-local wrappers are group members, it can't be implemented
/// outside of shipyard.
pub trait GroupMember: seal::Member {
    /// `false` when the storage is `!Send`, it can then only be borrowed exclusively from the world's thread.
    #[doc(hidden)]
    const THREAD_SAFE: bool;

    /// Returns the id of the storage this element refers to.
    #[doc(hidden)]
    fn storage_id() -> StorageId;
    /// Returns the type name of the storage this element refers to.
    #[doc(hidden)]
    fn storage_name() -> &'static str;
    /// Creates the storage if it doesn't exist yet.
    #[doc(hidden)]
    fn get_or_insert(all_storages: &mut AllStorages);
}

impl<T: Component + Send + Sync> seal::Member for T {}

impl<T: Component + Send + Sync> GroupMember for T {
    const THREAD_SAFE: bool = true;

    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<SparseSet<T>>()
    }

    #[inline]
    fn storage_name() -> &'static str {
        type_name::<SparseSet<T>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_component_or_insert_mut(Self::storage_id(), SparseSet::<T>::new);
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> seal::Member for NonSend<T> {}

#[cfg(feature = "thread_local")]
impl<T: Component + Sync> GroupMember for NonSend<T> {
    const THREAD_SAFE: bool = false;

    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<NonSend<SparseSet<T>>>()
    }

    #[inline]
    fn storage_name() -> &'static str {
        type_name::<NonSend<SparseSet<T>>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_component_or_insert_non_send_mut(Self::storage_id(), || {
            NonSend(SparseSet::<T>::new())
        });
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> seal::Member for NonSync<T> {}

#[cfg(feature = "thread_local")]
impl<T: Component + Send> GroupMember for NonSync<T> {
    const THREAD_SAFE: bool = true;

    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<NonSync<SparseSet<T>>>()
    }

    #[inline]
    fn storage_name() -> &'static str {
        type_name::<NonSync<SparseSet<T>>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_component_or_insert_non_sync_mut(Self::storage_id(), || {
            NonSync(SparseSet::<T>::new())
        });
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component> seal::Member for NonSendSync<T> {}

#[cfg(feature = "thread_local")]
impl<T: Component> GroupMember for NonSendSync<T> {
    const THREAD_SAFE: bool = false;

    #[inline]
    fn storage_id() -> StorageId {
        StorageId::of::<NonSendSync<SparseSet<T>>>()
    }

    #[inline]
    fn storage_name() -> &'static str {
        type_name::<NonSendSync<SparseSet<T>>>()
    }

    #[inline]
    fn get_or_insert(all_storages: &mut AllStorages) {
        all_storages.exclusive_component_or_insert_non_send_sync_mut(Self::storage_id(), || {
            NonSendSync(SparseSet::<T>::new())
        });
    }
}

/// What the world keeps about a [`GroupMember`].
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct MemberInfo {
    pub(crate) storage_id: StorageId,
    /// The storage's type name, to name it in errors.
    pub(crate) storage_name: &'static str,
    /// Creates the storage if it doesn't exist yet.
    pub(crate) create: fn(&mut AllStorages),
    /// `false` when the storage can only be borrowed exclusively from the world's thread.
    pub(crate) thread_safe: bool,
}

/// Trait used as bound for [`WorldBuilder::with_group`].
///
/// Only implemented for tuples of two or more elements, since a group needs at least two storages.\
/// It can't be implemented outside of shipyard.
#[doc(hidden)]
pub trait TupleGroup: seal::Tuple {
    /// Returns every member's information, in tuple order.
    fn members() -> Vec<MemberInfo>;
}

macro_rules! impl_group {
    ($(($member: ident, $index: tt))+) => {
        impl<$($member: GroupMember),+> seal::Tuple for ($($member,)+) {}

        impl<$($member: GroupMember),+> TupleGroup for ($($member,)+) {
            #[inline]
            fn members() -> Vec<MemberInfo> {
                vec![$(MemberInfo {
                    storage_id: $member::storage_id(),
                    storage_name: $member::storage_name(),
                    create: $member::get_or_insert,
                    thread_safe: $member::THREAD_SAFE,
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
/// This is configuration only: each group's members, how to create and borrow each member storage,
/// and the connected sets they make.\
/// It never changes once the world is built. Ranges, unions and trust are storage state
/// and live in each member's [`Groups`](crate::sparse_set::Groups).
#[derive(Clone, Default)]
pub(crate) struct Grouping {
    /// Every declared group's sorted member ids, in declaration order, without duplicates.
    pub(crate) groups: Vec<Arc<[StorageId]>>,
    /// A single entry per member storage.
    members: Vec<MemberInfo>,
    /// The connected sets made by `groups`, each member is in exactly one.
    pub(crate) sets: Vec<ConnectedSet>,
}

/// Storages linked together by overlapping declared groups.
///
/// `AB`, `AC` and `CD` make a single `ABCD` set, while `AB` and `CD` make two sets,
/// even when an entity owns all four components.\
/// Regroup only creates unions of groups that overlap, so the unions of a set stay inside it.
/// A set is everything regrouping one of its members can touch.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConnectedSet {
    /// Sorted member ids.
    pub(crate) members: Vec<StorageId>,
    /// `false` when a member can only be borrowed exclusively from the world's thread,
    /// regrouping the set then has to happen there.
    pub(crate) thread_safe: bool,
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
        let ids = sorted_ids(members.iter().map(|member| member.storage_id).collect());

        if self.groups.iter().any(|group| group[..] == ids[..]) {
            return;
        }

        for member in members {
            if self.member(&member.storage_id).is_none() {
                self.members.push(member);
            }
        }

        self.link(&ids);
        self.groups.push(Arc::from(ids));
    }

    /// Merges the new group made of `ids` with every set sharing one of its members.
    ///
    /// Each set is merged as soon as a group links it, so the sets are always complete.
    fn link(&mut self, ids: &[StorageId]) {
        let mut members = ids.to_vec();
        self.sets.retain(|set| {
            let linked = set.members.iter().any(|member| ids.contains(member));
            if linked {
                members.extend_from_slice(&set.members);
            }

            !linked
        });

        members.sort_unstable();
        members.dedup();

        let thread_safe = members.iter().all(|storage_id| {
            self.member(storage_id)
                .expect("members are known before being linked")
                .thread_safe
        });

        self.sets.push(ConnectedSet {
            members,
            thread_safe,
        });
    }

    /// Returns the information about the member storage `storage_id`, `None` if it isn't one.
    pub(crate) fn member(&self, storage_id: &StorageId) -> Option<&MemberInfo> {
        self.members
            .iter()
            .find(|member| &member.storage_id == storage_id)
    }

    /// Returns the connected set `storage_id` is part of, `None` if it isn't in any declared group.
    pub(crate) fn set_of(&self, storage_id: &StorageId) -> Option<&ConnectedSet> {
        self.sets
            .iter()
            .find(|set| set.members.binary_search(storage_id).is_ok())
    }

    /// Creates the member storages, attaches the groups and keeps `self` as `all_storages`'s configuration.
    ///
    /// Each member gets an empty range per group, in declaration order.\
    /// This is meant for a world without components, nothing is regrouped.
    pub(crate) fn install(self, all_storages: &mut AllStorages) {
        for member in &self.members {
            (member.create)(all_storages);
        }

        let storages = &mut all_storages.storages.get_mut().components_and_custom;
        for group in &self.groups {
            for storage_id in group.iter() {
                member_groups(storages, storage_id).attach(group.clone(), *storage_id);
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
fn sorted_ids(mut ids: Vec<StorageId>) -> Vec<StorageId> {
    ids.sort_unstable();

    assert!(
        ids.windows(2).all(|pair| pair[0] != pair[1]),
        "a group cannot contain identical storages"
    );

    ids
}
