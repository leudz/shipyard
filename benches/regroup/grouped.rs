use super::fixtures::{A, B, C, crossing_grouped_world, grouped_world, shuffled};
use criterion::Criterion;
use shipyard::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, EntityId, IntoIter, Remove, View, ViewMut,
};
use std::time::{Duration, Instant};

/// Times bulk creation of 100,000 `AB` entities, starting from an empty world with `(A, B)` grouped.
pub fn spawn_ab_bulk(criterion: &mut Criterion) {
    const ENTITIES: u32 = 100_000;

    // Input: logical entity `i` gets `A::new(i)` and `B::new(i)`.
    // We build it once, every invocation copies from it.
    let input: Vec<(A, B)> = (0..ENTITIES).map(|i| (A::new(i), B::new(i))).collect();

    // Measurement. Each invocation fills a fresh empty world, the checks get the last one.
    // The timer covers the initial regroup, the views are dropped before it.
    let mut last_world = None;
    criterion.bench_function("spawn_ab_bulk/shipyard_grouped", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_world = None;
                let mut world = grouped_world();

                let start = Instant::now();
                {
                    let (mut entities, mut vm_a, mut vm_b) = world
                        .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>)>()
                        .unwrap();
                    entities.bulk_add_entity((&mut vm_a, &mut vm_b), input.iter().copied());
                }
                world.regroup();
                elapsed += start.elapsed();

                last_world = Some(world);
            }
            elapsed
        })
    });

    // Checks: 100,000 live entities, each with both of its payloads,
    // and the `(A, B)` join matches each of them exactly once.
    // A filtered-out benchmark never ran, there's no world to check.
    let Some(world) = last_world else {
        return;
    };
    let (entities, v_a, v_b) = world.borrow::<(EntitiesView, View<A>, View<B>)>().unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize);
    assert_eq!(v_a.len(), ENTITIES as usize);
    assert_eq!(v_b.len(), ENTITIES as usize);
    // `A::new(i).x` is `i`, so each match tells us its logical entity.
    let mut matched = vec![false; ENTITIES as usize];
    for (a, b) in (&v_a, &v_b).iter() {
        let i = a.x as u32;
        assert_eq!(*a, A::new(i), "entity {i}");
        assert_eq!(*b, B::new(i), "entity {i}");
        assert!(!matched[i as usize], "entity {i} matched twice");
        matched[i as usize] = true;
    }
    assert!(matched.iter().all(|&m| m));
}

/// Times complete deletion of 10,000 scattered entities out of 100,000 `AB` entities,
/// `(A, B)` grouped.
pub fn delete_ab(criterion: &mut Criterion) {
    const ENTITIES: u32 = 100_000;
    const DELETED: usize = 10_000;
    const SEED: u64 = 0x5EED_0002;

    // Selection: the first 10,000 logical entities of the `SEED` shuffle of 0..100,000,
    // deleted in shuffled order.
    let mut selected = shuffled((0..ENTITIES).collect(), SEED);
    selected.truncate(DELETED);

    // Measurement. Each invocation builds a fresh population, the checks get the last one.
    // The timer covers the regroup after deletion, the view is dropped before it.
    let mut last_sample = None;
    criterion.bench_function("delete_ab/shipyard_grouped", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_sample = None;
                let mut world = grouped_world();

                // Population: logical entity `i` is `ids[i]`, with `A::new(i)` and `B::new(i)`.
                // It's regrouped before deletion starts.
                let ids: Vec<EntityId> = {
                    let (mut entities, mut vm_a, mut vm_b) = world
                        .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>)>()
                        .unwrap();

                    (0..ENTITIES)
                        .map(|i| {
                            entities.add_entity((&mut vm_a, &mut vm_b), (A::new(i), B::new(i)))
                        })
                        .collect()
                };
                world.regroup();
                let deleted: Vec<EntityId> = selected.iter().map(|&i| ids[i as usize]).collect();

                let start = Instant::now();
                {
                    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
                    for &id in &deleted {
                        all_storages.delete_entity(id);
                    }
                }
                world.regroup();
                elapsed += start.elapsed();

                last_sample = Some((world, ids));
            }
            elapsed
        })
    });

    // Checks: the selected entities are dead without components, the 90,000 survivors keep
    // their original values, and the `(A, B)` join matches exactly the survivors.
    // A filtered-out benchmark never ran, there's no world to check.
    let Some((world, ids)) = last_sample else {
        return;
    };
    let mut is_deleted = vec![false; ENTITIES as usize];
    for &i in &selected {
        is_deleted[i as usize] = true;
    }
    let (entities, v_a, v_b) = world.borrow::<(EntitiesView, View<A>, View<B>)>().unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize - DELETED);
    assert_eq!(v_a.len(), ENTITIES as usize - DELETED);
    assert_eq!(v_b.len(), ENTITIES as usize - DELETED);
    // `A::new(i).x` is `i`, so each match tells us its logical entity.
    let mut matched = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_b).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched[i], "entity {i} matched twice");
        matched[i] = true;
    }
    for (i, &id) in (0..ENTITIES).zip(&ids) {
        if is_deleted[i as usize] {
            assert!(!entities.is_alive(id), "entity {i}");
            assert!(!v_a.contains(id) && !v_b.contains(id), "entity {i}");
        } else {
            assert!(entities.is_alive(id), "entity {i}");
            assert_eq!(v_a[id], A::new(i), "entity {i}");
            assert_eq!(v_b[id], B::new(i), "entity {i}");
        }
        assert_eq!(matched[i as usize], !is_deleted[i as usize], "entity {i}");
    }
}

