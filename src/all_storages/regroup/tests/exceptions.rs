use super::matched::{crossing, crossing_groups, two_rounds};
use super::*;
use crate::all_storages::regroup::layout::test_observer::{PARTITION_CHECKS, PartitionCheck};

/// Checks the resulting plan, memberships, full trust, sparse mapping and preserved payloads.
fn assert_exceptions(declared: &[&[StorageId]], build: impl FnOnce() -> World) {
    let mut world = build();
    regroup_checked(&mut world, declared);
}

/// Asserts that after scattered B removals, the final memberships and payloads stay correct.
#[test]
fn exceptions_removal() {
    assert_exceptions(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(3) {
            world.remove::<(B,)>(entity);
        }

        world
    });
}

/// Asserts that different removals from both members preserve the remaining pairs and payloads.
#[test]
fn exceptions_both_members() {
    assert_exceptions(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(5) {
            world.remove::<(A,)>(entity);
        }
        for &entity in entities.iter().skip(2).step_by(7) {
            world.remove::<(B,)>(entity);
        }

        world
    });
}

/// Asserts that after A removals from AB, AC and ABC entities, the remaining joins and payloads stay
/// correct.
#[test]
fn exceptions_cross_remove_a() {
    let [ab, ac] = crossing_groups();
    assert_exceptions(&[&ab, &ac], || {
        let (mut world, entities) = crossing(64);
        for &entity in entities.iter().skip(1).step_by(5) {
            world.remove::<(A,)>(entity);
        }

        world
    });
}

/// Asserts that ABC entities losing B move to AC as A's range boundaries shift, preserving payloads.
#[test]
fn exceptions_cross_remove_b() {
    let [ab, ac] = crossing_groups();
    assert_exceptions(&[&ab, &ac], || {
        let (mut world, entities) = crossing(64);
        for &entity in entities.iter().skip(3).step_by(8) {
            world.remove::<(B,)>(entity);
        }

        world
    });
}

/// Asserts that a second round of removals on the rings the first one left rotated gives the
/// expected memberships and preserved payloads.
#[test]
fn exceptions_two_rounds() {
    let [ab, ac] = crossing_groups();
    assert_exceptions(&[&ab, &ac], two_rounds);
}

/// Asserts that after scattered entity deletions, the final memberships and payloads stay correct.
#[test]
fn exceptions_deletion() {
    assert_exceptions(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(3) {
            world.delete_entity(entity);
        }

        world
    });
}

/// Returns a regrouped world of tracked pairs, with some values modified since, then B removed
/// from scattered entities.
fn tracked() -> World {
    let mut world = World::builder_for_test()
        .with_group::<(TrackedA, TrackedB)>()
        .build();
    let entities: Vec<EntityId> = (0..32)
        .map(|i| world.add_entity((TrackedA(i), TrackedB)))
        .collect();
    world.regroup();
    world.run(|a: ViewMut<'_, TrackedA, track::All>| a.clear_all_inserted());
    for i in 32..40 {
        world.add_entity((TrackedA(i), TrackedB));
    }
    world.regroup();

    world.run(|mut a: ViewMut<'_, TrackedA, track::All>| {
        for &entity in entities.iter().skip(1).step_by(4) {
            (&mut a).get(entity).unwrap().0 += 100;
        }
    });
    for &entity in entities.iter().step_by(3) {
        world.remove::<(TrackedB,)>(entity);
    }

    world
}

/// The payload, tracking flags and timestamp mappings regroup must preserve.
#[derive(Debug, PartialEq)]
struct TrackedState {
    values: BTreeMap<EntityId, u32>,
    flags: BTreeMap<EntityId, (bool, bool)>,
    timestamps: [BTreeMap<EntityId, (u64, u64)>; 2],
    removals: [Vec<(EntityId, u64)>; 2],
}

