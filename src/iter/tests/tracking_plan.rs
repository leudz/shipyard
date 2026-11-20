use super::grouping_fixtures::A;
use crate::{Component, Get, IntoIter, View, ViewMut, World, track};
use alloc::vec::Vec;

struct Tracked(usize);
impl Component for Tracked {
    type Tracking = track::All;
}
struct Other;
impl Component for Other {
    type Tracking = track::All;
}

#[test]
fn dense_candidates_still_filter_after_splitting() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..4096).map(|i| world.add_entity((Tracked(i),))).collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_modified();
    {
        let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        for &id in ids.iter().step_by(64) {
            (&mut tracked).get(id).unwrap().0 += 1;
        }
    }
    let tracked = world.borrow::<View<'_, Tracked>>().unwrap();
    let expected: Vec<_> = ids.iter().step_by(64).copied().collect();
    let mut iter = tracked.modified().iter();
    assert!(iter.dense_candidates);
    assert_eq!(iter.next().unwrap().0, 1);
    let (left, right) = iter.split_remaining();
    assert_eq!(
        left.ids().chain(right.unwrap().ids()).collect::<Vec<_>>(),
        expected[1..]
    );
    assert_eq!(
        tracked.modified().iter().ids().collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        tracked.modified().iter().fold(0, |n, _| n + 1),
        expected.len()
    );
}

#[test]
fn sparse_captain_and_balanced_split() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..4096)
        .map(|i| world.add_entity((A(i), Tracked(i as usize))))
        .collect();
    world.clear_all_inserted_and_modified();
    world.run(|_: View<'_, Tracked>| {});
    {
        let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        for i in [3904, 4095] {
            (&mut tracked).get(ids[i]).unwrap().0 += 1;
        }
    }
    let (a, tracked) = world.borrow::<(View<'_, A>, View<'_, Tracked>)>().unwrap();
    let iter = (&a, tracked.modified()).iter();
    assert_eq!(iter.shiperator.single_captain, 1);
    assert_eq!(iter.candidate_work(), Some(128));
    let (left, right) = iter.split_remaining();
    let right = right.unwrap();
    assert_eq!(left.candidate_work(), Some(64));
    assert_eq!(right.candidate_work(), Some(64));
    assert_eq!(
        left.ids().chain(right.ids()).collect::<Vec<_>>(),
        [ids[3904], ids[4095]]
    );
    let reversed: Vec<_> = ids.iter().rev().copied().collect();
    let forced = (&reversed[..], tracked.modified()).iter();
    assert_eq!(forced.shiperator.single_captain, 0);
    assert_eq!(forced.candidate_work(), Some(ids.len()));
    assert_eq!(forced.ids().collect::<Vec<_>>(), [ids[4095], ids[3904]]);
}

#[test]
fn forced_driver_bounds_planning_and_empty_join() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..4096)
        .map(|i| world.add_entity((A(i), Tracked(i as usize))))
        .collect();
    world.clear_all_inserted_and_modified();
    world.run(|_: View<'_, Tracked>| {});
    let (a, tracked) = world.borrow::<(View<'_, A>, View<'_, Tracked>)>().unwrap();
    // 64 chunks exceed the one-row driver's budget, so no empty proof is built.
    let iter = (&ids[..1], (&a, tracked.modified())).iter();
    assert_eq!(iter.size_hint().1, Some(1));
    assert_eq!(iter.count(), 0);
    // The larger forced driver allows the empty proof even though tracking is a sailor.
    let iter = (&ids[..512], tracked.modified()).iter();
    assert_eq!(iter.size_hint().1, Some(0));
    assert_eq!(iter.count(), 0);
    assert_eq!((!tracked.modified()).iter().count(), ids.len());
    assert_eq!(
        (&a, crate::iter::Optional(&tracked)).iter().count(),
        ids.len()
    );
}

