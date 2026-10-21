use super::*;
use crate::sparse_set::Ring;

/// Returns every `(entity, value)` of `T`'s storage, sorted.
fn contents<T: Component + Send + Sync>(
    world: &World,
    value: impl Fn(&T) -> u32,
) -> Vec<(EntityId, u32)> {
    let view = world.borrow::<View<'_, T>>().unwrap();
    let mut contents: Vec<_> = view
        .iter()
        .with_id()
        .map(|(entity, component)| (entity, value(component)))
        .collect();
    contents.sort_unstable();

    contents
}

/// Returns `pairs` sorted.
fn sorted_pairs(mut pairs: Vec<(EntityId, u32)>) -> Vec<(EntityId, u32)> {
    pairs.sort_unstable();
    pairs
}

/// Returns `(entity, i)` for the entities `keep` accepts, `i` being their position in `entities`.
fn indexed(entities: &[EntityId], keep: impl Fn(usize) -> bool) -> Vec<(EntityId, u32)> {
    sorted_pairs(
        (0..)
            .zip(entities)
            .filter(|&(i, _)| keep(i as usize))
            .map(|(i, &entity)| (entity, i))
            .collect(),
    )
}

/// Asserts that permanent B removals spread over the range leave the pairs that lost B
/// ungrouped in A and every other pair grouped, with their values.
#[test]
fn removal_scattered() {
    let (mut world, e) = pairs(20);
    let lost = [1, 7, 13];
    for i in lost {
        world.remove::<(B,)>(e[i]);
    }

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    assert_eq!(contents::<A>(&world, |a| a.0), indexed(&e, |_| true));
    assert_eq!(
        contents::<B>(&world, |b| b.0),
        indexed(&e, |i| !lost.contains(&i))
    );
}

/// Asserts that deleting entities spread over the range leaves the remaining pairs grouped,
/// with their values.
#[test]
fn deletion_scattered() {
    let (mut world, e) = pairs(20);
    let deleted = [2, 9, 15];
    for i in deleted {
        world.delete_entity(e[i]);
    }

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    let expected = indexed(&e, |i| !deleted.contains(&i));
    assert_eq!(contents::<A>(&world, |a| a.0), expected);
    assert_eq!(contents::<B>(&world, |b| b.0), expected);
}

/// Asserts that removals damaging both members leave every lone component ungrouped
/// and the remaining pairs grouped, with their values.
#[test]
fn removal_both_members() {
    let (mut world, e) = pairs(20);
    let (lost_a, lost_b) = ([3, 11], [6, 16]);
    for i in lost_a {
        world.remove::<(A,)>(e[i]);
    }
    for i in lost_b {
        world.remove::<(B,)>(e[i]);
    }

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    assert_eq!(
        contents::<A>(&world, |a| a.0),
        indexed(&e, |i| !lost_a.contains(&i))
    );
    assert_eq!(
        contents::<B>(&world, |b| b.0),
        indexed(&e, |i| !lost_b.contains(&i))
    );
}

/// Asserts that pairs losing B and getting a new one back end up grouped again,
/// each with its new B.
#[test]
fn reinsert_scattered() {
    let (mut world, e) = pairs(20);
    let reinserted = [1, 8, 14];
    for i in reinserted {
        world.remove::<(B,)>(e[i]);
    }
    for i in reinserted {
        world.add_component(e[i], B(100 + i as u32));
    }

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    let expected = (0..)
        .zip(&e)
        .map(|(i, &entity)| {
            let b = if reinserted.contains(&(i as usize)) {
                100 + i
            } else {
                i
            };
            (entity, b)
        })
        .collect();
    assert_eq!(contents::<B>(&world, |b| b.0), sorted_pairs(expected));
}

/// Asserts that a mix of permanent removals, removals and re-additions in both members,
/// deletions, new pairs and an ungrouped A gaining B leaves every entity where its components
/// say, with its values.
#[test]
fn mixed() {
    let (mut world, e) = pairs(20);
    let lone = world.add_entity(A(50));
    world.regroup();

    world.remove::<(B,)>(e[2]);
    world.remove::<(B,)>(e[5]);
    world.add_component(e[5], B(105));
    world.delete_entity(e[9]);
    world.remove::<(A,)>(e[12]);
    world.add_component(e[12], A(112));
    let new: Vec<EntityId> = (0..3)
        .map(|i| world.add_entity((A(60 + i), B(60 + i))))
        .collect();
    world.add_component(lone, B(50));

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    let mut expected_a = indexed(&e, |i| i != 9 && i != 12);
    expected_a.extend([(e[12], 112), (lone, 50)]);
    expected_a.extend((60..).zip(&new).map(|(value, &entity)| (entity, value)));
    let mut expected_b = indexed(&e, |i| ![2, 5, 9].contains(&i));
    expected_b.extend([(e[5], 105), (lone, 50)]);
    expected_b.extend((60..).zip(&new).map(|(value, &entity)| (entity, value)));
    assert_eq!(contents::<A>(&world, |a| a.0), sorted_pairs(expected_a));
    assert_eq!(contents::<B>(&world, |b| b.0), sorted_pairs(expected_b));
}