/// Returns tracking data keyed by the complete entity id, independent of physical row order.
fn tracked_state(world: &World) -> TrackedState {
    let (a, b) = world
        .borrow::<(
            View<'_, TrackedA, track::All>,
            View<'_, TrackedB, track::All>,
        )>()
        .unwrap();
    TrackedState {
        values: a.iter().with_id().map(|(id, a)| (id, a.0)).collect(),
        flags: a
            .dense
            .iter()
            .map(|&id| (id, (a.is_inserted(id), a.is_modified(id))))
            .collect(),
        timestamps: [
            a.dense
                .iter()
                .enumerate()
                .map(|(row, &id)| {
                    (
                        id,
                        (a.insertion_data[row].get(), a.modification_data[row].get()),
                    )
                })
                .collect(),
            b.dense
                .iter()
                .enumerate()
                .map(|(row, &id)| {
                    (
                        id,
                        (b.insertion_data[row].get(), b.modification_data[row].get()),
                    )
                })
                .collect(),
        ],
        removals: [
            a.removal_data
                .iter()
                .map(|&(id, time)| (id, time.get()))
                .collect(),
            b.removal_data
                .iter()
                .map(|&(id, time)| (id, time.get()))
                .collect(),
        ],
    }
}

/// Asserts that exception partition preserves payloads, flags and raw timestamps for each entity.
#[test]
fn exceptions_tracked() {
    let mut world = tracked();
    let before = tracked_state(&world);
    let declared = [id::<TrackedA>(), id::<TrackedB>()];

    assert_plan(&world, &[&declared]);
    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&declared]);
    assert_eq!(tracked_state(&world), before);
}

/// Returns a world with the disjoint AB and CD, regrouped, then B and D removed from scattered
/// entities.
fn two_dirty_sets() -> World {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    let entities: Vec<EntityId> = (0..32)
        .map(|i| world.add_entity((A(i), B(i), C, D)))
        .collect();
    world.regroup();
    for &entity in entities.iter().step_by(3) {
        world.remove::<(B, D)>(entity);
    }

    world
}

/// Asserts that regroup_set changes only its selected set and preserves all component values.
#[test]
fn exceptions_regroup_set() {
    let mut world = two_dirty_sets();
    let c = snapshot::<C>(&world);
    let d = snapshot::<D>(&world);
    let values = |world: &World| {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        (
            a.iter()
                .with_id()
                .map(|(id, a)| (id, a.0))
                .collect::<BTreeMap<_, _>>(),
            b.iter()
                .with_id()
                .map(|(id, b)| (id, b.0))
                .collect::<BTreeMap<_, _>>(),
        )
    };
    let before = values(&world);

    world.regroup_set::<A>().unwrap();

    assert_eq!(snapshot::<C>(&world), c);
    assert_eq!(snapshot::<D>(&world), d);
    assert_eq!(values(&world), before);
    let a = snapshot::<A>(&world);
    let b = snapshot::<B>(&world);
    for (dense, ranges, unevaluated_start) in [&a, &b] {
        assert_eq!(*unevaluated_start, dense.len());
        assert!(ranges.iter().all(|&(_, len, _, trust)| len == trust));
    }

    world.regroup();

    assert_eq!(snapshot::<A>(&world), a);
    assert_eq!(snapshot::<B>(&world), b);
    assert_regrouped(&world);
    assert_memberships(&world, &[&ab(), &[id::<C>(), id::<D>()]]);
}

/// Returns the partition decisions made while checking a complete regroup.
fn partition_checks(mut world: World, declared: &[&[StorageId]]) -> Vec<PartitionCheck> {
    PARTITION_CHECKS.with_borrow_mut(|checks| *checks = Some(Vec::new()));
    regroup_checked(&mut world, declared);

    PARTITION_CHECKS.with_borrow_mut(|checks| checks.take().unwrap())
}

/// Asserts that an alone group with new pairs can partition both storages from exceptions.
#[test]
fn exceptions_alone_unevaluated() {
    let (mut world, entities) = pairs(64);
    for &entity in entities.iter().step_by(3) {
        world.remove::<(B,)>(entity);
    }
    world.add_entity((A(100), B(100)));

    let checks = partition_checks(world, &[&ab()]);

    assert_eq!(checks.len(), 2);
    assert!(
        checks.iter().all(|check| check.used_exceptions),
        "{checks:?}"
    );
}

