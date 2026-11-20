use super::grouping_fixtures::A;
use crate::iter::{IntoIter, Shiperator};
use crate::{Component, EntityId, Get, View, ViewMut, World, track};
use alloc::vec::Vec;

#[derive(Clone)]
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::All;
}

/// Compare optimized traversal against per-entity timestamp checks.
fn check(world: &World) {
    let tracked = world.borrow::<View<'_, Tracked>>().unwrap();
    let ids: Vec<_> = (&tracked).iter().ids().collect();
    let inserted: Vec<_> = ids
        .iter()
        .copied()
        .filter(|&id| tracked.is_inserted(id))
        .collect();
    let modified: Vec<_> = ids
        .iter()
        .copied()
        .filter(|&id| tracked.is_modified(id))
        .collect();
    let either: Vec<_> = ids
        .iter()
        .copied()
        .filter(|&id| tracked.is_inserted_or_modified(id))
        .collect();
    assert_eq!(
        tracked.inserted().iter().ids().collect::<Vec<_>>(),
        inserted
    );
    assert_eq!(
        tracked.modified().iter().ids().collect::<Vec<_>>(),
        modified
    );
    assert_eq!(
        tracked
            .inserted_or_modified()
            .iter()
            .ids()
            .collect::<Vec<_>>(),
        either
    );
    assert_eq!(
        tracked
            .modified()
            .iter()
            .with_id()
            .fold(Vec::new(), |mut ids, (id, _)| {
                ids.push(id);
                ids
            }),
        modified
    );
    #[cfg(feature = "parallel")]
    {
        use rayon::iter::ParallelIterator;
        let mut parallel = tracked
            .modified()
            .par_iter()
            .with_id()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        let mut expected = modified.clone();
        parallel.sort_unstable();
        expected.sort_unstable();
        assert_eq!(parallel, expected);
    }
    drop(tracked);
    let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
    assert_eq!(
        tracked.modified_mut().iter().ids().collect::<Vec<_>>(),
        modified
    );
}

#[test]
fn skips_empty_chunks_within_split_bounds() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..259).map(|i| world.add_entity((Tracked(i),))).collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    world.borrow::<ViewMut<'_, Tracked>>().unwrap()[ids[193]].0 += 1;
    let tracked = world.borrow::<View<'_, Tracked>>().unwrap();
    let iter = tracked.modified().iter();
    // SAFE: All tested ranges belong to this iterator's only segment.
    unsafe {
        assert_eq!(iter.shiperator.next_candidate_range(0, 259), 192..256);
        assert_eq!(iter.shiperator.next_candidate_range(65, 130), 130..130);
        assert_eq!(iter.shiperator.next_candidate_range(257, 259), 259..259);
    }
    let (left, right) = iter.split_current_segment_at(193);
    assert_eq!(left.count(), 0);
    assert_eq!(
        right.with_id().map(|(id, _)| id).collect::<Vec<_>>(),
        [ids[193]]
    );
    let mut iter = tracked.modified().iter();
    assert_eq!(iter.next().unwrap().0, 194);
    let (left, right) = iter.split_current_segment_at(20);
    assert_eq!(left.count(), 0);
    assert_eq!(right.count(), 0);
    drop(tracked);
    check(&world);
}

#[test]
fn regroup_and_removal_preserve_tracking() {
    let mut world = World::builder_for_test()
        .with_group::<(A, Tracked)>()
        .build();
    let ids: Vec<_> = (0..259).map(|i| world.add_entity((Tracked(i),))).collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    world.borrow::<ViewMut<'_, Tracked>>().unwrap()[ids[258]].0 += 1;
    for (i, &id) in ids.iter().enumerate().skip(128) {
        world.add_component(id, A(i as u32));
    }
    for i in 259..263 {
        world.add_entity((Tracked(i), A(i)));
    }
    check(&world);
    world.regroup();
    check(&world);
    world.delete_entity(ids[130]);
    world.delete_entity(ids[0]);
    check(&world);
    world.regroup();
    check(&world);
}

#[test]
fn mutation_entry_points_update_summaries() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..513).map(|i| world.add_entity((Tracked(i),))).collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    world.get::<&mut Tracked>(ids[0]).unwrap().0 += 1;
    {
        let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        tracked[ids[64]].0 += 1;
        (&mut tracked).get(ids[128]).unwrap().as_mut().0 += 1;
        tracked.apply(ids[192], ids[193], |a, _| a.0 += 1);
        tracked.apply_mut(ids[256], ids[320], |a, b| {
            a.0 += 1;
            b.0 += 1;
        });
        tracked.retain_mut(|id, mut value| {
            if id == ids[384] {
                value.0 += 1;
            }
            true
        });
        for (id, mut value) in (&mut tracked).iter().with_id() {
            if id == ids[448] {
                value.0 += 1;
            }
        }
    }
    check(&world);
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        8
    );
}

