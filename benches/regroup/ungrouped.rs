use super::fixtures::{A, B, shuffled};
use criterion::Criterion;
use shipyard::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, EntityId, IntoIter, Remove, View, ViewMut,
    World,
};
use std::time::{Duration, Instant};

/// Times complete deletion of 10,000 scattered entities out of 100,000 `AB` entities.
pub fn delete_ab(criterion: &mut Criterion) {
    const ENTITIES: u32 = 100_000;
    const DELETED: usize = 10_000;
    const SEED: u64 = 0x5EED_0002;

    // Selection: the first 10,000 logical entities of the `SEED` shuffle of 0..100,000,
    // deleted in shuffled order.
    let mut selected = shuffled((0..ENTITIES).collect(), SEED);
    selected.truncate(DELETED);

    // Measurement. Each invocation builds a fresh population, the checks get the last one.
    let mut last_sample = None;
    criterion.bench_function("delete_ab/shipyard_current", |bencher| {
        bencher.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iters {
                // We drop the previous world before making the next one, outside the timer.
                last_sample = None;
                let world = World::new();

                // Population: logical entity `i` is `ids[i]`, with `A::new(i)` and `B::new(i)`.
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
                let deleted: Vec<EntityId> = selected.iter().map(|&i| ids[i as usize]).collect();

                let start = Instant::now();
                {
                    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
                    for &id in &deleted {
                        all_storages.delete_entity(id);
                    }
                }
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

/// Times mutable `A` joined with shared `B` over 100,000 `AB` entities and 100,000 entities
/// with neither payload.
pub fn iterate_ab_full(criterion: &mut Criterion) {
    // Logical entities 0..100,000 are `AB`, 100,000..200,000 have neither payload.
    const ENTITIES: u32 = 200_000;
    const AB_END: u32 = 100_000;
    // Preparation removes `A` from this many of the `A` holders, then adds it back.
    const CHURNED: usize = 10_000;
    const SEED: u64 = 0x5EED_0001;

    let world = World::new();

    // Population: logical entity `i` is `ids[i]`, with `A::new(i)` and `B::new(i)` if it has them.
    let ids: Vec<EntityId> = {
        let (mut entities, mut vm_a, mut vm_b) = world
            .borrow::<(EntitiesViewMut, ViewMut<A>, ViewMut<B>)>()
            .unwrap();

        let mut ids: Vec<EntityId> = (0..AB_END)
            .map(|i| entities.add_entity((&mut vm_a, &mut vm_b), (A::new(i), B::new(i))))
            .collect();
        ids.extend((AB_END..ENTITIES).map(|_| entities.add_entity((), ())));

        ids
    };

    // Preparation: the eligible population is every `A` holder, logical 0..100,000.
    // We remove `A` from the first 10,000 of their `SEED` shuffle, in shuffled order,
    // then add the original values back in the same order.
    let mut churned = shuffled((0..AB_END).collect(), SEED);
    churned.truncate(CHURNED);
    {
        let (entities, mut vm_a) = world.borrow::<(EntitiesView, ViewMut<A>)>().unwrap();

        for &i in &churned {
            assert_eq!(vm_a.remove(ids[i as usize]), Some(A::new(i)));
        }
        for &i in &churned {
            entities.add_component(ids[i as usize], &mut vm_a, A::new(i));
        }
    }

    // Measurement. `invocations` counts the traversals, the checks need it.
    let mut invocations = 0;
    criterion.bench_function("iterate_ab_full/shipyard_current", |bencher| {
        bencher.iter_custom(|iters| {
            invocations += iters;

            let start = Instant::now();
            for _ in 0..iters {
                let (mut vm_a, v_b) = world.borrow::<(ViewMut<A>, View<B>)>().unwrap();
                (&mut vm_a, &v_b).iter().for_each(|(a, b)| {
                    a.x = a.x.wrapping_add(b.x);
                    a.y = a.y.wrapping_add(b.y);
                    a.z = a.z.wrapping_add(b.z);
                    a.w = a.w.wrapping_add(b.w);
                });
            }
            start.elapsed()
        })
    });

    // Checks: each traversal updated every `AB` entity exactly once, nothing else changed,
    // and the neither-payload entities are alive without payloads.
    let n = invocations;
    let (entities, v_a, v_b) = world.borrow::<(EntitiesView, View<A>, View<B>)>().unwrap();
    assert_eq!(entities.iter().count(), ENTITIES as usize);
    assert_eq!(v_a.len(), AB_END as usize);
    assert_eq!(v_b.len(), AB_END as usize);
    for (i, &id) in (0..AB_END).zip(&ids) {
        let b = B::new(i);
        let mut a = A::new(i);
        a.x = a.x.wrapping_add(n.wrapping_mul(b.x));
        a.y = a.y.wrapping_add(n.wrapping_mul(b.y));
        a.z = a.z.wrapping_add(n.wrapping_mul(b.z));
        a.w = a.w.wrapping_add(n.wrapping_mul(b.w));

        assert_eq!(v_a[id], a, "entity {i}");
        assert_eq!(v_b[id], b, "entity {i}");
    }
    for (i, &id) in (AB_END..ENTITIES).zip(&ids[AB_END as usize..]) {
        assert!(entities.is_alive(id), "entity {i}");
        assert!(!v_a.contains(id) && !v_b.contains(id), "entity {i}");
    }
}
