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

/// Asserts that a removal from B's AB lowers B's trust, and repair shrinks AB to that trust in
/// both members while the rotated AC keeps its order and trust.
#[test]
fn repair_removal() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(B,)>(e[1]);
    // B's AB isn't rotated, so its physical last component is e2 and only e0 is unchanged.
    assert_eq!(group_of::<B>(&world, &ab).1, 1);

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0]], 1));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0]], 1));
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3], e[4]], 2));
    assert_eq!(group_of::<C>(&world, &ac), (vec![e[3], e[4]], 2));
    assert_eq!(unevaluated_of::<A>(&world), [e[1], e[2]]);
    assert_eq!(unevaluated_of::<B>(&world), [e[2]]);
}

/// Asserts that a removal from a rotated range lowers its trust to what stays unchanged:
/// in A, AC's physical last slot holds e3, so removing e4 leaves nothing trusted.
/// Repair then empties AC in both members, AC stays attached and AB is untouched.
#[test]
fn repair_removal_rotated() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(A,)>(e[4]);
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3]], 0));

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ac), (vec![], 0));
    assert_eq!(group_of::<C>(&world, &ac), (vec![], 0));
    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
    assert_eq!(unevaluated_of::<A>(&world), [e[3]]);
    assert_eq!(unevaluated_of::<C>(&world), [e[3], e[4]]);
}

/// Asserts that with removals in several members, repair shrinks each group to the smallest
/// trust of its members: A's AB keeps 2 positions and B's all 3, C's AC keeps 1 and A's both.
#[test]
fn repair_removals_minimum() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(A,)>(e[2]);
    world.remove::<(C,)>(e[4]);
    assert_eq!(group_of::<A>(&world, &ab).1, 2);
    assert_eq!(group_of::<B>(&world, &ab).1, 3);
    assert_eq!(group_of::<A>(&world, &ac).1, 2);
    assert_eq!(group_of::<C>(&world, &ac).1, 1);

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1]], 2));
    assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1]], 2));
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3]], 1));
    assert_eq!(group_of::<C>(&world, &ac), (vec![e[3]], 1));
}

/// Asserts that removing a group's components until its range is empty in one member
/// leaves it attached and empty in every member once repaired.
#[test]
fn repair_emptied() {
    let (mut world, ab, _, e) = overlapping();
    for &entity in &e[..3] {
        world.remove::<(B,)>(entity);
    }

    repair(&world);

    assert_eq!(group_of::<A>(&world, &ab), (vec![], 0));
    assert_eq!(group_of::<B>(&world, &ab), (vec![], 0));
    assert_eq!(unevaluated_of::<A>(&world), [e[0], e[1], e[2]]);
}

/// Returns the entities owning a `T` and a `U`, sorted.
///
/// With `with_v`, they also own a `V`. Otherwise they lack one.
fn owning<T: Component + Send + Sync, U: Component + Send + Sync, V: Component + Send + Sync>(
    world: &World,
    with_v: bool,
) -> Vec<EntityId> {
    let (t, u, v) = world
        .borrow::<(View<'_, T>, View<'_, U>, View<'_, V>)>()
        .unwrap();

    sorted(
        t.dense
            .iter()
            .copied()
            .filter(|&entity| u.contains(entity) && v.contains(entity) == with_v)
            .collect(),
    )
}

/// Asserts that a full regroup after removals, deletions and a retain over overlapping groups
/// and their ABC union restores aligned, fully trusted ranges and an empty unevaluated region,
/// with every entity in the group matching exactly the components it still owns.
#[test]
fn regroup_after_removals() {
    let mut world = World::new_for_test();
    let mut entities = Vec::new();
    for index in 0..24 {
        let entity = match index % 3 {
            0 => world.add_entity((A(index), B(index))),
            1 => world.add_entity((A(index), C)),
            _ => world.add_entity((A(index), B(index), C)),
        };
        entities.push(entity);
    }
    let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);
    let ac = attach_ids(&world, &[id::<A>(), id::<C>()]);
    world.regroup();
    let abc = groups_of::<A>(&world)
        .into_iter()
        .find(|group| group.len() == 3)
        .unwrap();

    // Removals in every member, from AB, AC and the union, then deletions and a retain.
    world.remove::<(B,)>(entities[0]);
    world.remove::<(C,)>(entities[2]);
    world.remove::<(A,)>(entities[4]);
    world.remove::<(C,)>(entities[7]);
    world.delete_component::<(B,)>(entities[11]);
    world.delete_entity(entities[14]);
    world.delete_entity(entities[3]);
    world.retain::<A>(|_, a| a.0 % 5 != 0);

    world.regroup();

    assert_regrouped(&world);
    assert_eq!(
        sorted(group_of::<A>(&world, &abc).0),
        owning::<A, B, C>(&world, true)
    );
    assert_eq!(
        sorted(group_of::<A>(&world, &ab).0),
        owning::<A, B, C>(&world, false)
    );
    assert_eq!(
        sorted(group_of::<C>(&world, &ac).0),
        owning::<A, C, B>(&world, false)
    );
}

/// Asserts that removals from one of two disjoint groups don't move the other group's
/// components, and that regroup aligns the damaged one again.
#[test]
fn regroup_disjoint_after_removals() {
    let mut world = World::new_for_test();
    let entities: Vec<EntityId> = (0..8)
        .map(|index| world.add_entity((A(index), B(index), C, D)))
        .collect();
    world.group::<(A, B)>();
    world.group::<(C, D)>();
    let before = (dense_of::<C>(&world), dense_of::<D>(&world));

    world.remove::<(A,)>(entities[1]);
    world.remove::<(B,)>(entities[6]);
    world.regroup();

    assert_regrouped(&world);
    assert_eq!((dense_of::<C>(&world), dense_of::<D>(&world)), before);
}
