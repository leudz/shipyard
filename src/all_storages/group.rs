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

/// One element of a [`TupleGroup`].
///
/// Thread-local storages are written with the same wrappers used to borrow them, for example `NonSend<T>`.
#[doc(hidden)]
pub trait GroupMember {
    /// Returns the id of the storage this element refers to.
    fn storage_id() -> StorageId;
    /// Creates the storage if it doesn't exist yet.
    fn get_or_insert(all_storages: &mut AllStorages);
}

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
/// Only implemented for tuples of two or more elements, since a group needs at least two storages.
#[doc(hidden)]
pub trait TupleGroup {
    /// Returns the id of every member storage, in tuple order.
    fn storage_ids() -> Vec<StorageId>;
    /// Creates every member storage that doesn't exist yet.
    fn get_or_insert_storages(all_storages: &mut AllStorages);
}

macro_rules! impl_group {
    ($(($member: ident, $index: tt))+) => {
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

#[cfg(test)]
mod tests {
    use crate::{track, Component, Get, IntoIter, View, World};
    use alloc::sync::Arc;
    use alloc::vec::Vec;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    impl Component for A {
        type Tracking = track::Untracked;
    }

    #[derive(Debug, PartialEq)]
    struct B(u32);
    impl Component for B {
        type Tracking = track::Untracked;
    }

    struct C;
    impl Component for C {
        type Tracking = track::Untracked;
    }

    struct D;
    impl Component for D {
        type Tracking = track::Untracked;
    }

    /// Asserts that grouping storages that already hold components attaches one empty range
    /// sharing the same `Arc` to each member, and leaves every component unevaluated and reachable.
    #[test]
    fn basic() {
        let mut world = World::new();
        let e0 = world.add_entity((A(0), B(0)));
        let e1 = world.add_entity(A(1));

        world.group::<(A, B)>();

        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();
        assert_eq!(a.groups.ranges.len(), 1);
        assert_eq!(b.groups.ranges.len(), 1);
        assert!(Arc::ptr_eq(
            &a.groups.ranges[0].group,
            &b.groups.ranges[0].group
        ));
        assert_eq!(a.groups.ranges[0].len, 0);
        assert_eq!(b.groups.ranges[0].len, 0);
        assert_eq!(a.groups.unevaluated_start, 0);
        assert_eq!(b.groups.unevaluated_start, 0);
        assert_eq!(a.get(e0).unwrap(), &A(0));
        assert_eq!(a.get(e1).unwrap(), &A(1));
        assert_eq!(b.get(e0).unwrap(), &B(0));
    }

    /// Asserts that grouping storages that don't exist yet creates them empty.
    #[test]
    fn missing_storages() {
        let mut world = World::new();

        world.group::<(A, B)>();

        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();
        assert!(a.is_empty());
        assert!(b.is_empty());
        assert_eq!(a.groups.ranges.len(), 1);
    }

    /// Asserts that declaring the same group a second time, in a different order,
    /// leaves the original range and `Arc` untouched.
    #[test]
    fn duplicate() {
        let mut world = World::new();

        world.group::<(A, B)>();
        let original = world.borrow::<View<'_, A>>().unwrap().groups.ranges[0]
            .group
            .clone();

        world.group::<(B, A)>();

        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();
        assert_eq!(a.groups.ranges.len(), 1);
        assert_eq!(b.groups.ranges.len(), 1);
        assert!(Arc::ptr_eq(&a.groups.ranges[0].group, &original));
        assert!(Arc::ptr_eq(&b.groups.ranges[0].group, &original));
    }

    /// Asserts that `group` calls can be chained and that each call attaches its own group.
    #[test]
    fn chaining() {
        let mut world = World::new();

        world.group::<(A, B)>().group::<(C, D)>();

        let a = world.borrow::<View<'_, A>>().unwrap();
        let c = world.borrow::<View<'_, C>>().unwrap();
        let d = world.borrow::<View<'_, D>>().unwrap();
        assert_eq!(a.groups.ranges.len(), 1);
        assert_eq!(c.groups.ranges.len(), 1);
        assert!(Arc::ptr_eq(
            &c.groups.ranges[0].group,
            &d.groups.ranges[0].group
        ));
        assert!(!Arc::ptr_eq(
            &a.groups.ranges[0].group,
            &c.groups.ranges[0].group
        ));
    }

    /// Asserts that a group listing the same storage twice is rejected.
    #[test]
    #[should_panic(expected = "a group cannot contain identical storages")]
    fn repeated_storage() {
        let mut world = World::new();

        world.group::<(A, A)>();
    }

    /// Asserts that adding a storage to a second group is rejected until storages can hold several ranges.
    #[test]
    #[should_panic(expected = "a storage can only belong to one group for now")]
    fn second_group() {
        let mut world = World::new();

        world.group::<(A, B)>().group::<(A, C)>();
    }

    /// Asserts that `AllStorages::group` attaches a group like `World::group` does.
    #[test]
    fn all_storages() {
        let world = World::new();

        world.all_storages_mut().unwrap().group::<(A, B)>();

        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();
        assert!(Arc::ptr_eq(
            &a.groups.ranges[0].group,
            &b.groups.ranges[0].group
        ));
    }

    /// Asserts that a `NonSend` member attaches its range to the same storage `NonSend` views borrow.
    #[cfg(feature = "thread_local")]
    #[test]
    fn non_send() {
        use crate::borrow::NonSend;
        use crate::sparse_set::SparseSet;
        use crate::storage::StorageId;

        let mut world = World::new();

        world.group::<(A, NonSend<B>)>();

        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
        let group = &b.groups.ranges[0].group;
        assert!(Arc::ptr_eq(group, &a.groups.ranges[0].group));
        assert!(group.contains(&StorageId::of::<NonSend<SparseSet<B>>>()));
        assert_eq!(
            b.groups.storage_id,
            Some(StorageId::of::<NonSend<SparseSet<B>>>())
        );
    }

    /// Asserts that iterating grouped storages yields the same components as before grouping,
    /// since nothing reads the ranges yet.
    #[test]
    fn iteration() {
        let mut world = World::new();
        world.add_entity((A(0), B(0)));
        world.add_entity(A(1));
        world.add_entity((A(2), B(2)));
        world.add_entity(B(3));

        let collect = |world: &World| {
            let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
            (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect::<Vec<_>>()
        };

        let before = collect(&world);
        world.group::<(A, B)>();
        let after = collect(&world);

        assert_eq!(before, alloc::vec![(0, 0), (2, 2)]);
        assert_eq!(after, before);
    }
}