/// Times adding `B` back to 10,000 scattered entities, out of 90,000 `AB` and 10,000 `A`,
/// `(A, B)` grouped.
pub fn add_b(criterion: &mut Criterion) {
    const ENTITIES: u32 = 100_000;
    const RESTORED: usize = 10_000;
    const SEED: u64 = 0x5EED_0002;

    // Selection: the first 10,000 logical entities of the `SEED` shuffle of 0..100,000,
    // restored in shuffled order.
    let mut selected = shuffled((0..ENTITIES).collect(), SEED);
    selected.truncate(RESTORED);

    // Measurement. Each invocation builds a fresh population, the checks get the last one.
    // The timer covers the regroup after restoration, the views are dropped before it.
    let mut last_sample = None;
    criterion.bench_function("add_b/shipyard_grouped", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_sample = None;
                let mut world = grouped_world();

                // Population: logical entity `i` is `ids[i]`, with `A::new(i)` and `B::new(i)`.
                // It's regrouped before the removal.
                let ids: Vec<EntityId> = {
                    let (mut entities, mut vm_a, mut vm_b) = world
                        .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>)>()
                        .unwrap();

                    (0..ENTITIES)
                        .map(|i| {
                            entities.add_entity((&mut vm_a, &mut vm_b), (A::new(i), B::new(i)))
                        })
                        .collect()
                };
                world.regroup();

                // Starting state: we take `B` from the selected entities in selection order,
                // keeping the removed values to restore, and regroup.
                // Every selected entity had its `B`, so 100,000 `A` and 90,000 `B` are left.
                let removed: Vec<(EntityId, B)> = {
                    let mut vm_b = world.borrow::<ViewMut<B>>().unwrap();

                    selected
                        .iter()
                        .map(|&i| {
                            let id = ids[i as usize];
                            (id, vm_b.remove(id).unwrap())
                        })
                        .collect()
                };
                world.regroup();

                let start = Instant::now();
                {
                    let (entities, mut vm_b) =
                        world.borrow::<(EntitiesView, ViewMut<B>)>().unwrap();
                    for &(id, b) in &removed {
                        entities.add_component(id, &mut vm_b, b);
                    }
                }
                world.regroup();
                elapsed += start.elapsed();

                last_sample = Some((world, ids));
            }
            elapsed
        })
    });

    // Checks: all 100,000 entities are alive with their original `A` and `B`,
    // and the `(A, B)` join matches each of them exactly once.
    // A filtered-out benchmark never ran, there's no world to check.
    let Some((world, ids)) = last_sample else {
        return;
    };
    let (entities, v_a, v_b) = world.borrow::<(EntitiesView, View<A>, View<B>)>().unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize);
    assert_eq!(v_a.len(), ENTITIES as usize);
    assert_eq!(v_b.len(), ENTITIES as usize);
    // `A::new(i).x` is `i`, so each match tells us its logical entity.
    let mut matched = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_b).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched[i], "entity {i} matched twice");
        matched[i] = true;
    }
    for (i, &id) in (0..ENTITIES).zip(&ids) {
        assert!(entities.is_alive(id), "entity {i}");
        assert_eq!(v_a[id], A::new(i), "entity {i}");
        assert_eq!(v_b[id], B::new(i), "entity {i}");
        assert!(matched[i as usize], "entity {i}");
    }
}