#[test]
fn plans_follow_each_tracking_interval() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..130).map(|i| world.add_entity((Tracked(i),))).collect();
    world.clear_all_inserted_and_modified();
    world.run(|_: View<'_, Tracked>| {});
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        0
    );
    world.borrow::<ViewMut<'_, Tracked>>().unwrap()[ids[129]].0 += 1;
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .ids()
            .collect::<Vec<_>>(),
        [ids[129]]
    );
    world.clear_all_modified();
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        0
    );
}

#[test]
fn or_splits_weighted_follow_ups_and_omits_empty_branches() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..1300)
        .map(|i| world.add_entity((Tracked(i), Other)))
        .collect();
    world.clear_all_inserted_and_modified();
    world.run(|_: View<'_, Tracked>| {});
    {
        let mut other = world.borrow::<ViewMut<'_, Other>>().unwrap();
        for i in [65, 640, 1299] {
            *(&mut other).get(ids[i]).unwrap() = Other;
        }
    }
    let inserted: Vec<_> = (1300..2600)
        .map(|i| world.add_entity((Tracked(i),)))
        .collect();
    let (tracked, other) = world
        .borrow::<(View<'_, Tracked>, View<'_, Other>)>()
        .unwrap();
    let query = || (tracked.inserted() | other.modified()).iter();
    let expected = inserted
        .iter()
        .copied()
        .chain([ids[65], ids[640], ids[1299]])
        .collect::<Vec<_>>();
    let iter = query();
    assert!(iter.dense_candidates);
    assert_eq!(query().ids().collect::<Vec<_>>(), expected);
    let total = iter.candidate_work().unwrap();
    let (left, right) = iter.split_remaining();
    let right = right.unwrap();
    assert_eq!(left.candidate_work(), Some(total / 2));
    assert_eq!(right.candidate_work(), Some(total - total / 2));
    assert_eq!(left.ids().chain(right.ids()).collect::<Vec<_>>(), expected);
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        for threads in [1, 4, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let (tracked, other) = world
                        .borrow::<(View<'_, Tracked>, View<'_, Other>)>()
                        .unwrap();
                    let mut actual = (tracked.inserted() | other.modified())
                        .par_iter()
                        .with_id()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>();
                    let mut expected = expected.clone();
                    actual.sort_unstable();
                    expected.sort_unstable();
                    assert_eq!(actual, expected);
                });
        }
    }
    drop(tracked);
    drop(other);
    world.clear_all_inserted();
    let (tracked, other) = world
        .borrow::<(View<'_, Tracked>, View<'_, Other>)>()
        .unwrap();
    assert_eq!(
        (tracked.inserted() | other.modified())
            .iter()
            .ids()
            .collect::<Vec<_>>(),
        [ids[65], ids[640], ids[1299]]
    );
}

#[test]
fn candidate_bitmaps_preserve_single_and_disjoint_runs() {
    use crate::sparse_set::tracking_plan::TrackingPlan;

    let len = 130 * 64 - 7;
    for runs in [
        [0..4, 0..0],
        [62..66, 0..0],
        [127..130, 0..0],
        [2..5, 63..67],
    ] {
        let selected: Vec<_> = (0..len)
            .filter(|index| runs.iter().any(|run| run.contains(&(index / 64))))
            .collect();
        let plan = TrackingPlan::build(len, |chunk| runs.iter().any(|run| run.contains(&chunk)));
        for start in (0..len).step_by(31) {
            let before = selected.partition_point(|&index| index < start);
            assert_eq!(plan.count(0, start), before);
            assert_eq!(
                plan.next_in(start, len),
                selected.get(before).copied().unwrap_or(len)
            );
        }
        for count in (0..=selected.len()).step_by(17) {
            let cut = plan.split_at(0, len, count);
            assert_eq!(plan.count(0, cut), count);
            assert_eq!(plan.count(cut, len), selected.len() - count);
        }
    }
}
