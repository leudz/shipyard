use super::*;

/// Moves `entity`'s `T` and `U` components to the end of `group`'s range.
fn place<T: Component + Send + Sync, U: Component + Send + Sync>(
    world: &World,
    group: &Arc<[StorageId]>,
    entity: EntityId,
) {
    let (mut t, mut u) = world.borrow::<(ViewMut<'_, T>, ViewMut<'_, U>)>().unwrap();
    t.private_move_to(entity, GroupRegion::Range(group));
    u.private_move_to(entity, GroupRegion::Range(group));
}

/// Moves `entity`'s `T` component to `T`'s unevaluated region, damaging the range it leaves.
fn evict<T: Component + Send + Sync>(world: &World, entity: EntityId) {
    let mut t = world.borrow::<ViewMut<'_, T>>().unwrap();
    t.private_move_to(entity, GroupRegion::Unevaluated);
}

/// Runs the repair pass alone, without evaluating anything.
fn repair(world: &World) {
    let mut all_storages = world.all_storages_mut().unwrap();
    let storages = all_storages.storages.get_mut();
    let storage_ids = sorted_storage_ids(storages);

    repair_all(storages, &storage_ids);
}

/// Returns a world with AB holding `e0`, `e1` and `e2`, AC holding `e3` and `e4`,
/// all aligned and fully trusted, with the entities in index order.
///
/// AC is filled first, so each AB append passes a hole through it and rotates it in A.
fn overlapping() -> (World, Arc<[StorageId]>, Arc<[StorageId]>, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut entities = Vec::new();
    for index in 0..3 {
        entities.push(world.add_entity((A(index), B(index))));
    }
    for index in 3..5 {
        entities.push(world.add_entity((A(index), C)));
    }

    let ab = attach::<A, B>(&world);
    let ac = attach::<A, C>(&world);
    place::<A, C>(&world, &ac, entities[3]);
    place::<A, C>(&world, &ac, entities[4]);
    for &entity in &entities[..3] {
        place::<A, B>(&world, &ab, entity);
    }

    (world, ab, ac, entities)
}

/// Asserts that the `overlapping` fixture holds the layout tests rely on: AC is rotated in A
/// and not in C, both groups are aligned and fully trusted.
#[test]
fn overlapping_fixture() {
    let (world, ab, ac, e) = overlapping();

    assert_ne!(
        world
            .borrow::<View<'_, A>>()
            .unwrap()
            .groups
            .range(&ac)
            .unwrap()
            .shift,
        0
    );
    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3], e[4]], 2));
    assert_eq!(group_of::<C>(&world, &ac), (vec![e[3], e[4]], 2));
}

/// Asserts that repairing aligned, fully trusted groups doesn't move anything.
#[test]
fn repair_aligned() {
    let (world, _, _, _) = overlapping();
    let before = (
        dense_of::<A>(&world),
        dense_of::<B>(&world),
        dense_of::<C>(&world),
    );

    repair(&world);

    let after = (
        dense_of::<A>(&world),
        dense_of::<B>(&world),
        dense_of::<C>(&world),
    );
    assert_eq!(after, before);
}

/// Asserts that repair shrinks each of two overlapping groups to the prefix all its members
/// trust, in every member, even with B shorter than A and AC rotated in A.
#[test]
fn repair_overlapping() {
    let (world, ab, ac, e) = overlapping();
    // B's AB becomes [e0, e2] and only trusts e0. C's AC becomes [e3] and trusts it.
    evict::<B>(&world, e[1]);
    evict::<C>(&world, e[4]);

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0]], 1));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0]], 1));
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3]], 1));
    assert_eq!(group_of::<C>(&world, &ac), (vec![e[3]], 1));
}

/// Asserts that repair evicts each damaged suffix to its own storage's unevaluated region
/// and that every component stays reachable with its value.
#[test]
fn repair_evictions() {
    let (world, _, _, e) = overlapping();
    evict::<B>(&world, e[1]);
    evict::<C>(&world, e[4]);

    repair(&world);

    assert_eq!(unevaluated_of::<A>(&world), [e[1], e[2], e[4]]);
    assert_eq!(unevaluated_of::<B>(&world), [e[1], e[2]]);
    assert_eq!(unevaluated_of::<C>(&world), [e[4]]);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    for (index, &entity) in e.iter().enumerate() {
        assert_eq!(a.get(entity), Ok(&A(index as u32)));
    }
    for (index, &entity) in e[..3].iter().enumerate() {
        assert_eq!(b.get(entity), Ok(&B(index as u32)));
    }
}

/// Asserts that evicting from AB in A passes holes through the rotated AC
/// without changing its order or trust.
#[test]
fn repair_keeps_other_groups() {
    let (world, ab, ac, e) = overlapping();
    evict::<B>(&world, e[2]);

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1]], 2));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1]], 2));
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3], e[4]], 2));
    assert_eq!(group_of::<C>(&world, &ac), (vec![e[3], e[4]], 2));
}

/// Asserts that a member reset by a removal empties its groups in every member,
/// while groups it isn't part of stay as they were.
#[test]
fn repair_reset_member() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(C,)>(e[4]);

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ac), (vec![], 0));
    assert_eq!(group_of::<C>(&world, &ac), (vec![], 0));
    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
    assert_eq!(unevaluated_of::<A>(&world), [e[3], e[4]]);
    assert_eq!(unevaluated_of::<C>(&world), [e[3]]);
}
