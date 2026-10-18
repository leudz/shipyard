use super::*;

/// Returns the storages `entity` has to be grouped in, starting from its component in `storage_id`.
fn discover(world: &World, storage_id: StorageId, entity: EntityId) -> Option<Vec<StorageId>> {
    let mut all_storages = world.all_storages_mut().unwrap();
    let storages = all_storages.storages.get_mut();

    Discovery::default()
        .discover(storages, storage_id, entity)
        .map(<[StorageId]>::to_vec)
}

/// Returns the group made of `members`, creating it when it doesn't exist yet.
fn union(world: &World, members: &[StorageId]) -> Arc<[StorageId]> {
    let mut all_storages = world.all_storages_mut().unwrap();
    let storages = all_storages.storages.get_mut();
    let mut members = members.to_vec();
    members.sort_unstable();

    union_group(storages, &members)
}

/// Asserts that with AB and AC, an entity holding A, B and C has to be grouped in all three,
/// from any of them.
#[test]
fn discover_overlapping() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);
    let abc = sorted_ids(vec![id::<A>(), id::<B>(), id::<C>()]);

    assert_eq!(discover(&world, id::<A>(), entity), Some(abc.clone()));
    assert_eq!(discover(&world, id::<C>(), entity), Some(abc));
}

/// Asserts that with AB and AC, an entity holding only A and B has to be grouped in A and B.
#[test]
fn discover_one_satisfied() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C));
    let entity = world.add_entity((A(1), B(1)));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);

    assert_eq!(
        discover(&world, id::<A>(), entity),
        Some(sorted_ids(vec![id::<A>(), id::<B>()]))
    );
}

/// Asserts that an entity satisfying none of a storage's groups has nowhere to be grouped.
#[test]
fn discover_none() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C));
    let entity = world.add_entity(A(1));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);

    assert_eq!(discover(&world, id::<A>(), entity), None);
}

/// Asserts that with AB, BC and CD, an entity holding all four is grouped in all of them,
/// from either end of the chain.
#[test]
fn discover_chain() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<B>(), id::<C>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);
    let abcd = sorted_ids(vec![id::<A>(), id::<B>(), id::<C>(), id::<D>()]);

    assert_eq!(discover(&world, id::<A>(), entity), Some(abcd.clone()));
    assert_eq!(discover(&world, id::<D>(), entity), Some(abcd));
}

/// Asserts that groups an entity doesn't satisfy don't connect anything:
/// with AB, BE, EC and CD and no E, AB and CD stay apart.
#[test]
fn discover_unsatisfied_link() {
    let mut world = World::new_for_test();
    world.add_entity((B(0), C, E));
    let entity = world.add_entity((A(0), B(0), C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<B>(), id::<E>()]);
    attach_ids(&world, &[id::<E>(), id::<C>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);

    assert_eq!(
        discover(&world, id::<A>(), entity),
        Some(sorted_ids(vec![id::<A>(), id::<B>()]))
    );
    assert_eq!(
        discover(&world, id::<D>(), entity),
        Some(sorted_ids(vec![id::<C>(), id::<D>()]))
    );
}

/// Asserts that owning the components of two disjoint groups doesn't merge them.
#[test]
fn discover_disjoint() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);

    assert_eq!(
        discover(&world, id::<A>(), entity),
        Some(sorted_ids(vec![id::<A>(), id::<B>()]))
    );
    assert_eq!(
        discover(&world, id::<C>(), entity),
        Some(sorted_ids(vec![id::<C>(), id::<D>()]))
    );
}

/// Asserts that with AB and ABC, the larger group adds C only to entities holding C.
#[test]
fn discover_subset() {
    let mut world = World::new_for_test();
    let abc_entity = world.add_entity((A(0), B(0), C));
    let ab_entity = world.add_entity((A(1), B(1)));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<B>(), id::<C>()]);

    assert_eq!(
        discover(&world, id::<A>(), abc_entity),
        Some(sorted_ids(vec![id::<A>(), id::<B>(), id::<C>()]))
    );
    assert_eq!(
        discover(&world, id::<A>(), ab_entity),
        Some(sorted_ids(vec![id::<A>(), id::<B>()]))
    );
}

/// Asserts that a component left by a deleted entity doesn't satisfy a group
/// for the entity now using its index.
#[test]
fn discover_stale_generation() {
    use crate::AddComponent;

    let mut world = World::new_for_test();
    let deleted = world.add_entity((A(0), B(0)));
    world.delete_entity(deleted);
    let entity = world.add_entity(A(1));
    assert_eq!(entity.index(), deleted.index());
    world
        .borrow::<ViewMut<'_, B>>()
        .unwrap()
        .add_component_unchecked(deleted, B(0));
    attach_ids(&world, &[id::<A>(), id::<B>()]);

    assert_eq!(discover(&world, id::<A>(), entity), None);
}

/// Asserts that a missing union is created once, as a single shared definition attached
/// as an empty range after the last range of every member, and then reused.
#[test]
fn union_created_once() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C));
    let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);
    let ac = attach_ids(&world, &[id::<A>(), id::<C>()]);

    let abc = union(&world, &[id::<A>(), id::<B>(), id::<C>()]);
    let again = union(&world, &[id::<C>(), id::<A>(), id::<B>()]);

    assert!(Arc::ptr_eq(&abc, &again));
    let a = groups_of::<A>(&world);
    assert!(Arc::ptr_eq(&a[0], &ab) && Arc::ptr_eq(&a[1], &ac) && Arc::ptr_eq(&a[2], &abc));
    assert_eq!(a.len(), 3);
    assert!(
        groups_of::<B>(&world)
            .iter()
            .any(|group| Arc::ptr_eq(group, &abc))
    );
    assert!(
        groups_of::<C>(&world)
            .iter()
            .any(|group| Arc::ptr_eq(group, &abc))
    );
    let view = world.borrow::<View<'_, A>>().unwrap();
    assert_eq!(
        (view.groups.ranges[2].len, view.groups.ranges[2].trust),
        (0, 0)
    );
}

/// Asserts that a group satisfied alone is its own union, nothing new is attached.
#[test]
fn union_existing_group() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0)));
    let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);

    assert!(Arc::ptr_eq(&union(&world, &[id::<A>(), id::<B>()]), &ab));
    assert_eq!(groups_of::<A>(&world).len(), 1);
}

/// Asserts that a declared group equal to the union is reused instead of creating a new one.
#[test]
fn union_declared() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<A>(), id::<C>()]);
    let abc = attach_ids(&world, &[id::<A>(), id::<B>(), id::<C>()]);

    assert!(Arc::ptr_eq(
        &union(&world, &[id::<A>(), id::<B>(), id::<C>()]),
        &abc
    ));
    assert_eq!(groups_of::<A>(&world).len(), 3);
}

/// Asserts that a transitive union is created directly, without any intermediate union:
/// with AB, BC and CD, only ABCD is added.
#[test]
fn union_direct() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C, D));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<B>(), id::<C>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);

    let members = discover(&world, id::<A>(), entity).unwrap();
    let abcd = union(&world, &members);

    assert_eq!(abcd.len(), 4);
    assert_eq!(groups_of::<A>(&world).len(), 2);
    assert_eq!(groups_of::<B>(&world).len(), 3);
    assert_eq!(groups_of::<C>(&world).len(), 3);
    assert_eq!(groups_of::<D>(&world).len(), 2);
}
