use super::*;

/// Storage that isn't a `SparseSet`.
struct Custom;
impl Storage for Custom {}

/// Returns the plan of `group`, as `(kept, leaving, incoming)`.
fn counts(plan: &Plan, group: &Arc<[StorageId]>) -> (usize, usize, usize) {
    let group_plan = &plan.groups[plan.group_index(group)];

    (group_plan.kept, group_plan.leaving, group_plan.incoming)
}

/// Returns the plan of the group made of `members`, as `(kept, leaving, incoming)`.
fn counts_of(plan: &Plan, members: Vec<StorageId>) -> (usize, usize, usize) {
    let members = sorted_ids(members);
    let group_plan = plan
        .groups
        .iter()
        .find(|group_plan| *group_plan.group == *members)
        .unwrap();

    (group_plan.kept, group_plan.leaving, group_plan.incoming)
}

/// Returns the `(A, B)` group of a `pairs` world, with its entities in logical order.
fn ab_order(world: &World) -> (Arc<[StorageId]>, Vec<EntityId>) {
    let group = groups_of::<A>(world)[0].clone();
    let (order, _) = group_of::<A>(world, &group);

    (group, order)
}

/// Asserts that a regrouped world, and a world whose storages have no group,
/// have nothing to analyze.
#[test]
fn clean() {
    let (world, _) = pairs(4);
    world
        .add_custom_storage(StorageId::of::<Custom>(), Custom)
        .unwrap();
    let mut ungrouped = World::new_for_test();
    ungrouped.add_entity((A(0), B(0)));

    assert!(analyze_world(&world).is_none());
    assert!(analyze_world(&ungrouped).is_none());
}

/// Asserts that removing B from the logical last pair keeps every other pair,
/// nothing else is counted.
#[test]
fn tail() {
    let (mut world, _) = pairs(6);
    let (group, order) = ab_order(&world);
    world.remove::<(B,)>(order[5]);

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (5, 0, 0));
}

/// Asserts that a B removal in the middle of the range keeps the pairs before it
/// and counts the ones after it again, next to an ungrouped storage and a custom one
/// that the analysis skips.
#[test]
fn removal() {
    let (mut world, _) = pairs(8);
    world.add_entity((D,));
    world
        .add_custom_storage(StorageId::of::<Custom>(), Custom)
        .unwrap();
    let (group, order) = ab_order(&world);
    world.remove::<(B,)>(order[2]);

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (2, 0, 5));
    assert_eq!(plan.storages.len(), 2);
}

/// Asserts that deleting an entity keeps the pairs before it.
#[test]
fn deletion() {
    let (mut world, _) = pairs(8);
    let (group, order) = ab_order(&world);
    world.delete_entity(order[3]);

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (3, 0, 4));
}

/// Asserts that a pair losing B and getting a new one back is counted again in the group.
#[test]
fn reinsert() {
    let (mut world, _) = pairs(8);
    let (group, order) = ab_order(&world);
    world.remove::<(B,)>(order[1]);
    world.add_component(order[1], B(1));

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (1, 0, 7));
}

/// Asserts that with removals in both members, the group keeps the prefix both still trust.
#[test]
fn both_members() {
    let (mut world, _) = pairs(8);
    let (group, order) = ab_order(&world);
    world.remove::<(A,)>(order[4]);
    world.remove::<(B,)>(order[2]);

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (2, 0, 4));
}

/// Asserts that a group attached but never regrouped keeps nothing
/// and receives every pair from the unevaluated regions.
#[test]
fn attached() {
    let mut world = World::new_for_test();
    for i in 0..3 {
        world.add_entity((A(i), B(i)));
    }
    let group = attach::<A, B>(&world);

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (0, 0, 3));
}

/// Asserts that after clearing B and giving it back to some entities,
/// the group keeps nothing and receives only the refilled pairs.
#[test]
fn clear_refill() {
    let (mut world, e) = pairs(6);
    let (group, _) = ab_order(&world);
    world.borrow::<ViewMut<'_, B>>().unwrap().clear();
    for i in [1, 4] {
        world.add_component(e[i], B(i as u32));
    }

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (0, 0, 2));
}