/// Times removing `B` from 10,000 scattered entities out of 100,000 `AB` entities,
/// `(A, B)` grouped.
pub fn remove_b(criterion: &mut Criterion) {
    const ENTITIES: u32 = 100_000;
    const REMOVED: usize = 10_000;
    const SEED: u64 = 0x5EED_0002;

    // Selection: the first 10,000 logical entities of the `SEED` shuffle of 0..100,000,
    // removed in shuffled order.
    let mut selected = shuffled((0..ENTITIES).collect(), SEED);
    selected.truncate(REMOVED);

    // Measurement. Each invocation builds a fresh population, the checks get the last one.
    // The timer covers the regroup after removal, the view is dropped before it.
    let mut last_sample = None;
    criterion.bench_function("remove_b/shipyard_grouped", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_sample = None;
                let mut world = grouped_world();

                // Population: logical entity `i` is `ids[i]`, with `A::new(i)` and `B::new(i)`.
                // It's regrouped before removal starts.
                let ids: Vec<EntityId> = {
                    let (mut entities, mut vm_a, mut vm_b) = world
                        .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>)>()
                        .unwrap();

                    (0..ENTITIES)
                        .map(|i| {
                            entities.add_entity((&mut vm_a, &mut vm_b), (A::new(i), B::new(i)))
                        })
                        .collect()
                };
                world.regroup();
                let removed: Vec<EntityId> = selected.iter().map(|&i| ids[i as usize]).collect();

                let start = Instant::now();
                {
                    let mut vm_b = world.borrow::<ViewMut<B>>().unwrap();
                    for &id in &removed {
                        vm_b.remove(id);
                    }
                }
                world.regroup();
                elapsed += start.elapsed();

                last_sample = Some((world, ids));
            }
            elapsed
        })
    });

    // Checks: all 100,000 entities are alive with their original `A`, only the selected ones
    // lost their `B`, the others keep it unchanged, and the `(A, B)` join matches exactly
    // the unselected entities.
    // A filtered-out benchmark never ran, there's no world to check.
    let Some((world, ids)) = last_sample else {
        return;
    };
    let mut is_removed = vec![false; ENTITIES as usize];
    for &i in &selected {
        is_removed[i as usize] = true;
    }
    let (entities, v_a, v_b) = world.borrow::<(EntitiesView, View<A>, View<B>)>().unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize);
    assert_eq!(v_a.len(), ENTITIES as usize);
    assert_eq!(v_b.len(), ENTITIES as usize - REMOVED);
    // `A::new(i).x` is `i`, so each match tells us its logical entity.
    let mut matched = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_b).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched[i], "entity {i} matched twice");
        matched[i] = true;
    }
    for (i, &id) in (0..ENTITIES).zip(&ids) {
        assert!(entities.is_alive(id), "entity {i}");
        assert_eq!(v_a[id], A::new(i), "entity {i}");
        if is_removed[i as usize] {
            assert!(!v_b.contains(id), "entity {i}");
        } else {
            assert_eq!(v_b[id], B::new(i), "entity {i}");
        }
        assert_eq!(matched[i as usize], !is_removed[i as usize], "entity {i}");
    }
}

