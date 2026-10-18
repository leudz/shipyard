use super::*;

/// Returns the group of `T`'s range whose members are `ids`.
fn find<T: Component + Send + Sync>(world: &World, ids: &[StorageId]) -> Arc<[StorageId]> {
    let ids = sorted_ids(ids.to_vec());

    groups_of::<T>(world)
        .into_iter()
        .find(|group| group[..] == ids[..])
        .expect("the group is attached to the storage")
}

/// Asserts that an entity of AB gaining C moves to an ABC union in A, B and C,
/// while AB keeps the others, aligned and fully trusted.
#[test]
fn promote_gained() {
    let mut world = World::new_for_test();
    let e0 = world.add_entity((A(0), B(0)));
    let e1 = world.add_entity((A(1), B(1)));
    let e2 = world.add_entity((A(2), B(2)));
    world.add_entity((A(3), C));
    let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);
    world.regroup();

    world.add_component(e1, C);
    world.regroup();

    assert_regrouped(&world);
    let abc = find::<A>(&world, &[id::<A>(), id::<B>(), id::<C>()]);
    assert_eq!(group_of::<A>(&world, &abc).0, [e1]);
    assert_eq!(group_of::<B>(&world, &abc).0, [e1]);
    assert_eq!(group_of::<C>(&world, &abc).0, [e1]);
    assert_eq!(sorted(group_of::<A>(&world, &ab).0), [e0, e2]);
}

/// Asserts that with AB, BC and CD, an entity of AB gaining C and D moves straight to ABCD,
/// without creating ABC or BCD on the way.
#[test]
fn promote_transitive() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0)));
    world.add_entity((C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<B>(), id::<C>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);
    world.regroup();

    world.add_component(entity, (C, D));
    world.regroup();

    assert_regrouped(&world);
    let abcd = find::<A>(&world, &[id::<A>(), id::<B>(), id::<C>(), id::<D>()]);
    assert_eq!(group_of::<D>(&world, &abcd).0, [entity]);
    assert_eq!(groups_of::<B>(&world).len(), 3);
    assert_eq!(groups_of::<C>(&world).len(), 3);
}

/// Asserts that an entity held by the disjoint AB and CD moves to ABCDE once gaining E
/// connects them through BE and EC, leaving both AB and CD.
#[test]
fn promote_two_sources() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C, D));
    let other = world.add_entity((A(1), B(1), C, D));
    world.add_entity((B(2), E));
    let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);
    let cd = attach_ids(&world, &[id::<C>(), id::<D>()]);
    attach_ids(&world, &[id::<B>(), id::<E>()]);
    attach_ids(&world, &[id::<E>(), id::<C>()]);
    world.regroup();
    assert_eq!(
        sorted(group_of::<A>(&world, &ab).0),
        sorted(vec![entity, other])
    );

    world.add_component(entity, E);
    world.regroup();

    assert_regrouped(&world);
    let all = [id::<A>(), id::<B>(), id::<C>(), id::<D>(), id::<E>()];
    let abcde = find::<E>(&world, &all);
    assert_eq!(group_of::<A>(&world, &abcde).0, [entity]);
    assert_eq!(group_of::<C>(&world, &abcde).0, [entity]);
    assert_eq!(group_of::<A>(&world, &ab).0, [other]);
    assert_eq!(group_of::<D>(&world, &cd).0, [other]);
}

/// Asserts that an entity whose components come from a range, the ungrouped region and the
/// unevaluated region all at once ends in the same union: with AB, AC and CD, an AB entity
/// holding an ungrouped D gains C.
#[test]
fn promote_mixed_sources() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), D));
    world.add_entity((A(1), C));
    world.add_entity((C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);
    world.regroup();
    assert_eq!(ungrouped_of::<D>(&world), [entity]);

    world.add_component(entity, C);
    world.regroup();

    assert_regrouped(&world);
    let abcd = find::<A>(&world, &[id::<A>(), id::<B>(), id::<C>(), id::<D>()]);
    for members in [
        group_of::<A>(&world, &abcd).0,
        group_of::<B>(&world, &abcd).0,
        group_of::<C>(&world, &abcd).0,
        group_of::<D>(&world, &abcd).0,
    ] {
        assert_eq!(members, [entity]);
    }
    assert!(ungrouped_of::<D>(&world).is_empty());
}