#[test]
fn sorting_and_bulk_insertion_preserve_tracking() {
    let mut world = World::new_for_test();
    let ids: Vec<EntityId> = world
        .bulk_add_entity((0..259).map(|i| (Tracked(i),)))
        .collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    world.borrow::<ViewMut<'_, Tracked>>().unwrap()[ids[258]].0 += 1;
    world.bulk_add_entity((259..323).map(|i| (Tracked(i),)));
    check(&world);
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .sort_unstable_by(|a, b| b.0.cmp(&a.0));
    check(&world);
    #[cfg(feature = "std")]
    {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut calls = 0;
            world
                .borrow::<ViewMut<'_, Tracked>>()
                .unwrap()
                .sort_unstable_by(|a, b| {
                    calls += 1;
                    assert!(calls < 100, "comparator panic");
                    a.0.cmp(&b.0)
                });
        }));
        check(&world);
    }
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_writes_share_chunk_summaries() {
    use rayon::iter::ParallelIterator;
    let mut world = World::new_for_test();
    world.bulk_add_entity((0..4099).map(|i| (Tracked(i),)));
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    {
        let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        (&mut tracked).par_iter().for_each(|mut t| {
            if t.0 % 7 == 0 {
                t.0 += 1;
            }
        });
    }
    check(&world);
}

#[test]
fn or_and_nested_captains_skip_without_losing_ids() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..259)
        .map(|i| world.add_entity((Tracked(i), A(i))))
        .collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    world.borrow::<ViewMut<'_, Tracked>>().unwrap()[ids[193]].0 += 1;
    let inserted = world.add_entity((Tracked(259), A(259)));
    for i in 260..1040 {
        world.add_entity((A(i),));
    }
    let (a, tracked) = world.borrow::<(View<'_, A>, View<'_, Tracked>)>().unwrap();
    let query = || (tracked.inserted() | tracked.modified()).iter();
    assert_eq!(query().ids().collect::<Vec<_>>(), [inserted, ids[193]]);
    assert_eq!(
        query().with_id().fold(Vec::new(), |mut ids, (id, _)| {
            ids.push(id);
            ids
        }),
        [inserted, ids[193]]
    );
    assert_eq!(
        (&a, (tracked.modified(),)).iter().ids().collect::<Vec<_>>(),
        [ids[193]]
    );
}

#[test]
fn summary_newer_than_query_still_checks_older_matches() {
    use crate::tracking::TrackingTimestamp;
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..65).map(|i| world.add_entity((Tracked(i),))).collect();
    {
        let tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        tracked.sparse_set.modification_data[1] = TrackingTimestamp::new(2);
        tracked.sparse_set.modification_data[2] = TrackingTimestamp::new(5);
        tracked.sparse_set.refresh_tracking_chunks(0..65);
    }
    let mut tracked = world.borrow::<View<'_, Tracked>>().unwrap();
    tracked.last_modification = TrackingTimestamp::new(1);
    tracked.current = TrackingTimestamp::new(3);
    assert_eq!(
        tracked.modified().iter().ids().collect::<Vec<_>>(),
        [ids[1]]
    );
}

#[test]
fn get_respects_dynamic_tracking_and_storage_replacement() {
    let mut world = World::new_for_test();
    let id = world.add_entity((A(0),));
    {
        let mut values = world.borrow::<ViewMut<'_, A>>().unwrap();
        (&mut values).get(id).unwrap().0 += 1;
        assert!(values.modification_data.is_empty());
        values.track_modification();
        assert_eq!((&mut values).get(id).unwrap().0, 1);
        assert_eq!(values.modification_data[0].get(), 0);
        (&mut values).get(id).unwrap().0 += 1;
    }
    {
        let values = world.borrow::<View<'_, A, track::Modification>>().unwrap();
        assert_eq!(values.modified().iter().ids().collect::<Vec<_>>(), [id]);
    }

    let mut values = world
        .borrow::<ViewMut<'_, A, track::Modification>>()
        .unwrap();
    *values.sparse_set = crate::sparse_set::SparseSet::new();
    let current = values.current;
    let _ = values.sparse_set.insert(id, A(5), current);
    (&mut values).get(id).unwrap().0 += 1;
    assert_eq!(values.get(id).unwrap().0, 6);
    assert!(values.modification_data.is_empty());
}