/// Asserts that an entity reusing a deleted grouped entity's index is counted with its own
/// components, and the deleted one isn't.
#[test]
fn generation() {
    let (mut world, _) = pairs(6);
    let (group, order) = ab_order(&world);
    world.delete_entity(order[2]);
    let reused = world.add_entity((A(9), B(9)));
    assert_eq!(reused.index(), order[2].index());

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (2, 0, 4));
}

/// Asserts that an ungrouped A gaining B is counted as promoted in A
/// and joins the group, which keeps all its pairs.
#[test]
fn ungrouped_promotion() {
    let (mut world, _) = pairs(4);
    let lone = world.add_entity((A(4),));
    world.regroup();
    let (group, _) = ab_order(&world);
    world.add_component(lone, B(4));

    let plan = assert_plan(&world, &[&ab()]);

    assert_eq!(counts(&plan, &group), (4, 0, 1));
    assert_eq!(plan.storage(&id::<A>()).promoted, 1);
    assert_eq!(plan.storage(&id::<B>()).promoted, 0);
}

/// Asserts that with AB and AC, a pair gaining C leaves AB's kept prefix, even though both
/// AB ranges are fully trusted, and the prefix keeps its other pairs.
/// The ABC union is created for it.
#[test]
fn gain_leaves_prefix() {
    let builder = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>();
    let (mut world, _) = pairs_in(builder, 6);
    let (group, order) = ab_order(&world);
    world.add_component(order[3], C);

    let plan = assert_plan(&world, &[&ab(), &[id::<A>(), id::<C>()]]);

    assert_eq!(counts(&plan, &group), (6, 1, 0));
    let abc = vec![id::<A>(), id::<B>(), id::<C>()];
    assert_eq!(counts_of(&plan, abc), (0, 0, 1));
}

/// Asserts that with AB, AC and AD, a pair gaining both C and D leaves AB once,
/// though two unevaluated components lead to it.
#[test]
fn gained_twice() {
    let builder = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .with_group::<(A, D)>();
    let (mut world, _) = pairs_in(builder, 6);
    let (group, order) = ab_order(&world);
    world.add_component(order[2], (C, D));

    let declared: [&[StorageId]; 3] = [&ab(), &[id::<A>(), id::<C>()], &[id::<A>(), id::<D>()]];
    let plan = assert_plan(&world, &declared);

    assert_eq!(counts(&plan, &group), (6, 1, 0));
    let abcd = vec![id::<A>(), id::<B>(), id::<C>(), id::<D>()];
    assert_eq!(counts_of(&plan, abcd), (0, 0, 1));
}

/// Asserts that with AB and AC, an ungrouped A gaining both B and C is promoted once.
#[test]
fn promoted_twice() {
    let builder = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>();
    let (mut world, _) = pairs_in(builder, 4);
    let lone = world.add_entity((A(4),));
    world.regroup();
    world.add_component(lone, (B(4), C));

    let plan = assert_plan(&world, &[&ab(), &[id::<A>(), id::<C>()]]);

    assert_eq!(plan.storage(&id::<A>()).promoted, 1);
    let abc = vec![id::<A>(), id::<B>(), id::<C>()];
    assert_eq!(counts_of(&plan, abc), (0, 0, 1));
}

/// Asserts that with AB, CD, BE and CE, an entity grouped in both AB and CD gaining E leaves
/// both kept prefixes, joining them in a single ABCDE union.
#[test]
fn joining() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .with_group::<(B, E)>()
        .with_group::<(C, E)>()
        .build();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C, D)))
        .collect();
    world.regroup();
    world.add_component(e[2], E);

    let declared: [&[StorageId]; 4] = [
        &ab(),
        &[id::<C>(), id::<D>()],
        &[id::<B>(), id::<E>()],
        &[id::<C>(), id::<E>()],
    ];
    let plan = assert_plan(&world, &declared);

    assert_eq!(counts_of(&plan, ab().to_vec()), (4, 1, 0));
    assert_eq!(counts_of(&plan, vec![id::<C>(), id::<D>()]), (4, 1, 0));
    let all = vec![id::<A>(), id::<B>(), id::<C>(), id::<D>(), id::<E>()];
    assert_eq!(counts_of(&plan, all), (0, 0, 1));
}