/// Builds a world with AB and AC holding entities in every combination,
/// some of them gaining components after a first regroup.
fn overlapping_world() -> World {
    let mut world = World::new_for_test();
    let mut entities = Vec::new();
    for index in 0..24 {
        let entity = match index % 4 {
            0 => world.add_entity((A(index), B(index))),
            1 => world.add_entity((A(index), C)),
            2 => world.add_entity((A(index), B(index), C)),
            _ => world.add_entity((B(index), C)),
        };
        entities.push(entity);
    }
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);
    world.regroup();

    for &entity in entities.iter().step_by(4) {
        world.add_component(entity, C);
    }
    for &entity in entities.iter().skip(1).step_by(8) {
        world.add_component(entity, B(100));
    }

    world
}

/// Asserts that regrouping overlapping groups leaves every storage fully evaluated
/// with every range aligned and fully trusted, every entity holding A, B and C in ABC.
#[test]
fn regroup_overlapping() {
    let mut world = overlapping_world();

    world.regroup();

    assert_regrouped(&world);
    let abc = find::<A>(&world, &[id::<A>(), id::<B>(), id::<C>()]);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let mut expected: Vec<EntityId> = (&a, &b, &c).iter().ids().collect();
    expected.sort_unstable();
    drop((a, b, c));
    assert_eq!(sorted(group_of::<C>(&world, &abc).0), expected);
}

/// Asserts that a second regroup without any change doesn't move anything.
#[test]
fn regroup_twice() {
    let mut world = overlapping_world();
    world.regroup();
    let before = (
        dense_of::<A>(&world),
        dense_of::<B>(&world),
        dense_of::<C>(&world),
    );

    world.regroup();

    let after = (
        dense_of::<A>(&world),
        dense_of::<B>(&world),
        dense_of::<C>(&world),
    );
    assert_eq!(after, before);
}

/// Asserts that iterating overlapping groups yields the same items before and after regroup.
#[test]
fn iteration_overlapping() {
    let mut world = overlapping_world();
    let collect = |world: &World| {
        let (a, b, c) = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();
        let mut ab: Vec<(u32, u32)> = (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect();
        let mut ac: Vec<u32> = (&a, &c).iter().map(|(a, _)| a.0).collect();
        let mut abc: Vec<u32> = (&a, &b, &c).iter().map(|(a, _, _)| a.0).collect();
        ab.sort_unstable();
        ac.sort_unstable();
        abc.sort_unstable();
        (ab, ac, abc)
    };

    let before = collect(&world);
    world.regroup();
    let after = collect(&world);

    assert_eq!(after, before);
}

/// Asserts that promotions keep insertion and modification states and call no callback.
#[test]
fn promote_tracking() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    let mut world = World::new_for_test();
    world.add_entity(TrackedA(0));
    let old = world.add_entity((TrackedA(1), TrackedB));
    world.run(|a: ViewMut<'_, TrackedA, track::All>| a.clear_all_inserted());
    let new = world.add_entity((TrackedA(2), TrackedB));
    world.run(|mut a: ViewMut<'_, TrackedA, track::All>| {
        (&mut a).get(old).unwrap().0 = 10;
    });
    world.add_entity((TrackedA(3), C));
    attach_ids(&world, &[id::<TrackedA>(), id::<TrackedB>()]);
    attach_ids(&world, &[id::<TrackedA>(), id::<C>()]);

    let calls = Arc::new(AtomicUsize::new(0));
    {
        let mut a = world.borrow::<ViewMut<'_, TrackedA, track::All>>().unwrap();
        let insertions = calls.clone();
        a.on_insertion(move |_, _| {
            insertions.fetch_add(1, Ordering::Relaxed);
        });
        let removals = calls.clone();
        a.on_removal(move |_, _| {
            removals.fetch_add(1, Ordering::Relaxed);
        });
    }

    let state = |world: &World| {
        let a = world.borrow::<View<'_, TrackedA, track::All>>().unwrap();
        [
            a.is_inserted(old),
            a.is_inserted(new),
            a.is_modified(old),
            a.is_modified(new),
        ]
    };
    let before = state(&world);

    world.regroup();
    world.add_component(old, C);
    world.add_component(new, C);
    world.regroup();

    assert_eq!(before, [false, true, true, false]);
    assert_eq!(state(&world), before);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let abc = find::<C>(&world, &[id::<TrackedA>(), id::<TrackedB>(), id::<C>()]);
    assert_eq!(
        sorted(group_of::<C>(&world, &abc).0),
        sorted(vec![old, new])
    );
}