/// Asserts that unevaluated rows in a crossing set require the classification fallback.
#[test]
fn exceptions_unevaluated() {
    let (mut world, entities) = crossing(64);
    for &entity in entities.iter().skip(1).step_by(5) {
        world.remove::<(A,)>(entity);
    }
    world.add_entity((A(100), B(100), C));

    let [ab, ac] = crossing_groups();
    let checks = partition_checks(world, &[&ab, &ac]);

    assert_eq!(checks.len(), 3);
    assert!(
        checks
            .iter()
            .all(|check| !check.rows_covered && !check.used_exceptions),
        "{checks:?}"
    );
}

/// Asserts that promotion alone rejects exception partition in A, while B remains eligible.
#[test]
fn exceptions_promoted() {
    let (mut world, _) = pairs(32);
    for i in 32..40 {
        world.add_entity(A(i));
    }
    world.regroup();
    world.add_component(dense_of::<A>(&world)[32], B(100));

    let checks = partition_checks(world, &[&ab()]);
    let a = checks
        .iter()
        .find(|check| check.storage_id == id::<A>())
        .unwrap();
    let b = checks
        .iter()
        .find(|check| check.storage_id == id::<B>())
        .unwrap();

    assert!(a.rows_covered && a.untouched && a.no_departures, "{a:?}");
    assert!(!a.unpromoted && !a.used_exceptions, "{a:?}");
    assert!(b.used_exceptions, "{b:?}");
}

/// Asserts that a group losing entities from its kept prefix requires the fallback.
#[test]
fn exceptions_leaving() {
    let (mut world, entities) = crossing(64);
    world.add_component(entities[1], C);

    let [ab, ac] = crossing_groups();
    let checks = partition_checks(world, &[&ab, &ac]);

    assert!(
        checks.iter().any(|check| !check.no_departures),
        "{checks:?}"
    );
    assert!(
        checks.iter().all(|check| !check.used_exceptions),
        "{checks:?}"
    );
}

/// Asserts that a departure alone rejects B's exceptions when the departing pair was its last row.
#[test]
fn exceptions_leaving_edge() {
    let builder = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>();
    let (mut world, _) = pairs_in(builder, 16);
    let group = groups_of::<B>(&world)[0].clone();
    let last = *group_of::<B>(&world, &group).0.last().unwrap();
    world.add_component(last, C);

    let checks = partition_checks(world, &[&ab(), &[id::<A>(), id::<C>()]]);
    let b = checks
        .iter()
        .find(|check| check.storage_id == id::<B>())
        .unwrap();

    assert!(b.rows_covered && b.untouched && b.unpromoted, "{b:?}");
    assert!(!b.no_departures && !b.used_exceptions, "{b:?}");
}

/// Asserts that moved ids alone reject A's recorded rows, while B and C remain eligible.
#[test]
fn exceptions_touched() {
    let (mut world, entities) = crossing(64);
    for &entity in entities.iter().skip(1).step_by(8) {
        world.remove::<(B,)>(entity);
    }

    let [ab, ac] = crossing_groups();
    let checks = partition_checks(world, &[&ab, &ac]);
    let a = checks
        .iter()
        .find(|check| check.storage_id == id::<A>())
        .unwrap();

    assert!(a.rows_covered && a.unpromoted && a.no_departures, "{a:?}");
    assert!(!a.untouched && !a.used_exceptions, "{a:?}");
    assert_eq!(
        checks.iter().filter(|check| check.used_exceptions).count(),
        2
    );
}

/// Asserts that evaluated and unevaluated exceptions both survive in the same storage plan.
#[test]
fn exceptions_both_scans() {
    let (mut world, entities) = pairs(8);
    world.remove::<(B,)>(entities[1]);
    world.add_entity(A(20));
    world.add_entity((A(21), B(21)));
    let unevaluated_start = world
        .borrow::<View<'_, A>>()
        .unwrap()
        .groups
        .unevaluated_start;

    let plan = assert_plan(&world, &[&ab()]);
    let exceptions = &plan.storage(&id::<A>()).exceptions;

    assert_eq!(exceptions.len(), 2);
    assert!(exceptions[0].0 < unevaluated_start);
    assert!(exceptions[1].0 >= unevaluated_start);
    assert!(
        exceptions
            .iter()
            .all(|&(_, destination)| destination.is_none())
    );
    regroup_checked(&mut world, &[&ab()]);
}
