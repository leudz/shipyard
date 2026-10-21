use super::*;

/// Clones `source`'s storages with a registered clone function into `destination`.
fn clone_into(source: &World, destination: &World) {
    source
        .all_storages()
        .unwrap()
        .clone_storages_to(&mut destination.all_storages_mut().unwrap());
}

/// Returns a world without groups with one entity per value owning `B(value)`, B's clone
/// registered.
fn source_of_b(values: &[u32]) -> World {
    let mut world = World::builder_for_test().build();
    for &value in values {
        world.add_entity(B(value));
    }
    world.register_clone::<SparseSet<B>>();

    world
}

/// Returns `(entity, a, b)` for every entity owning A and B, sorted, iterating them together.
fn ab_contents(world: &World) -> Vec<(EntityId, u32, u32)> {
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut contents = Vec::new();
    (&a, &b)
        .iter()
        .with_id()
        .for_each(|(entity, (a, b))| contents.push((entity, a.0, b.0)));
    contents.sort_unstable();

    contents
}

/// Returns `true` when both lists hold the same groups, by pointer, in the same order.
fn same_groups(left: &[Arc<[StorageId]>], right: &[Arc<[StorageId]>]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| Arc::ptr_eq(left, right))
}

/// Asserts that a replaced member keeps the destination's group definitions, by pointer,
/// with every copied component unevaluated.
#[test]
fn definitions() {
    let (destination, entities) = pairs(3);
    let groups = groups_of::<B>(&destination);

    clone_into(&source_of_b(&[10, 11, 12]), &destination);

    assert!(same_groups(&groups_of::<B>(&destination), &groups));
    assert_eq!(unevaluated_of::<B>(&destination), sorted(entities));
    let b = destination.borrow::<View<'_, B>>().unwrap();
    assert!(b.groups.ranges.iter().all(|range| range.len == 0));
}

/// Asserts that a member without a registered clone function stays exactly as it was.
#[test]
fn unregistered() {
    let (destination, _) = pairs(3);
    let a = snapshot::<A>(&destination);

    clone_into(&source_of_b(&[10, 11, 12]), &destination);

    assert_eq!(snapshot::<A>(&destination), a);
}

/// Asserts that iterating a group right after one of its members is replaced pairs every entity
/// with its copied component, before any regroup.
#[test]
fn iteration() {
    let (destination, entities) = pairs(3);

    clone_into(&source_of_b(&[10, 11, 12]), &destination);

    let expected: Vec<_> = (0..)
        .zip(&entities)
        .map(|(i, &entity)| (entity, i, 10 + i))
        .collect();
    assert_eq!(ab_contents(&destination), expected);
}

/// Asserts that regrouping after a replacement aligns the group again, keeping the copied values.
#[test]
fn regroup() {
    let (destination, entities) = pairs(3);
    let group = groups_of::<A>(&destination).remove(0);

    clone_into(&source_of_b(&[10, 11, 12]), &destination);
    destination.regroup_set::<A>().unwrap();

    assert_regrouped(&destination);
    assert_eq!(group_of::<A>(&destination, &group).0.len(), 3);
    let expected: Vec<_> = (0..)
        .zip(&entities)
        .map(|(i, &entity)| (entity, i, 10 + i))
        .collect();
    assert_eq!(ab_contents(&destination), expected);
}

/// Asserts that replacing every member of a group keeps both definitions, iterates correctly
/// and regroups.
#[test]
fn several() {
    let (destination, entities) = pairs(3);
    let a_groups = groups_of::<A>(&destination);
    let b_groups = groups_of::<B>(&destination);
    let mut source = World::builder_for_test().build();
    for i in 0..3 {
        source.add_entity((A(20 + i), B(10 + i)));
    }
    source.register_clone::<SparseSet<A>>();
    source.register_clone::<SparseSet<B>>();

    clone_into(&source, &destination);

    assert!(same_groups(&groups_of::<A>(&destination), &a_groups));
    assert!(same_groups(&groups_of::<B>(&destination), &b_groups));
    assert_eq!(unevaluated_of::<A>(&destination).len(), 3);
    let expected: Vec<_> = (0..)
        .zip(&entities)
        .map(|(i, &entity)| (entity, 20 + i, 10 + i))
        .collect();
    assert_eq!(ab_contents(&destination), expected);
    destination.regroup_set::<B>().unwrap();
    assert_regrouped(&destination);
    assert_eq!(ab_contents(&destination), expected);
}