/// Asserts that in a three member group where only B loses and gets back components,
/// every entity is grouped again in all three members with its values.
#[test]
fn one_member_of_three() {
    let mut world = World::builder_for_test().with_group::<(A, B, C)>().build();
    let e: Vec<EntityId> = (0..20).map(|i| world.add_entity((A(i), B(i), C))).collect();
    world.regroup();
    let reinserted = [0, 6, 11, 19];
    for i in reinserted {
        world.remove::<(B,)>(e[i]);
        world.add_component(e[i], B(100 + i as u32));
    }

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&[id::<A>(), id::<B>(), id::<C>()]]);
    assert_eq!(contents::<A>(&world, |a| a.0), indexed(&e, |_| true));
    let expected_b = (0..)
        .zip(&e)
        .map(|(i, &entity)| {
            let b = if reinserted.contains(&(i as usize)) {
                100 + i
            } else {
                i
            };
            (entity, b)
        })
        .collect();
    assert_eq!(contents::<B>(&world, |b| b.0), sorted_pairs(expected_b));
}

/// Returns the ABC union's range in `T`.
fn abc_range<T: Component + Send + Sync>(world: &World) -> (Ring, usize) {
    let view = world.borrow::<View<'_, T>>().unwrap();
    let range = view
        .groups
        .ranges
        .iter()
        .find(|range| range.group.len() == 3)
        .unwrap();

    (range.ring(), range.trust)
}

/// Asserts that with AB, AC and their ABC union rotated in A, AB pairs gaining C, ABC entities
/// losing C or B, and an AC pair gaining B all end up where their components say.
///
/// Only B and C lose components, so A's ABC range stays rotated and fully trusted while holding
/// entities that have to leave it, and others have to join it from outside.
#[test]
fn rotated_promotion_demotion() {
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
    let (ring, trust) = abc_range::<A>(&world);
    assert!(ring.len > 0 && ring.wrap_offset() != 0);
    assert_eq!(trust, ring.len);

    world.add_component(e[3], C);
    world.add_component(e[9], C);
    world.remove::<(C,)>(e[5]);
    world.remove::<(B,)>(e[11]);
    world.add_component(e[7], B(7));

    let (ring, trust) = abc_range::<A>(&world);
    assert!(ring.wrap_offset() != 0);
    assert_eq!(trust, ring.len);

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
    assert_eq!(contents::<A>(&world, |a| a.0), indexed(&e, |_| true));
    let has_b = |i: usize| (i % 3 != 1 || [4, 7].contains(&i)) && i != 11;
    assert_eq!(contents::<B>(&world, |b| b.0), indexed(&e, has_b));
}

/// Asserts that an entity reusing a deleted grouped entity's index joins the group with its own
/// components, and the deleted one's are gone.
#[test]
fn generation_reuse() {
    let (mut world, e) = pairs(10);
    world.delete_entity(e[3]);
    let reused = world.add_entity((A(30), B(30)));
    assert_eq!(reused.index(), e[3].index());

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    let mut expected = indexed(&e, |i| i != 3);
    expected.push((reused, 30));
    assert_eq!(contents::<A>(&world, |a| a.0), sorted_pairs(expected));
}

/// Asserts that clearing a member then giving B back to some pairs regroups them,
/// while the others stay ungrouped in A.
#[test]
fn clear_refill() {
    let (mut world, e) = pairs(10);
    world.borrow::<ViewMut<'_, B>>().unwrap().clear();
    for i in [2, 5, 6] {
        world.add_component(e[i], B(i as u32));
    }

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&ab()]);
    assert_eq!(
        contents::<B>(&world, |b| b.0),
        indexed(&e, |i| [2, 5, 6].contains(&i))
    );
}

