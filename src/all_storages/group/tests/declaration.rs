use super::*;

/// Asserts that grouping storages that already hold components attaches one range
/// sharing the same `Arc` to each member, regroups them, and keeps every component reachable.
#[test]
fn basic() {
    let mut world = World::new_for_test();
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
    assert_eq!(a.groups.ranges[0].len, 1);
    assert_eq!(b.groups.ranges[0].len, 1);
    assert_eq!(a.groups.unevaluated_start, 2);
    assert_eq!(b.groups.unevaluated_start, 1);
    assert_eq!(a.get(e0).unwrap(), &A(0));
    assert_eq!(a.get(e1).unwrap(), &A(1));
    assert_eq!(b.get(e0).unwrap(), &B(0));
}

/// Asserts that grouping storages that don't exist yet creates them empty.
#[test]
fn missing_storages() {
    let mut world = World::new_for_test();

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
    let mut world = World::new_for_test();

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
    let mut world = World::new_for_test();

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
    let mut world = World::new_for_test();

    world.group::<(A, A)>();
}

/// Asserts that `AllStorages::group` attaches a group like `World::group` does.
#[test]
fn all_storages() {
    let world = World::new_for_test();

    world.all_storages_mut().unwrap().group::<(A, B)>();

    let a = world.borrow::<View<'_, A>>().unwrap();
    let b = world.borrow::<View<'_, B>>().unwrap();
    assert!(Arc::ptr_eq(
        &a.groups.ranges[0].group,
        &b.groups.ranges[0].group
    ));
}

/// Asserts that iterating grouped storages yields the same components as before grouping,
/// in any order since regroup can reorder them.
#[test]
fn iteration() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0)));
    world.add_entity(A(1));
    world.add_entity((A(2), B(2)));
    world.add_entity(B(3));

    let collect = |world: &World| {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let mut items = (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect::<Vec<_>>();
        items.sort_unstable();
        items
    };

    let before = collect(&world);
    world.group::<(A, B)>();
    let after = collect(&world);

    assert_eq!(before, alloc::vec![(0, 0), (2, 2)]);
    assert_eq!(after, before);
}

/// Storage that isn't a `SparseSet`.
struct Custom(u32);
impl crate::storage::Storage for Custom {}

/// Asserts that a custom storage next to group members is left alone by the group declaration
/// and by regroup, and stays reachable with its content afterwards.
#[test]
fn custom_storage() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0)));
    world
        .all_storages
        .get_mut()
        .exclusive_storage_or_insert_mut(StorageId::of::<Custom>(), || Custom(7));

    world.group::<(A, B)>();
    world.regroup();

    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]), [entity]);
    let custom = world
        .all_storages
        .get_mut()
        .exclusive_storage_or_insert_mut(StorageId::of::<Custom>(), || Custom(0));
    assert_eq!(custom.0, 7);
}

/// Returns the member ids of every group declared when building `world`, in declaration order.
fn declared(world: &mut World) -> Vec<Vec<StorageId>> {
    world
        .all_storages
        .get_mut()
        .grouping
        .groups
        .iter()
        .map(|group| group.to_vec())
        .collect()
}

/// Asserts that declaring a group on the builder creates its missing storages empty,
/// attaches one range sharing the same `Arc` to each member and records the declaration.
#[test]
fn builder() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();

    assert_eq!(declared(&mut world), [group_ids(&[id::<A>(), id::<B>()])]);
    let a = world.borrow::<View<'_, A>>().unwrap();
    let b = world.borrow::<View<'_, B>>().unwrap();
    assert!(a.is_empty());
    assert!(b.is_empty());
    assert_eq!(a.groups.ranges.len(), 1);
    assert_eq!(b.groups.ranges.len(), 1);
    assert!(Arc::ptr_eq(
        &a.groups.ranges[0].group,
        &b.groups.ranges[0].group
    ));
}

/// Asserts that chained builder declarations each attach their own group.
#[test]
fn builder_chaining() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();

    assert_eq!(
        declared(&mut world),
        [
            group_ids(&[id::<A>(), id::<B>()]),
            group_ids(&[id::<C>(), id::<D>()])
        ]
    );
    let a = world.borrow::<View<'_, A>>().unwrap();
    let c = world.borrow::<View<'_, C>>().unwrap();
    let d = world.borrow::<View<'_, D>>().unwrap();
    assert_eq!(a.groups.ranges.len(), 1);
    assert!(Arc::ptr_eq(
        &c.groups.ranges[0].group,
        &d.groups.ranges[0].group
    ));
    assert!(!Arc::ptr_eq(
        &a.groups.ranges[0].group,
        &c.groups.ranges[0].group
    ));
}

/// Asserts that a storage declared in two groups gets its ranges in declaration order,
/// without any union since the world is empty.
#[test]
fn builder_order() {
    let world = World::builder_for_test()
        .with_group::<(A, C)>()
        .with_group::<(A, B)>()
        .build();

    assert_eq!(
        groups_of::<A>(&world),
        [
            group_ids(&[id::<A>(), id::<C>()]),
            group_ids(&[id::<A>(), id::<B>()])
        ]
    );
}

/// Asserts that declaring the same group twice on the builder, in a different order,
/// declares it once.
#[test]
fn builder_duplicate() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, A)>()
        .build();

    assert_eq!(declared(&mut world).len(), 1);
    assert_eq!(groups_of::<A>(&world).len(), 1);
    assert_eq!(groups_of::<B>(&world).len(), 1);
}

/// Asserts that a builder group listing the same storage twice is rejected when declared.
#[test]
#[should_panic(expected = "a group cannot contain identical storages")]
fn builder_repeated_storage() {
    let _ = World::builder_for_test().with_group::<(A, B, A)>();
}

/// Asserts that declarations survive every builder transition, the ones made before
/// and after the transitions all end up attached.
#[test]
fn builder_transitions() {
    let builder = World::builder()
        .with_group::<(A, B)>()
        .with_custom_lock::<parking_lot::RawRwLock>();
    #[cfg(feature = "thread_local")]
    let builder = builder.with_custom_thread_id(|| 0);
    #[cfg(feature = "parallel")]
    let builder = builder.with_local_thread_pool(
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap(),
    );
    let mut world = builder.with_group::<(C, D)>().build();

    assert_eq!(declared(&mut world).len(), 2);
    assert_eq!(groups_of::<A>(&world).len(), 1);
    assert_eq!(groups_of::<B>(&world).len(), 1);
    assert_eq!(groups_of::<C>(&world).len(), 1);
    assert_eq!(groups_of::<D>(&world).len(), 1);
}

/// Asserts that a world built without groups has no declaration.
#[test]
fn builder_none() {
    let mut world = World::builder_for_test().build();

    assert!(declared(&mut world).is_empty());
}
