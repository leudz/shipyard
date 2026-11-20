use rayon::prelude::*;
use shipyard::{Component, Get, IntoIter, View, ViewMut, World};

#[derive(Component)]
#[track(All)]
struct Tracked(usize);
#[derive(Component)]
#[track(All)]
struct Other;

fn check(world: &World) {
    let (tracked, other) = world.borrow::<(View<Tracked>, View<Other>)>().unwrap();
    let all: Vec<_> = (&tracked).iter().ids().collect();
    let expected: Vec<_> = all
        .iter()
        .copied()
        .filter(|&id| tracked.is_modified(id))
        .collect();
    assert_eq!(
        tracked.modified().iter().ids().collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        tracked.modified().iter().fold(0, |n, _| n + 1),
        expected.len()
    );
    let mut parallel = tracked
        .modified()
        .par_iter()
        .with_id()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    let mut sorted = expected.clone();
    sorted.sort_unstable();
    parallel.sort_unstable();
    assert_eq!(parallel, sorted);
    let mut joined = (&other, tracked.modified())
        .iter()
        .ids()
        .collect::<Vec<_>>();
    let mut expected_join = expected
        .into_iter()
        .filter(|&id| other.contains(id))
        .collect::<Vec<_>>();
    joined.sort_unstable();
    expected_join.sort_unstable();
    assert_eq!(joined, expected_join);
    let inserted: Vec<_> = all
        .into_iter()
        .filter(|&id| tracked.is_inserted(id))
        .collect();
    assert_eq!(
        tracked.inserted().iter().ids().collect::<Vec<_>>(),
        inserted
    );
    let mut expected_or: Vec<_> = (&tracked)
        .iter()
        .ids()
        .filter(|&id| tracked.is_inserted(id))
        .chain((&other).iter().ids().filter(|&id| other.is_modified(id)))
        .collect();
    expected_or.sort_unstable();
    expected_or.dedup();
    let mut actual = (tracked.inserted() | other.modified())
        .par_iter()
        .with_id()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, expected_or);
}

#[test]
fn chunk_boundaries_regroup_and_remove() {
    for grouped in [false, true] {
        let mut world = if grouped {
            World::builder().with_group::<(Tracked, Other)>().build()
        } else {
            World::new()
        };
        let ids: Vec<_> = world
            .bulk_add_entity((0..4099).map(|i| (Tracked(i),)))
            .collect();
        world.clear_all_inserted_and_modified();
        world.run(|_: View<Tracked>| {});
        for &i in &[0, 63, 64, 65, 2048, 4096, 4098] {
            world.get::<&mut Tracked>(ids[i]).unwrap().0 += 1;
            world.add_component(ids[i], Other);
        }
        check(&world);
        world.regroup();
        check(&world);
        world.delete_entity(ids[0]);
        world.delete_entity(ids[2048]);
        check(&world);
        world.regroup();
        check(&world);
        world.clear_all_inserted_and_modified();
        world.run(|_: View<Tracked>| {});
        check(&world);
    }
}

#[test]
fn dense_whole_mark_and_late_writes() {
    let mut world = World::new();
    let ids: Vec<_> = world
        .bulk_add_entity((0..4099).map(|i| (Tracked(i), Other)))
        .collect();
    world.clear_all_inserted_and_modified();
    world.run(|_: View<Tracked>| {});
    {
        let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
        let values: Vec<_> = (&mut tracked).iter().take(65).collect();
        values.into_par_iter().for_each(|mut t| t.0 += 1);
    }
    check(&world);
    assert_eq!(
        world
            .borrow::<View<Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        65
    );
    world.clear_all_modified();
    world.run(|_: View<Tracked>| {});
    {
        let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
        (&mut tracked).iter().for_each(|mut t| t.0 += 1);
    }
    check(&world);
    world.clear_all_modified();
    world.run(|_: View<Tracked>| {});
    world.get::<&mut Tracked>(ids[4098]).unwrap().0 += 1;
    for i in 0..130 {
        world.add_entity(Tracked(i));
    }
    check(&world);
}

#[test]
fn dense_and_sparse_filtered_mutable_iteration() {
    for step in [1, 64, 1024] {
        let mut world = World::new();
        let ids: Vec<_> = world
            .bulk_add_entity((0..4099).map(|i| (Tracked(i), Other)))
            .collect();
        world.clear_all_inserted_and_modified();
        world.run(|_: View<Tracked>| {});
        for &id in ids.iter().step_by(step) {
            world.get::<&mut Tracked>(id).unwrap().0 += 1;
        }
        check(&world);
        let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
        tracked.modified_mut().par_iter().for_each(|mut t| t.0 += 1);
        drop(tracked);
        check(&world);
    }
}

#[test]
fn forced_small_driver_negated_and_empty_filters() {
    let mut world = World::new();
    let ids: Vec<_> = world
        .bulk_add_entity((0..4099).map(|i| (Tracked(i), Other)))
        .collect();
    world.clear_all_inserted_and_modified();
    world.run(|_: View<Tracked>| {});
    let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
    assert_eq!((&ids[..1], tracked.modified()).iter().count(), 0);
    assert_eq!((!tracked.modified()).iter().count(), ids.len());
    (&mut tracked).get(ids[4098]).unwrap().0 += 1;
    let reversed: Vec<_> = ids.iter().rev().copied().collect();
    assert_eq!(
        (&reversed[..], tracked.modified())
            .iter()
            .ids()
            .collect::<Vec<_>>(),
        [ids[4098]]
    );
}

#[test]
fn captain_becoming_sailor_keeps_late_writes() {
    for mode in 0..3 {
        let mut world = World::builder().with_group::<(Tracked, Other)>().build();
        world.bulk_add_entity((0..1024).map(|i| (Tracked(i), Other)));
        world.bulk_add_entity((0..8192).map(|i| (Tracked(i),)));
        let others: Vec<_> = world.bulk_add_entity((0..2048).map(|_| (Other,))).collect();
        world.regroup();
        for (i, &id) in others.iter().take(128).enumerate() {
            world.add_component(id, Tracked(i));
        }
        world
            .borrow::<ViewMut<Tracked>>()
            .unwrap()
            .clear_all_modified();
        {
            let (mut tracked, other) = world.borrow::<(ViewMut<Tracked>, View<Other>)>().unwrap();
            match mode {
                0 => (&mut tracked, &other)
                    .iter()
                    .for_each(|(mut t, _)| t.0 += 1),
                1 => (&mut tracked, &other)
                    .par_iter()
                    .for_each(|(mut t, _)| t.0 += 1),
                _ => {
                    let items: Vec<_> = (&mut tracked, &other).iter().collect();
                    items.into_par_iter().for_each(|(mut t, _)| t.0 += 1);
                }
            }
        }
        check(&world);
        assert_eq!(
            world
                .borrow::<View<Tracked>>()
                .unwrap()
                .modified()
                .iter()
                .count(),
            1152
        );
    }
}