/// Asserts that replacing a member with an empty storage keeps its definitions, leaves nothing
/// to iterate and regroups the remaining members out of the group.
#[test]
fn empty() {
    let (destination, _) = pairs(3);
    let groups = groups_of::<B>(&destination);

    clone_into(&source_of_b(&[]), &destination);

    assert!(same_groups(&groups_of::<B>(&destination), &groups));
    assert!(ab_contents(&destination).is_empty());
    destination.regroup_set::<A>().unwrap();
    assert_regrouped(&destination);
    assert_eq!(ungrouped_of::<A>(&destination).len(), 3);
}

/// Asserts that a replaced member keeps the unions the destination attached, and regrouping
/// puts every entity where its components say.
#[test]
fn unions() {
    let mut destination = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    destination.add_entity((A(0), B(0), C));
    destination.add_entity((A(1), B(1)));
    destination.add_entity((A(2), C));
    destination.regroup();
    let groups = groups_of::<B>(&destination);
    // B is in AB and in the ABC union.
    assert_eq!(groups.len(), 2);
    let mut source = source_of_b(&[10, 11]);
    source.add_entity(());

    clone_into(&source, &destination);

    assert!(same_groups(&groups_of::<B>(&destination), &groups));
    destination.regroup_set::<A>().unwrap();
    assert_regrouped(&destination);
    assert_memberships(&destination, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that the source's groups never reach the destination: a replaced member only gets
/// the destination's definitions, and the destination's connected sets don't change.
#[test]
fn source_groups() {
    let (destination, _) = pairs(3);
    let groups = groups_of::<B>(&destination);
    let set = destination
        .all_storages()
        .unwrap()
        .grouping
        .set_of(id::<A>())
        .cloned();
    let mut source = World::builder_for_test().with_group::<(B, C)>().build();
    for i in 0..3 {
        source.add_entity((B(10 + i), C));
    }
    source.regroup();
    source.register_clone::<SparseSet<B>>();
    source.register_clone::<SparseSet<C>>();

    clone_into(&source, &destination);

    assert!(same_groups(&groups_of::<B>(&destination), &groups));
    let all_storages = destination.all_storages().unwrap();
    assert_eq!(all_storages.grouping.set_of(id::<A>()).cloned(), set);
    assert!(all_storages.grouping.set_of(id::<C>()).is_none());
    drop(all_storages);
    assert!(groups_of::<C>(&destination).is_empty());
}

/// Asserts that a `NonSend` member keeps its definitions when replaced, and regroups.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::AddComponent;
    use crate::borrow::NonSend;

    let mut destination = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let entity = destination.add_entity(A(0));
    destination
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B(0));
    destination.regroup();
    let groups = groups_of::<A>(&destination);
    let mut source = World::builder_for_test().build();
    let copied = source.add_entity(());
    source
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(copied, B(10));
    source.register_clone::<NonSend<SparseSet<B>>>();

    clone_into(&source, &destination);

    {
        let b = destination.borrow::<NonSend<View<'_, B>>>().unwrap();
        let b_groups: Vec<_> = b
            .groups
            .ranges
            .iter()
            .map(|range| range.group.clone())
            .collect();
        assert!(same_groups(&b_groups, &groups));
        assert_eq!(b.groups.unevaluated_start, 0);
    }
    destination.regroup_set::<A>().unwrap();
    let b = destination.borrow::<NonSend<View<'_, B>>>().unwrap();
    assert_eq!(b.groups.unevaluated_start, b.len());
    assert_eq!(b.groups.ranges[0].len, 1);
}
