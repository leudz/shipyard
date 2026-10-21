use super::*;

/// Asserts that with overlapping AB and AC, regroup moves the entities holding A, B and C
/// to an ABC union, while the others join AB or AC.
#[test]
fn overlap_populated() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let abc = world.add_entity((A(0), B(0), C));
    let ab = world.add_entity((A(1), B(1)));
    let ac = world.add_entity((A(2), C));

    world.regroup();

    let all = [id::<A>(), id::<B>(), id::<C>()];
    assert_eq!(range_of::<A>(&world, &all), [abc]);
    assert_eq!(range_of::<B>(&world, &all), [abc]);
    assert_eq!(range_of::<C>(&world, &all), [abc]);
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]), [ab]);
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<C>()]), [ac]);
    assert!(is_evaluated::<A>(&world) && is_evaluated::<B>(&world) && is_evaluated::<C>(&world));
}

/// Asserts that BC connects AB and CD, an entity holding all four moving to an ABCD union
/// instead of staying in AB and CD.
#[test]
fn overlap_connecting() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .with_group::<(B, C)>()
        .build();
    let entity = world.add_entity((A(0), B(0), C, D));

    world.regroup();

    let all = [id::<A>(), id::<B>(), id::<C>(), id::<D>()];
    assert_eq!(range_of::<A>(&world, &all), [entity]);
    assert_eq!(range_of::<D>(&world, &all), [entity]);
    assert!(range_of::<A>(&world, &[id::<A>(), id::<B>()]).is_empty());
    assert!(range_of::<D>(&world, &[id::<C>(), id::<D>()]).is_empty());
}

/// Asserts that an entity satisfying only one of two overlapping groups joins that group alone.
#[test]
fn overlap_partial() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let entity = world.add_entity((A(0), C));

    world.regroup();

    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]), []);
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<C>()]), [entity]);
    assert_eq!(range_of::<C>(&world, &[id::<A>(), id::<C>()]), [entity]);
}

/// Asserts that a declared group equal to the union of two others is used as their union,
/// regroup doesn't attach another range for it.
#[test]
fn declared_union() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .with_group::<(C, B, A)>()
        .build();
    let declared = world.borrow::<View<'_, A>>().unwrap().groups.ranges[2]
        .group
        .clone();
    let entity = world.add_entity((A(0), B(0), C));

    world.regroup();

    let a = world.borrow::<View<'_, A>>().unwrap();
    assert_eq!(a.groups.ranges.len(), 3);
    assert!(Arc::ptr_eq(&a.groups.ranges[2].group, &declared));
    assert_eq!(a.dense[a.groups.ranges[2].start], entity);
}

/// Asserts that overlapping groups on empty storages attach a range per group to each member.
#[test]
fn overlap_empty() {
    let world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .with_group::<(C, D)>()
        .build();

    assert_eq!(groups_of::<A>(&world).len(), 2);
    assert_eq!(groups_of::<C>(&world).len(), 2);
    assert!(world.borrow::<View<'_, A>>().unwrap().is_empty());
}

/// Asserts that the order of declarations doesn't change which entities end in which group.
#[test]
fn overlap_order() {
    let populate = |world: &mut World| {
        for index in 0..12 {
            match index % 3 {
                0 => world.add_entity((A(index), B(index), C)),
                1 => world.add_entity((A(index), B(index))),
                _ => world.add_entity((A(index), C)),
            };
        }
    };
    let mut ab_first = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    populate(&mut ab_first);
    ab_first.regroup();
    let mut ac_first = World::builder_for_test()
        .with_group::<(A, C)>()
        .with_group::<(A, B)>()
        .build();
    populate(&mut ac_first);
    ac_first.regroup();

    for ids in [
        [id::<A>(), id::<B>()].as_slice(),
        &[id::<A>(), id::<C>()],
        &[id::<A>(), id::<B>(), id::<C>()],
    ] {
        assert_eq!(range_of::<A>(&ab_first, ids), range_of::<A>(&ac_first, ids));
    }
}
