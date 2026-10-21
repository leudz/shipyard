use super::*;

/// Returns the entities in `T`'s range, in order.
fn range_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
    let view = world.borrow::<View<'_, T>>().unwrap();

    view.dense[..view.groups.grouped_end()].to_vec()
}

/// Asserts that the first regroup places the entities holding both components in both ranges,
/// and the entities holding only one in that storage's ungrouped region.
///
/// Before it, every component added to the grouped storages is unevaluated.
#[test]
fn first_regroup() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let ab0 = world.add_entity((A(0), B(0)));
    let a1 = world.add_entity(A(1));
    let b2 = world.add_entity(B(2));
    let ab3 = world.add_entity((A(3), B(3)));
    assert_eq!(unevaluated_of::<A>(&world), [ab0, a1, ab3]);
    assert!(ungrouped_of::<A>(&world).is_empty());

    world.regroup();

    assert_regrouped(&world);
    assert_eq!(sorted(range_of::<A>(&world)), [ab0, ab3]);
    assert_eq!(sorted(range_of::<B>(&world)), [ab0, ab3]);
    assert_eq!(ungrouped_of::<A>(&world), [a1]);
    assert_eq!(ungrouped_of::<B>(&world), [b2]);
    assert!(unevaluated_of::<A>(&world).is_empty());
    assert!(unevaluated_of::<B>(&world).is_empty());
}

/// Asserts that every component keeps its value after the first regroup moves it.
#[test]
fn first_regroup_values() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let ab0 = world.add_entity((A(0), B(0)));
    let a1 = world.add_entity(A(1));
    let b2 = world.add_entity(B(2));
    let ab3 = world.add_entity((A(3), B(3)));

    world.regroup();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    assert_eq!(a.get(ab0), Ok(&A(0)));
    assert_eq!(a.get(a1), Ok(&A(1)));
    assert_eq!(a.get(ab3), Ok(&A(3)));
    assert_eq!(b.get(ab0), Ok(&B(0)));
    assert_eq!(b.get(b2), Ok(&B(2)));
    assert_eq!(b.get(ab3), Ok(&B(3)));
}

/// Asserts that new entities land in both unevaluated regions,
/// and that regroup appends them to both ranges without moving the previous range contents.
#[test]
fn new_components() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    world.add_entity((A(0), B(0)));
    world.add_entity(A(1));
    world.add_entity((A(2), B(2)));
    world.regroup();
    let range_before = range_of::<A>(&world);

    let ab3 = world.add_entity((A(3), B(3)));
    let ab4 = world.add_entity((A(4), B(4)));

    assert_eq!(unevaluated_of::<A>(&world), [ab3, ab4]);
    assert_eq!(unevaluated_of::<B>(&world), [ab3, ab4]);

    world.regroup();

    assert_regrouped(&world);
    let range_after = range_of::<A>(&world);
    assert_eq!(range_after[..2], range_before);
    assert_eq!(sorted(range_after[2..].to_vec()), [ab3, ab4]);
}

/// Asserts that adding B to an entity whose A sits in A's ungrouped region puts B in B's unevaluated region,
/// and that regroup then moves the entity into both ranges.
#[test]
fn late_component() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let ab0 = world.add_entity((A(0), B(0)));
    let a1 = world.add_entity(A(1));
    world.regroup();

    world.add_component(a1, B(1));

    assert_eq!(ungrouped_of::<A>(&world), [a1]);
    assert_eq!(unevaluated_of::<B>(&world), [a1]);

    world.regroup();

    assert_regrouped(&world);
    assert_eq!(sorted(range_of::<A>(&world)), [ab0, a1]);
    assert!(ungrouped_of::<A>(&world).is_empty());
}

/// Asserts that removing a grouped A leaves the rest of A's range in place, only trusting the
/// positions before the removed one, while B keeps its range.
/// Regroup then leaves the entity that lost A ungrouped in B and the others grouped.
#[test]
fn removal() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let ab0 = world.add_entity((A(0), B(0)));
    let ab1 = world.add_entity((A(1), B(1)));
    let ab2 = world.add_entity((A(2), B(2)));
    world.regroup();

    world.remove::<(A,)>(ab1);

    assert_eq!(range_of::<A>(&world), [ab0, ab2]);
    assert_eq!(
        world.borrow::<View<'_, A>>().unwrap().groups.ranges[0].trust,
        1
    );
    assert!(unevaluated_of::<A>(&world).is_empty());
    assert_eq!(range_of::<B>(&world).len(), 3);

    world.regroup();

    assert_regrouped(&world);
    assert_eq!(sorted(range_of::<B>(&world)), [ab0, ab2]);
    assert_eq!(ungrouped_of::<B>(&world), [ab1]);
}

