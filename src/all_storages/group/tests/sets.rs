use super::*;
use crate::all_storages::group::ConnectedSet;

/// Returns the connected set holding `T`'s storage, `None` when it isn't grouped.
fn set_of<T: Component + Send + Sync>(world: &mut World) -> Option<ConnectedSet> {
    world
        .all_storages
        .get_mut()
        .grouping
        .set_of(id::<T>())
        .cloned()
}

/// Returns the members of every connected set of `world`, in creation order.
fn sets(world: &mut World) -> Vec<Vec<StorageId>> {
    world
        .all_storages
        .get_mut()
        .grouping
        .sets
        .iter()
        .map(|set| set.members.clone())
        .collect()
}

/// Asserts that `AB`, `AC` and `CD` make a single `ABCD` set, reachable from every member,
/// so `A`'s set holds `D` even though no group lists them together.
#[test]
fn transitive() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .with_group::<(C, D)>()
        .build();

    let abcd = group_ids(&[id::<A>(), id::<B>(), id::<C>(), id::<D>()]);
    assert_eq!(sets(&mut world), [abcd.clone()]);
    assert_eq!(set_of::<A>(&mut world).unwrap().members, abcd);
    assert_eq!(set_of::<D>(&mut world).unwrap().members, abcd);
}

/// Asserts that a group linking two existing sets merges them into one.
#[test]
fn bridge() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .with_group::<(B, C)>()
        .build();

    assert_eq!(
        sets(&mut world),
        [group_ids(&[id::<A>(), id::<B>(), id::<C>(), id::<D>()])]
    );
}

/// Asserts that groups without a shared member make separate sets.
#[test]
fn disjoint() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();

    assert_eq!(
        sets(&mut world),
        [
            group_ids(&[id::<A>(), id::<B>()]),
            group_ids(&[id::<C>(), id::<D>()])
        ]
    );
}

/// Asserts that an entity owning the components of two disjoint groups doesn't link them,
/// regroup keeps it in both groups and the sets stay separate.
#[test]
fn disjoint_shared_entity() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    world.add_entity((A(0), B(0), C, D));

    world.regroup();

    assert_eq!(groups_of::<A>(&world).len(), 1);
    assert_eq!(groups_of::<C>(&world).len(), 1);
    assert_eq!(sets(&mut world).len(), 2);
}

/// Asserts that declaring the same group several times, in any order, makes a single set.
#[test]
fn repeated() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, A)>()
        .with_group::<(A, B)>()
        .build();

    assert_eq!(sets(&mut world), [group_ids(&[id::<A>(), id::<B>()])]);
}

/// Asserts that a union created by regroup is attached to the storages
/// without changing the declared groups or the sets.
#[test]
fn unions() {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    world.add_entity((A(0), B(0), C));

    world.regroup();

    let abc = group_ids(&[id::<A>(), id::<B>(), id::<C>()]);
    assert!(groups_of::<A>(&world).contains(&abc));
    assert_eq!(world.all_storages.get_mut().grouping.groups.len(), 2);
    assert_eq!(sets(&mut world), [abc]);
}

/// Asserts that a storage outside every declared group isn't in any set.
#[test]
fn undeclared() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    world.add_entity(C);

    assert!(set_of::<C>(&mut world).is_none());
}

/// Asserts that a set of `Send + Sync` members can be regrouped from any thread.
#[test]
fn thread_safe() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();

    assert!(set_of::<A>(&mut world).unwrap().thread_safe);
}