/// Times adding `A` back to 2,500 scattered entities in each of the `AB`, `AC` and `ABC` quarters,
/// out of 25,000 entities each of `A`, `AB`, `AC` and `ABC`, `(A, B)` and `(A, C)` grouped.
pub fn cross_add_a(criterion: &mut Criterion) {
    // Logical entities 0..25,000 are `A`, 25,000..50,000 `AB`, 50,000..75,000 `AC`,
    // and 75,000..100,000 `ABC`.
    const ENTITIES: u32 = 100_000;
    const AB_START: u32 = 25_000;
    const AC_START: u32 = 50_000;
    const ABC_START: u32 = 75_000;
    const RESTORED_PER_QUARTER: usize = 2_500;
    const SEED: u64 = 0x5EED_0004;

    // Selection: the first 2,500 logical entities of each of the `AB`, `AC` and `ABC` quarters'
    // shuffles, seeded `SEED`, `SEED + 1` and `SEED + 2`, taken in turns: `AB`, `AC`, `ABC`, `AB`, ...
    // We restore them in that order.
    let ab_shuffle = shuffled((AB_START..AC_START).collect(), SEED);
    let ac_shuffle = shuffled((AC_START..ABC_START).collect(), SEED + 1);
    let abc_shuffle = shuffled((ABC_START..ENTITIES).collect(), SEED + 2);
    let selected: Vec<u32> = (0..RESTORED_PER_QUARTER)
        .flat_map(|j| [ab_shuffle[j], ac_shuffle[j], abc_shuffle[j]])
        .collect();

    // Measurement. Each invocation builds a fresh population, the checks get the last one.
    // The timer covers the regroup after restoration, the views are dropped before it.
    let mut last_sample = None;
    criterion.bench_function("cross_add_a/shipyard_grouped", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_sample = None;
                let mut world = crossing_grouped_world();

                // Population: logical entity `i` is `ids[i]`, with `A::new(i)`, and `B::new(i)`
                // and `C::new(i)` if it has them. It's regrouped before the removal.
                let ids: Vec<EntityId> = {
                    let (mut entities, mut vm_a, mut vm_b, mut vm_c) = world
                        .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>, ViewMut<C>)>()
                        .unwrap();

                    let mut ids: Vec<EntityId> = (0..AB_START)
                        .map(|i| entities.add_entity(&mut vm_a, A::new(i)))
                        .collect();
                    ids.extend((AB_START..AC_START).map(|i| {
                        entities.add_entity((&mut vm_a, &mut vm_b), (A::new(i), B::new(i)))
                    }));
                    ids.extend((AC_START..ABC_START).map(|i| {
                        entities.add_entity((&mut vm_a, &mut vm_c), (A::new(i), C::new(i)))
                    }));
                    ids.extend((ABC_START..ENTITIES).map(|i| {
                        entities.add_entity(
                            (&mut vm_a, &mut vm_b, &mut vm_c),
                            (A::new(i), B::new(i), C::new(i)),
                        )
                    }));

                    ids
                };
                world.regroup();

                // Starting state: we take `A` from the selected entities in selection order,
                // keeping the removed values to restore, and regroup.
                let removed: Vec<(EntityId, A)> = {
                    let mut vm_a = world.borrow::<ViewMut<A>>().unwrap();

                    selected
                        .iter()
                        .map(|&i| {
                            let id = ids[i as usize];
                            (id, vm_a.remove(id).unwrap())
                        })
                        .collect()
                };
                world.regroup();
                // That leaves 92,500 `A`, 50,000 `B` and `C`, and 45,000 matches for each join.
                {
                    let (v_a, v_b, v_c) = world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
                    assert_eq!(v_a.len(), 92_500);
                    assert_eq!(v_b.len(), 50_000);
                    assert_eq!(v_c.len(), 50_000);
                    assert_eq!((&v_a, &v_b).iter().count(), 45_000);
                    assert_eq!((&v_a, &v_c).iter().count(), 45_000);
                }

                let start = Instant::now();
                {
                    let (entities, mut vm_a) =
                        world.borrow::<(EntitiesView, ViewMut<A>)>().unwrap();
                    for &(id, a) in &removed {
                        entities.add_component(id, &mut vm_a, a);
                    }
                }
                world.regroup();
                elapsed += start.elapsed();

                last_sample = Some((world, ids));
            }
            elapsed
        })
    });

    // Checks: all 100,000 entities are alive with exactly their quarter's payloads, unchanged,
    // and each join matches exactly its two quarters, each entity once.
    // A filtered-out benchmark never ran, there's no world to check.
    let Some((world, ids)) = last_sample else {
        return;
    };
    let (entities, v_a, v_b, v_c) = world
        .borrow::<(EntitiesView, View<A>, View<B>, View<C>)>()
        .unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize);
    assert_eq!(v_a.len(), ENTITIES as usize);
    // The `AB` and `ABC` quarters hold `B`, the `AC` and `ABC` quarters hold `C`.
    assert_eq!(v_b.len(), 50_000);
    assert_eq!(v_c.len(), 50_000);
    // `A::new(i).x` is `i`, so each match tells us its logical entity.
    let mut matched_ab = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_b).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched_ab[i], "entity {i} matched twice");
        matched_ab[i] = true;
    }
    let mut matched_ac = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_c).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched_ac[i], "entity {i} matched twice");
        matched_ac[i] = true;
    }
    for (i, &id) in (0..ENTITIES).zip(&ids) {
        let has_b = (AB_START..AC_START).contains(&i) || i >= ABC_START;
        let has_c = i >= AC_START;

        assert!(entities.is_alive(id), "entity {i}");
        assert_eq!(v_a[id], A::new(i), "entity {i}");
        if has_b {
            assert_eq!(v_b[id], B::new(i), "entity {i}");
        } else {
            assert!(!v_b.contains(id), "entity {i}");
        }
        if has_c {
            assert_eq!(v_c[id], C::new(i), "entity {i}");
        } else {
            assert!(!v_c.contains(id), "entity {i}");
        }
        assert_eq!(matched_ab[i as usize], has_b, "entity {i}");
        assert_eq!(matched_ac[i as usize], has_c, "entity {i}");
    }
}

