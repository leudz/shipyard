use super::*;

/// Checks the plan, memberships and full-trust alignment while preserving survivors and payloads.
fn assert_matching(declared: &[&[StorageId]], build: impl FnOnce() -> World) {
    let mut world = build();
    let state = |world: &World| {
        let (entities, a, b, c) = world
            .borrow::<(
                crate::EntitiesView<'_>,
                View<'_, A>,
                View<'_, B>,
                View<'_, C>,
            )>()
            .unwrap();
        (
            sorted(entities.iter().collect()),
            a.iter()
                .with_id()
                .map(|(id, a)| (id, a.0))
                .collect::<BTreeMap<_, _>>(),
            b.iter()
                .with_id()
                .map(|(id, b)| (id, b.0))
                .collect::<BTreeMap<_, _>>(),
            sorted(c.dense.clone()),
        )
    };
    let before = state(&world);

    assert_plan(&world, declared);
    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, declared);
    assert_eq!(state(&world), before);
}

/// The groups of [`crossing`].
pub(super) fn crossing_groups() -> [Vec<StorageId>; 2] {
    [ab().to_vec(), sorted_ids(vec![id::<A>(), id::<C>()])]
}

/// Returns a world grouping AB and AC, with A, AB, AC and ABC entities in turn, regrouped.
pub(super) fn crossing(count: u32) -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let entities = (0..count)
        .map(|i| match i % 4 {
            0 => world.add_entity(A(i)),
            1 => world.add_entity((A(i), B(i))),
            2 => world.add_entity((A(i), C)),
            _ => world.add_entity((A(i), B(i), C)),
        })
        .collect();
    world.regroup();

    (world, entities)
}

/// Returns [`crossing`] after a round of A removals and its regroup, which leaves rotated rings
/// in A.
fn first_round() -> (World, Vec<EntityId>) {
    let (mut world, entities) = crossing(64);
    for &entity in entities.iter().skip(1).step_by(5) {
        world.remove::<(A,)>(entity);
    }
    world.regroup();

    (world, entities)
}

/// Returns [`first_round`]'s world after a second round of A and B removals.
pub(super) fn two_rounds() -> World {
    let (mut world, entities) = first_round();
    for &entity in entities.iter().skip(2).step_by(5) {
        world.remove::<(A,)>(entity);
    }
    for &entity in entities.iter().skip(3).step_by(6) {
        world.remove::<(B,)>(entity);
    }

    world
}

/// Asserts that scattered B removals preserve the remaining AB pairs and their payloads.
#[test]
fn matched_removal() {
    assert_matching(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(3) {
            world.remove::<(B,)>(entity);
        }

        world
    });
}

/// Asserts that different removals from both members leave exactly the pairs still owning A and B.
#[test]
fn matched_both_members() {
    assert_matching(&[&ab()], || {
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

/// Asserts that scattered deletions preserve the surviving pairs, with no deleted entity reappearing.
#[test]
fn matched_deletion() {
    assert_matching(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(3) {
            world.delete_entity(entity);
        }

        world
    });
}

/// Asserts that A removals across AB, AC and ABC preserve the remaining joins and payloads.
#[test]
fn matched_cross_remove_a() {
    let [ab, ac] = crossing_groups();
    assert_matching(&[&ab, &ac], || {
        let (mut world, entities) = crossing(64);
        for &entity in entities.iter().skip(1).step_by(5) {
            world.remove::<(A,)>(entity);
        }

        world
    });
}

/// Asserts that ABC entities losing B move to AC while preserving their other components.
#[test]
fn matched_cross_remove_b() {
    let [ab, ac] = crossing_groups();
    assert_matching(&[&ab, &ac], || {
        let (mut world, entities) = crossing(64);
        for &entity in entities.iter().skip(3).step_by(8) {
            world.remove::<(B,)>(entity);
        }

        world
    });
}

/// Asserts that a second round of removals preserves memberships on rings left rotated by the first.
#[test]
fn matched_two_rounds() {
    let (world, _) = first_round();
    let (_, ranges, _) = snapshot::<A>(&world);
    assert!(
        ranges
            .iter()
            .any(|&(_, len, shift, _)| len > 0 && shift.rem_euclid(len.cast_signed()) != 0),
        "{ranges:?}"
    );

    let [ab, ac] = crossing_groups();
    assert_matching(&[&ab, &ac], two_rounds);
}

/// Returns [`crossing`] after scattered A removals, with C added to a lone A entity:
/// its only unevaluated row.
pub(super) fn one_unevaluated() -> World {
    let (mut world, entities) = crossing(64);
    for &entity in entities.iter().skip(1).step_by(5) {
        world.remove::<(A,)>(entity);
    }
    world.add_component(entities[4], C);
    {
        let (a, b, c) = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();
        let unevaluated = [
            a.len() - a.groups.unevaluated_start,
            b.len() - b.groups.unevaluated_start,
            c.len() - c.groups.unevaluated_start,
        ];
        assert_eq!(unevaluated.into_iter().sum::<usize>(), 1);
    }

    world
}

/// Asserts that a crossing set with one unevaluated row preserves the expected memberships and payloads.
#[test]
fn matched_unevaluated() {
    let [ab, ac] = crossing_groups();
    assert_matching(&[&ab, &ac], one_unevaluated);
}

/// Asserts that removed and re-added B components rejoin AB with their replacement payloads.
#[test]
fn matched_readded() {
    assert_matching(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(3) {
            world.remove::<(B,)>(entity);
        }
        for (i, &entity) in (100..).zip(&entities).step_by(6) {
            world.add_component(entity, B(i));
        }

        world
    });
}

/// Asserts that reused indices classify the new AB and lone A entities using their new generations.
#[test]
fn matched_new_generation() {
    assert_matching(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(4) {
            world.delete_entity(entity);
        }
        for i in 0..16 {
            let entity = if i % 2 == 0 {
                world.add_entity((A(100 + i), B(100 + i)))
            } else {
                world.add_entity(A(100 + i))
            };
            let old = entities
                .iter()
                .find(|old| old.index() == entity.index())
                .unwrap();
            assert_ne!(*old, entity);
        }

        world
    });
}

/// Asserts that with AB and AC, AB entities past the kept prefix that gain C go to ABC:
/// they still own every member of AB, but that's no longer their destination.
#[test]
fn matched_gained() {
    let [ab, ac] = crossing_groups();
    assert_matching(&[&ab, &ac], || {
        let (mut world, entities) = crossing(64);
        world.remove::<(A,)>(entities[1]);
        for &entity in entities.iter().skip(5).step_by(8) {
            world.add_component(entity, C);
        }
        {
            let (a, b, c) = world
                .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
                .unwrap();
            let a_range = a
                .groups
                .ranges
                .iter()
                .find(|range| *range.group == ab)
                .unwrap();
            let b_range = b.groups.range(&a_range.group).unwrap();
            let mut aligned = 0;
            for &entity in entities.iter().skip(5).step_by(8) {
                let row = a.sparse.get(entity).unwrap().uindex();
                let logical = a_range.ring().logical_of(row);
                assert!(logical >= a_range.trust);
                if logical < b_range.len && b.dense[b_range.ring().physical_of(logical)] == entity {
                    aligned += 1;
                }
            }
            assert!(aligned > 0);
            assert!(c.groups.unevaluated_start < c.len());
        }

        world
    });
}
