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