/// Times removing `A` from 2,500 scattered entities in each of the `AB`, `AC` and `ABC` quarters,
/// out of 25,000 entities each of `A`, `AB`, `AC` and `ABC`, `(A, B)` and `(A, C)` grouped.
pub fn cross_remove_a(criterion: &mut Criterion) {
    // Logical entities 0..25,000 are `A`, 25,000..50,000 `AB`, 50,000..75,000 `AC`,
    // and 75,000..100,000 `ABC`.
    const ENTITIES: u32 = 100_000;
    const AB_START: u32 = 25_000;
    const AC_START: u32 = 50_000;
    const ABC_START: u32 = 75_000;
    const REMOVED_PER_QUARTER: usize = 2_500;
    const SEED: u64 = 0x5EED_0004;

    // Selection: the first 2,500 logical entities of each of the `AB`, `AC` and `ABC` quarters'
    // shuffles, seeded `SEED`, `SEED + 1` and `SEED + 2`, taken in turns: `AB`, `AC`, `ABC`, `AB`, ...
    // We remove them in that order.
    let ab_shuffle = shuffled((AB_START..AC_START).collect(), SEED);
    let ac_shuffle = shuffled((AC_START..ABC_START).collect(), SEED + 1);
    let abc_shuffle = shuffled((ABC_START..ENTITIES).collect(), SEED + 2);
    let selected: Vec<u32> = (0..REMOVED_PER_QUARTER)
        .flat_map(|j| [ab_shuffle[j], ac_shuffle[j], abc_shuffle[j]])
        .collect();

    // Measurement. Each invocation builds a fresh population, the checks get the last one.
    // The timer covers the regroup after removal, the view is dropped before it.
    let mut last_sample = None;
    criterion.bench_function("cross_remove_a/shipyard_grouped", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_sample = None;
                let mut world = crossing_grouped_world();

                // Population: logical entity `i` is `ids[i]`, with `A::new(i)`, and `B::new(i)`
                // and `C::new(i)` if it has them. It's regrouped before removal starts.
                let ids: Vec<EntityId> = {
                    let (mut entities, mut vm_a, mut vm_b, mut vm_c) = world
                        .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>, ViewMut<C>)>()
                        .unwrap();

                    let mut ids: Vec<EntityId> = (0..AB_START)
                        .map(|i| entities.add_entity(&mut vm_a, A::new(i)))
                        .collect();
                    ids.extend((AB_START..AC_START).map(|i| {
                        entities.add_entity((&mut vm_a, &mut vm_b), (A::new(i), B::new(i)))
                    }));
                    ids.extend((AC_START..ABC_START).map(|i| {
                        entities.add_entity((&mut vm_a, &mut vm_c), (A::new(i), C::new(i)))
                    }));
                    ids.extend((ABC_START..ENTITIES).map(|i| {
                        entities.add_entity(
                            (&mut vm_a, &mut vm_b, &mut vm_c),
                            (A::new(i), B::new(i), C::new(i)),
                        )
                    }));

                    ids
                };
                world.regroup();
                let removed: Vec<EntityId> = selected.iter().map(|&i| ids[i as usize]).collect();

                let start = Instant::now();
                {
                    let mut vm_a = world.borrow::<ViewMut<A>>().unwrap();
                    for &id in &removed {
                        vm_a.remove(id);
                    }
                }
                world.regroup();
                elapsed += start.elapsed();

                last_sample = Some((world, ids));
            }
            elapsed
        })
    });

    // Checks: all 100,000 entities are alive, only the selected ones lost their `A`, every other
    // payload is unchanged, and each join matches exactly the unselected entities of its two
    // quarters, each once.
    // A filtered-out benchmark never ran, there's no world to check.
    let Some((world, ids)) = last_sample else {
        return;
    };
    let mut is_removed = vec![false; ENTITIES as usize];
    for &i in &selected {
        is_removed[i as usize] = true;
    }
    let (entities, v_a, v_b, v_c) = world
        .borrow::<(EntitiesView, View<A>, View<B>, View<C>)>()
        .unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize);
    assert_eq!(v_a.len(), 92_500);
    // The `AB` and `ABC` quarters hold `B`, the `AC` and `ABC` quarters hold `C`.
    assert_eq!(v_b.len(), 50_000);
    assert_eq!(v_c.len(), 50_000);
    // `A::new(i).x` is `i`, so each match tells us its logical entity.
    let mut matched_ab = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_b).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched_ab[i], "entity {i} matched twice");
        matched_ab[i] = true;
    }
    let mut matched_ac = vec![false; ENTITIES as usize];
    for (id, (a, _)) in (&v_a, &v_c).iter().with_id() {
        let i = a.x as usize;
        assert_eq!(id, ids[i], "entity {i}");
        assert!(!matched_ac[i], "entity {i} matched twice");
        matched_ac[i] = true;
    }
    assert_eq!(matched_ab.iter().filter(|&&m| m).count(), 45_000);
    assert_eq!(matched_ac.iter().filter(|&&m| m).count(), 45_000);
    for (i, &id) in (0..ENTITIES).zip(&ids) {
        let has_a = !is_removed[i as usize];
        let has_b = (AB_START..AC_START).contains(&i) || i >= ABC_START;
        let has_c = i >= AC_START;

        assert!(entities.is_alive(id), "entity {i}");
        if has_a {
            assert_eq!(v_a[id], A::new(i), "entity {i}");
        } else {
            assert!(!v_a.contains(id), "entity {i}");
        }
        if has_b {
            assert_eq!(v_b[id], B::new(i), "entity {i}");
        } else {
            assert!(!v_b.contains(id), "entity {i}");
        }
        if has_c {
            assert_eq!(v_c[id], C::new(i), "entity {i}");
        } else {
            assert!(!v_c.contains(id), "entity {i}");
        }
        assert_eq!(matched_ab[i as usize], has_a && has_b, "entity {i}");
        assert_eq!(matched_ac[i as usize], has_a && has_c, "entity {i}");
    }
}
