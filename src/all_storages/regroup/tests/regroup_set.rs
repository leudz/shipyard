use super::*;
use crate::error;
use crate::views::AllStoragesView;
use core::any::type_name;

/// Asserts that `World::regroup_set` regroups the named set and leaves the other dirty set
/// exactly as it was.
#[test]
fn only_set() {
    let world = two_sets();
    let c = snapshot::<C>(&world);
    let d = snapshot::<D>(&world);

    world.regroup_set::<A>().unwrap();

    assert!(unevaluated_of::<A>(&world).is_empty());
    assert!(unevaluated_of::<B>(&world).is_empty());
    assert_eq!(group_of::<A>(&world, &groups_of::<A>(&world)[0]).0.len(), 3);
    assert_eq!(snapshot::<C>(&world), c);
    assert_eq!(snapshot::<D>(&world), d);
}

/// Asserts that naming a member reached through another group regroups the whole connected set.
#[test]
fn any_member() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, C)>()
        .build();
    for i in 0..3 {
        world.add_entity((A(i), B(i), C));
    }

    world.regroup_set::<C>().unwrap();

    assert!(unevaluated_of::<A>(&world).is_empty());
    assert_regrouped(&world);
}

/// Asserts that a component outside every group returns `NotGrouped` with its storage's name
/// and id.
#[test]
fn not_grouped() {
    let mut world = two_sets();
    world.add_entity(E);

    let result = world.regroup_set::<E>();

    assert!(matches!(
        result,
        Err(error::RegroupSet::NotGrouped { name, id: failed })
            if name == type_name::<SparseSet<E>>() && failed == id::<E>()
    ));
}

/// Asserts that a borrowed member the caller didn't name fails the regroup with that member's
/// borrow error, naming its storage, and nothing moves.
#[test]
fn unnamed_member_borrowed() {
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

    let result = world.regroup_set::<A>();

    assert!(matches!(
        result,
        Err(error::RegroupSet::Storage(error::GetStorage::StorageBorrow {
            name: Some(name),
            id: failed,
            borrow: error::Borrow::Unique,
        })) if name == type_name::<SparseSet<C>>() && failed == id::<C>()
    ));
    drop(held);
    assert_eq!(snapshot::<A>(&world), a);
    assert_eq!(snapshot::<B>(&world), b);
}

/// Asserts that a storage outside the set can stay borrowed, exclusively, during `regroup_set`.
#[test]
fn unrelated_borrow() {
    let world = two_sets();
    let c = world.borrow::<ViewMut<'_, C>>().unwrap();

    world.regroup_set::<A>().unwrap();

    drop(c);
    assert!(unevaluated_of::<A>(&world).is_empty());
}

/// Asserts that `World::regroup_set` returns an `AllStorages` borrow error while `AllStorages`
/// is borrowed mutably.
#[test]
fn all_storages_borrowed() {
    let world = two_sets();
    let held = world.all_storages_mut().unwrap();

    let result = world.regroup_set::<A>();

    assert!(matches!(
        result,
        Err(error::RegroupSet::Storage(
            error::GetStorage::AllStoragesBorrow(error::Borrow::Shared)
        ))
    ));
    drop(held);
}

/// Asserts that `AllStorages::regroup_set` works from a system holding `AllStorages` shared.
#[test]
fn system() {
    let world = two_sets();

    world
        .run(|all_storages: AllStoragesView<'_>| all_storages.regroup_set::<B>())
        .unwrap();

    assert!(unevaluated_of::<A>(&world).is_empty());
    assert_eq!(unevaluated_of::<C>(&world).len(), 3);
}

/// Asserts that a set can be named through a thread-local wrapper member.
#[cfg(feature = "thread_local")]
#[test]
fn wrapper() {
    use crate::AddComponent;
    use crate::borrow::NonSend;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let entity = world.add_entity(A(0));
    world
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B(0));

    world.regroup_set::<NonSend<B>>().unwrap();

    assert!(unevaluated_of::<A>(&world).is_empty());
    let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
    assert_eq!(b.groups.unevaluated_start, b.len());
}