/// Asserts that a new entity owning the components of two disjoint groups doesn't create
/// a union of them.
#[test]
fn disjoint() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    world.add_entity((A(0), B(0), C, D));
    world.regroup();
    world.add_entity((A(1), B(1), C, D));

    let plan = assert_plan(&world, &[&ab(), &[id::<C>(), id::<D>()]]);

    assert_eq!(plan.groups.len(), 2);
}

/// Asserts that with AB, BC and CD, an entity of the ABCD union losing D needs an ABC union,
/// created while counting the damaged range, and every other entity stays in ABCD.
#[test]
fn demotion_union() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, C)>()
        .with_group::<(C, D)>()
        .build();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C, D)))
        .collect();
    world.regroup();
    world.remove::<(D,)>(e[1]);

    let declared: [&[StorageId]; 3] = [&ab(), &[id::<B>(), id::<C>()], &[id::<C>(), id::<D>()]];
    let plan = assert_plan(&world, &declared);

    let abc = vec![id::<A>(), id::<B>(), id::<C>()];
    assert_eq!(counts_of(&plan, abc), (0, 0, 1));
    let all = vec![id::<A>(), id::<B>(), id::<C>(), id::<D>()];
    let (kept, leaving, incoming) = counts_of(&plan, all);
    assert_eq!(kept - leaving + incoming, 3);
}

/// Asserts that with AB, AC and their ABC union rotated in A, pairs gaining C, ABC entities
/// losing C or B, and an AC pair gaining B are all accounted for.
#[test]
fn rotated() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let e: Vec<EntityId> = (0..24)
        .map(|i| match i % 3 {
            0 => world.add_entity((A(i), B(i))),
            1 => world.add_entity((A(i), C)),
            _ => world.add_entity((A(i), B(i), C)),
        })
        .collect();
    world.regroup();
    // Promotions shrink AB and AC in A, which rotates the ABC range behind them.
    world.add_component(e[0], C);
    world.add_component(e[4], B(4));
    world.regroup();
    world.add_component(e[3], C);
    world.add_component(e[9], C);
    world.remove::<(C,)>(e[5]);
    world.remove::<(B,)>(e[11]);
    world.add_component(e[7], B(7));

    assert_plan(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that a missing union is created once, as a single shared definition attached
/// as an empty range after the last range of every member, and reused by the next analysis.
#[test]
fn union_created_once() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C));
    let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);
    let ac = attach_ids(&world, &[id::<A>(), id::<C>()]);

    analyze_world(&world);

    let a = groups_of::<A>(&world);
    assert_eq!(a.len(), 3);
    assert!(Arc::ptr_eq(&a[0], &ab) && Arc::ptr_eq(&a[1], &ac));
    let abc = a[2].clone();
    assert_eq!(*abc, *sorted_ids(vec![id::<A>(), id::<B>(), id::<C>()]));
    assert!(Arc::ptr_eq(&groups_of::<B>(&world)[1], &abc));
    assert!(Arc::ptr_eq(&groups_of::<C>(&world)[1], &abc));
    assert_eq!(group_of::<A>(&world, &abc), (vec![], 0));

    world.add_entity((A(1), B(1), C));
    analyze_world(&world);

    assert_eq!(groups_of::<A>(&world).len(), 3);
}

/// Asserts that a group satisfied alone is its own union, nothing new is attached.
#[test]
fn union_existing_group() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0)));
    attach_ids(&world, &[id::<A>(), id::<B>()]);

    analyze_world(&world);

    assert_eq!(groups_of::<A>(&world).len(), 1);
}

/// Asserts that a declared group equal to the union is reused instead of creating a new one.
#[test]
fn union_declared() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);
    attach_ids(&world, &[id::<A>(), id::<B>(), id::<C>()]);

    analyze_world(&world);

    assert_eq!(groups_of::<A>(&world).len(), 3);
}

/// Asserts that a transitive union is created directly, without any intermediate union:
/// with AB, BC and CD, only ABCD is added.
#[test]
fn union_direct() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<B>(), id::<C>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);

    analyze_world(&world);

    assert_eq!(groups_of::<A>(&world).len(), 2);
    assert_eq!(groups_of::<B>(&world).len(), 3);
    assert_eq!(groups_of::<C>(&world).len(), 3);
    assert_eq!(groups_of::<D>(&world).len(), 2);
    assert_eq!(groups_of::<D>(&world)[1].len(), 4);
}