/// Asserts that regroup moves insertion and modification timestamps with their components.
#[test]
fn tracking() {
    let mut world = World::builder_for_test()
        .with_group::<(TrackedA, TrackedB)>()
        .build();
    // Holding only A, it sits before the others in A and forces them to move.
    world.add_entity(TrackedA(0));
    let old = world.add_entity((TrackedA(1), TrackedB));
    world.run(|a: ViewMut<'_, TrackedA, track::All>| a.clear_all_inserted());
    let new = world.add_entity((TrackedA(2), TrackedB));
    world.run(|mut a: ViewMut<'_, TrackedA, track::All>| {
        (&mut a).get(old).unwrap().0 = 10;
    });

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
    let after = state(&world);

    assert_eq!(before, [false, true, true, false]);
    assert_eq!(after, before);
}

/// Builds a world with a group, then adds, removes and regroups components.
fn build_world() -> World {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let mut entities = Vec::new();
    for index in 0..30 {
        let entity = match index % 3 {
            0 => world.add_entity((A(index), B(index))),
            1 => world.add_entity((A(index), C)),
            _ => world.add_entity((B(index), C)),
        };
        entities.push(entity);
    }

    world.regroup();

    for index in 30..40 {
        world.add_entity((A(index), B(index)));
    }
    world.remove::<(A,)>(entities[3]);
    world.add_component(entities[1], B(1));
    world.regroup();

    world
}

/// Asserts that two worlds built with identical operations end up with identical layouts,
/// even though their storage maps may iterate in different orders.
#[test]
fn determinism() {
    let world0 = build_world();
    let world1 = build_world();

    assert_regrouped(&world0);
    assert_eq!(dense_of::<A>(&world0), dense_of::<A>(&world1));
    assert_eq!(dense_of::<B>(&world0), dense_of::<B>(&world1));
    assert_eq!(dense_of::<C>(&world0), dense_of::<C>(&world1));
}

/// Asserts that regroup leaves storages without a range untouched,
/// all their components stay implicitly ungrouped.
#[test]
fn ungrouped_storage() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    let e0 = world.add_entity((A(0), C));
    let e1 = world.add_entity((B(1), C));
    let e2 = world.add_entity((A(2), B(2), C));
    let dense_before = dense_of::<C>(&world);

    world.regroup();

    assert_eq!(ungrouped_of::<C>(&world), [e0, e1, e2]);
    assert!(unevaluated_of::<C>(&world).is_empty());
    let c = world.borrow::<View<'_, C>>().unwrap();
    assert_eq!(c.dense, dense_before);
    assert!(!c.groups.has_ranges());
    assert_eq!(c.groups.unevaluated_start, 0);
}

/// Asserts that iterating `(A, B)` yields the same items before and after regroup, in any order.
#[test]
fn iteration() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    world.add_entity((A(0), B(0)));
    world.add_entity(A(1));
    world.regroup();
    world.add_entity((A(2), B(2)));
    world.add_entity(B(3));
    world.add_entity((A(4), B(4)));

    let collect = |world: &World| {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let mut items = (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect::<Vec<_>>();
        items.sort_unstable();
        items
    };

    let before = collect(&world);
    world.regroup();
    let after = collect(&world);

    assert_eq!(before, [(0, 0), (2, 2), (4, 4)]);
    assert_eq!(after, before);
}

/// Asserts that a group with a `NonSend` member is regrouped like any other.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::add_component::AddComponent;
    use crate::borrow::NonSend;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let ab0 = world.add_entity(A(0));
    let a1 = world.add_entity(A(1));
    let ab2 = world.add_entity(A(2));
    {
        let mut b = world.borrow::<NonSend<ViewMut<'_, B>>>().unwrap();
        b.add_component_unchecked(ab2, B(2));
        b.add_component_unchecked(ab0, B(0));
    }

    world.regroup();

    // `assert_regrouped` checks B's range holds the same logical sequence as A's.
    assert_regrouped(&world);
    assert_eq!(sorted(range_of::<A>(&world)), [ab0, ab2]);
    assert_eq!(ungrouped_of::<A>(&world), [a1]);
    let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
    assert_eq!(b.get(ab0), Ok(&B(0)));
    assert_eq!(b.get(ab2), Ok(&B(2)));
}
