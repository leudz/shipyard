use super::*;

/// Asserts that a `NonSend` member attaches its range to the same storage `NonSend` views borrow.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::borrow::NonSend;
    use crate::sparse_set::SparseSet;
    use crate::storage::StorageId;

    let mut world = World::new_for_test();

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

/// Asserts that a `NonSend` member can be part of overlapping groups,
/// an entity satisfying both ending in their union.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_overlap() {
    use crate::borrow::NonSend;
    use crate::{AddComponent, ViewMut};

    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), C));
    world
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B(0));

    world.group::<(A, NonSend<B>)>().group::<(A, C)>();

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

/// Asserts that `NonSync` and `NonSendSync` members attach their range to their wrapper's own
/// storage, sharing the group with their `Send + Sync` partner, without creating the
/// `Send + Sync` storage of the same component.
#[test]
fn non_sync_and_non_send_sync() {
    use crate::borrow::{NonSendSync, NonSync};
    use crate::sparse_set::SparseSet;
    use crate::storage::StorageId;

    let mut world = World::new_for_test();

    world
        .group::<(A, NonSync<C>)>()
        .group::<(B, NonSendSync<D>)>();

    let c = world.borrow::<NonSync<View<'_, C>>>().unwrap();
    let d = world.borrow::<NonSendSync<View<'_, D>>>().unwrap();
    let c_id = StorageId::of::<NonSync<SparseSet<C>>>();
    let d_id = StorageId::of::<NonSendSync<SparseSet<D>>>();
    assert_eq!(c.groups.storage_id, Some(c_id));
    assert_eq!(d.groups.storage_id, Some(d_id));
    assert_eq!(&c.groups.ranges[0].group[..], group_ids(&[id::<A>(), c_id]));
    assert_eq!(&d.groups.ranges[0].group[..], group_ids(&[id::<B>(), d_id]));
    drop((c, d));

    let storages = world.all_storages.get_mut().storages.get_mut();
    assert!(!storages.contains_key(&id::<C>()));
    assert!(!storages.contains_key(&id::<D>()));
}
