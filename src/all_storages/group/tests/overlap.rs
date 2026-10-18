use super::*;

/// Asserts that a storage can be part of two groups, holding a range for each.
#[test]
fn second_group() {
    let mut world = World::new_for_test();

    world.group::<(A, B)>().group::<(A, C)>();

    assert_eq!(
        groups_of::<A>(&world),
        [
            group_ids(&[id::<A>(), id::<B>()]),
            group_ids(&[id::<A>(), id::<C>()])
        ]
    );
    assert_eq!(groups_of::<B>(&world).len(), 1);
    assert_eq!(groups_of::<C>(&world).len(), 1);
}

/// Asserts that declaring AC after AB was populated moves the AB entities holding C
/// to an ABC union, while the others stay in AB or join AC.
#[test]
fn overlap_populated() {
    let mut world = World::new_for_test();
    let abc = world.add_entity((A(0), B(0), C));
    let ab = world.add_entity((A(1), B(1)));
    let ac = world.add_entity((A(2), C));
    world.group::<(A, B)>();
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]).len(), 2);

    world.group::<(A, C)>();

    let all = [id::<A>(), id::<B>(), id::<C>()];
    assert_eq!(range_of::<A>(&world, &all), [abc]);
    assert_eq!(range_of::<B>(&world, &all), [abc]);
    assert_eq!(range_of::<C>(&world, &all), [abc]);
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]), [ab]);
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<C>()]), [ac]);
    assert!(is_evaluated::<A>(&world) && is_evaluated::<B>(&world) && is_evaluated::<C>(&world));
}

/// Asserts that declaring BC connects the disjoint AB and CD,
/// an entity holding all four moving to an ABCD union.
#[test]
fn overlap_connecting() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C, D));
    world.group::<(A, B)>().group::<(C, D)>();
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]), [entity]);

    world.group::<(B, C)>();

    let all = [id::<A>(), id::<B>(), id::<C>(), id::<D>()];
    assert_eq!(range_of::<A>(&world, &all), [entity]);
    assert_eq!(range_of::<D>(&world, &all), [entity]);
    assert!(range_of::<A>(&world, &[id::<A>(), id::<B>()]).is_empty());
    assert!(range_of::<D>(&world, &[id::<C>(), id::<D>()]).is_empty());
}

/// Asserts that a component left ungrouped by an earlier group joins a new group it satisfies.
#[test]
fn overlap_ungrouped() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), C));
    world.group::<(A, B)>();
    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<B>()]), []);

    world.group::<(A, C)>();

    assert_eq!(range_of::<A>(&world, &[id::<A>(), id::<C>()]), [entity]);
    assert_eq!(range_of::<C>(&world, &[id::<A>(), id::<C>()]), [entity]);
}

/// Asserts that declaring an existing group again doesn't regroup,
/// even when other components are waiting for it.
#[test]
fn duplicate_unevaluated() {
    let mut world = World::new_for_test();
    world.group::<(A, B)>().group::<(A, C)>();
    world.add_entity((A(0), B(0), C));

    world.group::<(B, A)>();

    assert!(!is_evaluated::<A>(&world));
    assert!(!is_evaluated::<C>(&world));
}

/// Asserts that declaring a group equal to an existing union reuses the union,
/// without attaching anything or regrouping.
#[test]
fn duplicate_union() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C));
    world.group::<(A, B)>().group::<(A, C)>();
    let union = world.borrow::<View<'_, A>>().unwrap().groups.ranges[2]
        .group
        .clone();
    world.add_entity((A(1), B(1), C));

    world.group::<(C, B, A)>();

    let a = world.borrow::<View<'_, A>>().unwrap();
    assert_eq!(a.groups.ranges.len(), 3);
    assert!(Arc::ptr_eq(&a.groups.ranges[2].group, &union));
    assert_eq!(a.dense[a.groups.ranges[2].start], entity);
    assert_ne!(a.groups.unevaluated_start, a.len());
}

/// Asserts that overlapping groups can be declared on empty storages.
#[test]
fn overlap_empty() {
    let mut world = World::new_for_test();

    world.group::<(A, B)>().group::<(A, C)>().group::<(C, D)>();

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
    let mut ab_first = World::new_for_test();
    populate(&mut ab_first);
    ab_first.group::<(A, B)>().group::<(A, C)>();
    let mut ac_first = World::new_for_test();
    populate(&mut ac_first);
    ac_first.group::<(A, C)>().group::<(A, B)>();

    for ids in [
        [id::<A>(), id::<B>()].as_slice(),
        &[id::<A>(), id::<C>()],
        &[id::<A>(), id::<B>(), id::<C>()],
    ] {
        assert_eq!(range_of::<A>(&ab_first, ids), range_of::<A>(&ac_first, ids));
    }
}