/// Asserts that a regroup after scattered removals keeps every remaining component's insertion
/// and modification state, and calls no callback.
#[test]
fn tracking_removals() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    let mut world = World::builder_for_test()
        .with_group::<(TrackedA, TrackedB)>()
        .build();
    let e: Vec<EntityId> = (0..12)
        .map(|i| world.add_entity((TrackedA(i), TrackedB)))
        .collect();
    world.run(|a: ViewMut<'_, TrackedA, track::All>| a.clear_all_inserted());
    for i in 6..12 {
        world.add_entity((TrackedA(i), TrackedB));
    }
    world.run(|mut a: ViewMut<'_, TrackedA, track::All>| {
        for i in [1, 4, 8] {
            (&mut a).get(e[i]).unwrap().0 += 100;
        }
    });
    world.regroup();

    let calls = Arc::new(AtomicUsize::new(0));
    {
        let mut a = world.borrow::<ViewMut<'_, TrackedA, track::All>>().unwrap();
        let removals = calls.clone();
        a.on_removal(move |_, _| {
            removals.fetch_add(1, Ordering::Relaxed);
        });
    }
    let state = |world: &World| {
        let a = world.borrow::<View<'_, TrackedA, track::All>>().unwrap();
        let mut state: Vec<_> = a
            .iter()
            .with_id()
            .map(|(entity, value)| {
                (
                    entity,
                    value.0,
                    a.is_inserted(entity),
                    a.is_modified(entity),
                )
            })
            .collect();
        state.sort_unstable_by_key(|&(entity, ..)| entity);
        state
    };
    for i in [0, 4, 7, 10] {
        world.remove::<(TrackedB,)>(e[i]);
    }
    let before = state(&world);

    world.regroup();

    assert_regrouped(&world);
    assert_memberships(&world, &[&[id::<TrackedA>(), id::<TrackedB>()]]);
    assert_eq!(state(&world), before);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

/// Asserts that a regroup after scattered changes leaves every query with the same results:
/// the grouped pairs, grouped queries filtered on insertion and modification, and the removal log.
/// Then a mutable grouped traversal reaches every pair exactly once, modifying only them.
#[test]
fn queries() {
    let mut world = World::builder_for_test()
        .with_group::<(TrackedA, TrackedB)>()
        .build();
    let e: Vec<EntityId> = (0..16)
        .map(|i| world.add_entity((TrackedA(i), TrackedB)))
        .collect();
    world.regroup();
    world.run(|a: ViewMut<'_, TrackedA, track::All>| a.clear_all_inserted());
    world.run(|mut a: ViewMut<'_, TrackedA, track::All>| {
        for i in [2, 9] {
            (&mut a).get(e[i]).unwrap().0 += 100;
        }
    });
    for i in [1, 6, 13] {
        world.remove::<(TrackedB,)>(e[i]);
    }
    world.add_component(e[6], TrackedB);
    world.add_entity((TrackedA(16), TrackedB));
    world.delete_entity(e[11]);

    let results = |world: &World| {
        let (a, b) = world
            .borrow::<(
                View<'_, TrackedA, track::All>,
                View<'_, TrackedB, track::All>,
            )>()
            .unwrap();
        let ids = |iter: &mut dyn Iterator<Item = EntityId>| sorted(iter.collect());

        (
            ids(&mut (&a, &b).iter().with_id().map(|(entity, _)| entity)),
            ids(&mut (a.inserted(), &b)
                .iter()
                .with_id()
                .map(|(entity, _)| entity)),
            ids(&mut (a.modified(), &b)
                .iter()
                .with_id()
                .map(|(entity, _)| entity)),
            ids(&mut b.removed()),
        )
    };
    let values = |world: &World| {
        let a = world.borrow::<View<'_, TrackedA, track::All>>().unwrap();
        let mut values: Vec<(EntityId, u32)> = a
            .iter()
            .with_id()
            .map(|(entity, a)| (entity, a.0))
            .collect();
        values.sort_unstable();
        values
    };
    let before = results(&world);
    let values_before = values(&world);

    world.regroup();

    assert_regrouped(&world);
    assert_eq!(results(&world), before);
    assert_eq!(values(&world), values_before);

    world.run(
        |(mut a, b): (
            ViewMut<'_, TrackedA, track::All>,
            View<'_, TrackedB, track::All>,
        )| {
            for (mut a, _) in (&mut a, &b).iter() {
                a.0 += 1000;
            }
        },
    );

    let pairs = &before.0;
    let expected: Vec<(EntityId, u32)> = values_before
        .iter()
        .map(|&(entity, value)| {
            (
                entity,
                value + if pairs.contains(&entity) { 1000 } else { 0 },
            )
        })
        .collect();
    assert_eq!(values(&world), expected);
    assert_eq!(results(&world).2, *pairs);
}
