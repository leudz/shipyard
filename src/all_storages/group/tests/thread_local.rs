use super::*;

/// Asserts that a `NonSend` member can be part of overlapping groups,
/// an entity satisfying both ending in their union.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_overlap() {
    use crate::borrow::NonSend;
    use crate::{AddComponent, ViewMut};

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .with_group::<(A, C)>()
        .build();
    let entity = world.add_entity((A(0), C));
    world
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B(0));

    world.regroup();

    let all = [
        id::<A>(),
        StorageId::of::<NonSend<SparseSet<B>>>(),
        id::<C>(),
    ];
    assert_eq!(range_of::<A>(&world, &all), [entity]);
    assert_eq!(range_of::<C>(&world, &all), [entity]);
    let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
    assert_eq!(b.groups.ranges.len(), 2);
    assert_eq!(b.dense[b.groups.ranges[1].start], entity);
}

/// Asserts that each thread-local wrapper can be declared on the builder,
/// its range attached to the wrapper's own storage and shared with its partner,
/// without creating the `Send + Sync` storage of the same component.
#[test]
fn builder_wrappers() {
    use crate::borrow::{NonSend, NonSendSync, NonSync};
    use crate::sparse_set::SparseSet;
    use crate::storage::StorageId;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .with_group::<(A, NonSync<C>)>()
        .with_group::<(A, NonSendSync<D>)>()
        .build();

    let b_id = StorageId::of::<NonSend<SparseSet<B>>>();
    let c_id = StorageId::of::<NonSync<SparseSet<C>>>();
    let d_id = StorageId::of::<NonSendSync<SparseSet<D>>>();
    let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
    let c = world.borrow::<NonSync<View<'_, C>>>().unwrap();
    let d = world.borrow::<NonSendSync<View<'_, D>>>().unwrap();
    assert_eq!(b.groups.storage_id, Some(b_id));
    assert_eq!(c.groups.storage_id, Some(c_id));
    assert_eq!(d.groups.storage_id, Some(d_id));
    assert_eq!(&b.groups.ranges[0].group[..], group_ids(&[id::<A>(), b_id]));
    assert_eq!(&c.groups.ranges[0].group[..], group_ids(&[id::<A>(), c_id]));
    assert_eq!(&d.groups.ranges[0].group[..], group_ids(&[id::<A>(), d_id]));
    drop((b, c, d));

    let storages = world.all_storages.get_mut().storages.get_mut();
    assert!(storages.any(&id::<B>()).is_none());
    assert!(storages.any(&id::<C>()).is_none());
    assert!(storages.any(&id::<D>()).is_none());
}

/// Asserts that each thread-local view tells the scheduler the id of the storage it borrows,
/// the same id the matching group member uses, so workloads can match borrows with group members.
#[test]
fn view_ids() {
    use crate::all_storages::GroupMember;
    use crate::borrow::{BorrowInfo, NonSend, NonSendSync, NonSync};
    use crate::views::ViewMut;

    fn id_of<V: BorrowInfo>() -> StorageId {
        let mut info = Vec::new();
        V::borrow_info(&mut info);
        info[0].storage_id
    }

    assert_eq!(id_of::<NonSend<View<'_, A>>>(), NonSend::<A>::storage_id());
    assert_eq!(
        id_of::<NonSend<ViewMut<'_, A>>>(),
        NonSend::<A>::storage_id()
    );
    assert_eq!(id_of::<NonSync<View<'_, A>>>(), NonSync::<A>::storage_id());
    assert_eq!(
        id_of::<NonSync<ViewMut<'_, A>>>(),
        NonSync::<A>::storage_id()
    );
    assert_eq!(
        id_of::<NonSendSync<View<'_, A>>>(),
        NonSendSync::<A>::storage_id()
    );
    assert_eq!(
        id_of::<NonSendSync<ViewMut<'_, A>>>(),
        NonSendSync::<A>::storage_id()
    );
}

/// Returns the connected set holding `A`'s storage.
fn set_of_a(world: &mut World) -> crate::all_storages::group::ConnectedSet {
    world
        .all_storages
        .get_mut()
        .grouping
        .set_of(&id::<A>())
        .cloned()
        .unwrap()
}

/// Asserts that a `NonSend` member is in its set under its wrapper's storage id,
/// and makes the set regroupable from the world's thread only.
#[test]
fn set_non_send() {
    use crate::borrow::NonSend;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();

    let set = set_of_a(&mut world);
    let b_id = StorageId::of::<NonSend<SparseSet<B>>>();
    assert_eq!(set.members, group_ids(&[id::<A>(), b_id]));
    assert!(!set.thread_safe);
}

/// Asserts that a `NonSync` member can be borrowed exclusively from any thread,
/// so it doesn't restrict its set.
#[test]
fn set_non_sync() {
    use crate::borrow::NonSync;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSync<B>)>()
        .build();

    assert!(set_of_a(&mut world).thread_safe);
}

/// Asserts that a `NonSendSync` member makes its set regroupable from the world's thread only.
#[test]
fn set_non_send_sync() {
    use crate::borrow::NonSendSync;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSendSync<B>)>()
        .build();

    assert!(!set_of_a(&mut world).thread_safe);
}

/// Asserts that a `NonSend` member restricts its whole set, including members it doesn't share
/// a group with: `A` is only grouped with `B`, but `B` links it to `NonSend<C>`.
#[test]
fn set_non_send_transitive() {
    use crate::borrow::NonSend;

    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, NonSend<C>)>()
        .build();

    assert!(!set_of_a(&mut world).thread_safe);
}
