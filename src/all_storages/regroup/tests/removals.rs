use super::*;

/// Removes `entity`'s `T` and adds it back, the component lands in `T`'s unevaluated region.
fn reinsert<T: Component + Send + Sync>(world: &mut World, entity: EntityId) {
    let (component,) = world.remove::<(T,)>(entity);
    world.add_component(entity, component.unwrap());
}

/// Returns a world with AB holding `e0`, `e1` and `e2`, AC holding `e3` and `e4`,
/// all aligned and fully trusted, with the entities in index order.
///
/// `e2` joins AB after the first regroup, so AC slides by one slot in A and gets rotated.
fn overlapping() -> (World, Arc<[StorageId]>, Arc<[StorageId]>, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut entities = Vec::new();
    for index in 0..2 {
        entities.push(world.add_entity((A(index), B(index))));
    }
    for index in 3..5 {
        entities.push(world.add_entity((A(index), C)));
    }

    let ab = attach::<A, B>(&world);
    let ac = attach::<A, C>(&world);
    world.regroup();
    entities.insert(2, world.add_entity((A(2), B(2))));
    world.regroup();

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

/// The `(A, B)` and `(A, C)` groups, for [`assert_memberships`].
fn ab_ac() -> [[StorageId; 2]; 2] {
    [ab(), [id::<A>(), id::<C>()]]
}

/// Regroups `world`, then panics unless every group is aligned and fully trusted
/// and every entity sits where its components say, with AB and AC declared.
fn regroup(world: &mut World) {
    world.regroup();

    assert_regrouped(world);
    let [ab, ac] = ab_ac();
    assert_memberships(world, &[&ab, &ac]);
}

/// Asserts that regrouping aligned, fully trusted groups doesn't move anything,
/// even with AC rotated in A.
#[test]
fn aligned() {
    let (mut world, _, _, _) = overlapping();
    let before = (
        dense_of::<A>(&world),
        dense_of::<B>(&world),
        dense_of::<C>(&world),
    );

    regroup(&mut world);

    let after = (
        dense_of::<A>(&world),
        dense_of::<B>(&world),
        dense_of::<C>(&world),
    );
    assert_eq!(after, before);
}

/// Asserts that with components moved out of both groups to their unevaluated region,
/// each group keeps the prefix all its members trust and gets its entities back,
/// even with B shorter than A and AC rotated in A.
#[test]
fn evicted() {
    let (mut world, ab, ac, e) = overlapping();
    // B's AB becomes [e0, e2] and only trusts e0. C's AC becomes [e3] and trusts it.
    reinsert::<B>(&mut world, e[1]);
    reinsert::<C>(&mut world, e[4]);

    regroup(&mut world);

    assert_eq!(sorted(group_of::<A>(&world, &ab).0), e[..3]);
    assert_eq!(group_of::<A>(&world, &ab).0[0], e[0]);
    assert_eq!(sorted(group_of::<A>(&world, &ac).0), e[3..]);
    assert_eq!(group_of::<A>(&world, &ac).0[0], e[3]);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    for (index, &entity) in e.iter().enumerate() {
        assert_eq!(a.get(entity), Ok(&A(index as u32)));
    }
    for (index, &entity) in e[..3].iter().enumerate() {
        assert_eq!(b.get(entity), Ok(&B(index as u32)));
    }
}

/// Asserts that a component moved out of AB's end in B comes back to it,
/// while the rotated AC keeps its order.
#[test]
fn other_groups_kept() {
    let (mut world, ab, ac, e) = overlapping();
    reinsert::<B>(&mut world, e[2]);

    regroup(&mut world);

    assert_eq!(group_of::<A>(&world, &ab).0, e[..3]);
    assert_eq!(group_of::<A>(&world, &ac).0, e[3..]);
    assert_eq!(group_of::<C>(&world, &ac).0, e[3..]);
}

/// Asserts that a removal from B's AB keeps the prefix both members trust,
/// while the rotated AC keeps its order.
#[test]
fn removal() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(B,)>(e[1]);
    // B's AB isn't rotated, so its physical last component is e2 and only e0 is unchanged.
    assert_eq!(group_of::<B>(&world, &ab).1, 1);

    regroup(&mut world);

    assert_eq!(group_of::<A>(&world, &ab).0[0], e[0]);
    assert_eq!(sorted(group_of::<A>(&world, &ab).0), [e[0], e[2]]);
    assert_eq!(group_of::<A>(&world, &ac).0, e[3..]);
    assert_eq!(ungrouped_of::<A>(&world), [e[1]]);
}

/// Asserts that a removal from a rotated range that leaves nothing trusted,
/// in A, AC's physical last slot holds e3 so removing e4 does, regroups what's left
/// while AB keeps its order.
#[test]
fn removal_rotated() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(A,)>(e[4]);
    assert_eq!(group_of::<A>(&world, &ac), (vec![e[3]], 0));

    regroup(&mut world);

    assert_eq!(group_of::<A>(&world, &ac).0, [e[3]]);
    assert_eq!(group_of::<C>(&world, &ac).0, [e[3]]);
    assert_eq!(group_of::<A>(&world, &ab).0, e[..3]);
    assert_eq!(ungrouped_of::<C>(&world), [e[4]]);
}

/// Asserts that with removals in several members, each group keeps the prefix all its members
/// trust: AB keeps e0 and e1, AC keeps e3.
#[test]
fn removals_minimum() {
    let (mut world, ab, ac, e) = overlapping();
    world.remove::<(A,)>(e[2]);
    world.remove::<(C,)>(e[4]);
    assert_eq!(group_of::<A>(&world, &ab).1, 2);
    assert_eq!(group_of::<B>(&world, &ab).1, 3);
    assert_eq!(group_of::<A>(&world, &ac).1, 2);
    assert_eq!(group_of::<C>(&world, &ac).1, 1);

    regroup(&mut world);

    assert_eq!(group_of::<B>(&world, &ab).0, [e[0], e[1]]);
    assert_eq!(group_of::<A>(&world, &ac).0, [e[3]]);
}

/// Asserts that removing a group's components until its range is empty in one member
/// leaves it attached and empty in every member once regrouped.
#[test]
fn emptied() {
    let (mut world, ab, _, e) = overlapping();
    for &entity in &e[..3] {
        world.remove::<(B,)>(entity);
    }

    regroup(&mut world);

    assert_eq!(group_of::<A>(&world, &ab), (vec![], 0));
    assert_eq!(group_of::<B>(&world, &ab), (vec![], 0));
    assert_eq!(ungrouped_of::<A>(&world), e[..3]);
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
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    let entities: Vec<EntityId> = (0..8)
        .map(|index| world.add_entity((A(index), B(index), C, D)))
        .collect();
    world.regroup();
    world.regroup();
    let before = (dense_of::<C>(&world), dense_of::<D>(&world));

    world.remove::<(A,)>(entities[1]);
    world.remove::<(B,)>(entities[6]);
    world.regroup();

    assert_regrouped(&world);
    assert_eq!((dense_of::<C>(&world), dense_of::<D>(&world)), before);
}
