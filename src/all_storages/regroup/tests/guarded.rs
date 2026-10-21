use super::*;
use crate::all_storages::ConnectedSet;
use crate::error;

/// Returns the connected set holding `T`'s storage.
fn set_of<T: Component + Send + Sync>(world: &World) -> ConnectedSet {
    world
        .all_storages()
        .unwrap()
        .grouping
        .set_of(id::<T>())
        .expect("the storage is grouped")
        .clone()
}

/// Regroups the connected set holding `T`'s storage, borrowing only that set.
fn regroup_set_of<T: Component + Send + Sync>(world: &World) -> Result<(), error::GetStorage> {
    let set = set_of::<T>(world);

    world.all_storages().unwrap().regroup_connected_set(&set)
}

/// Asserts that regrouping AB's set groups its entities and leaves CD's dirty storages exactly
/// as they were.
#[test]
fn one_set() {
    let world = two_sets();
    let c = snapshot::<C>(&world);
    let d = snapshot::<D>(&world);

    regroup_set_of::<A>(&world).unwrap();

    assert!(unevaluated_of::<A>(&world).is_empty());
    assert!(unevaluated_of::<B>(&world).is_empty());
    assert_eq!(group_of::<A>(&world, &groups_of::<A>(&world)[0]).0.len(), 3);
    assert_eq!(snapshot::<C>(&world), c);
    assert_eq!(snapshot::<D>(&world), d);
    assert_eq!(unevaluated_of::<C>(&world).len(), 3);
}

/// Asserts that a storage outside the set can stay borrowed, exclusively, while the set
/// regroups.
#[test]
fn unrelated_borrow() {
    let world = two_sets();
    let c = world.borrow::<ViewMut<'_, C>>().unwrap();

    regroup_set_of::<A>(&world).unwrap();

    drop(c);
    assert!(unevaluated_of::<A>(&world).is_empty());
}

/// Asserts that a borrow of a member the regroup doesn't start from fails it with that member's
/// borrow error, before anything moves, and releases the members it borrowed already.
#[test]
fn conflicting_borrow() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, C)>()
        .build();
    for i in 0..3 {
        world.add_entity((A(i), B(i), C));
    }
    let a = snapshot::<A>(&world);
    let b = snapshot::<B>(&world);
    let held = world.borrow::<View<'_, C>>().unwrap();

    let result = regroup_set_of::<A>(&world);

    assert!(matches!(
        result,
        Err(error::GetStorage::StorageBorrow {
            id: failed,
            borrow: error::Borrow::Unique,
            ..
        }) if failed == id::<C>()
    ));
    drop(held);
    assert_eq!(snapshot::<A>(&world), a);
    assert_eq!(snapshot::<B>(&world), b);
    // A and B were borrowed before C failed, they're free again.
    drop(world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap());
    regroup_set_of::<A>(&world).unwrap();
    assert!(unevaluated_of::<C>(&world).is_empty());
}

/// Asserts that regrouping a set with overlapping groups attaches the union its entities need,
/// every entity ending where its components say.
#[test]
fn union() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    world.add_entity((A(0), B(0), C));
    world.add_entity((A(1), B(1)));
    world.add_entity((A(2), C));

    regroup_set_of::<B>(&world).unwrap();

    assert_eq!(groups_of::<A>(&world).len(), 3);
    assert_regrouped(&world);
    assert_memberships(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that regrouping a clean set changes nothing.
#[test]
fn clean() {
    let world = two_sets();
    regroup_set_of::<A>(&world).unwrap();
    let a = snapshot::<A>(&world);
    let b = snapshot::<B>(&world);

    regroup_set_of::<A>(&world).unwrap();

    assert_eq!(snapshot::<A>(&world), a);
    assert_eq!(snapshot::<B>(&world), b);
}

/// Asserts that a clean set is still borrowed: a held borrow of a member fails the regroup.
#[test]
fn clean_still_borrows() {
    let world = two_sets();
    regroup_set_of::<A>(&world).unwrap();
    let held = world.borrow::<View<'_, B>>().unwrap();

    let result = regroup_set_of::<A>(&world);

    assert!(matches!(
        result,
        Err(error::GetStorage::StorageBorrow { id: failed, .. }) if failed == id::<B>()
    ));
    drop(held);
}

/// Asserts that a set with a `NonSend` member regroups from the world's thread.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::AddComponent;
    use crate::borrow::NonSend;
    use crate::sparse_set::SparseSet;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let entity = world.add_entity(A(0));
    world
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B(0));

    regroup_set_of::<A>(&world).unwrap();

    assert!(unevaluated_of::<A>(&world).is_empty());
    let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
    assert_eq!(b.groups.unevaluated_start, b.len());
    assert!(
        set_of::<A>(&world)
            .members
            .contains(&StorageId::of::<NonSend<SparseSet<B>>>())
    );
}

/// Asserts that regrouping a set with a `NonSend` member from another thread fails with a
/// wrong-thread error naming that member, `A` being borrowed and released on the way.
#[cfg(all(feature = "thread_local", feature = "std"))]
#[test]
fn non_send_wrong_thread() {
    use crate::borrow::NonSend;
    use crate::sparse_set::SparseSet;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    world.add_entity(A(0));
    let set = set_of::<A>(&world);

    let result = std::thread::scope(|scope| {
        scope
            .spawn(|| world.all_storages().unwrap().regroup_connected_set(&set))
            .join()
            .unwrap()
    });

    assert!(matches!(
        result,
        Err(error::GetStorage::StorageBorrow {
            name: Some(name),
            id: failed,
            borrow: error::Borrow::WrongThread,
        }) if name == core::any::type_name::<NonSend<SparseSet<B>>>()
            && failed == StorageId::of::<NonSend<SparseSet<B>>>()
    ));
    drop(world.borrow::<ViewMut<'_, A>>().unwrap());
}
