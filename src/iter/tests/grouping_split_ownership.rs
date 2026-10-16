//! Splitting the current segment and sharing the plan between the halves.
//!
//! This is the body of the parallel producer's `split`, tested without Rayon so Miri can run it.\
//! Plans are written by hand over real storages, like in `grouping_bulk` whose helpers we reuse.

use super::grouping_bulk::{mixed, planned, window, PAIR_LAYOUT};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::plan::{Coverage, CoverageSpan, NodeKind, NodeState};
use crate::iter::{IntoIter, Mixed, Shiper, Shiperator};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::vec::Vec;

#[derive(Debug, PartialEq)]
pub(super) struct A(pub(super) u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
pub(super) struct B(pub(super) u32);
impl Component for B {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct Modif(u32);
impl Component for Modif {
    type Tracking = track::Modification;
}

/// Returns the number of follow-ups left, `None` without plan.
fn follow_ups<S>(iter: &Shiper<S>) -> Option<usize> {
    iter.schedule
        .as_ref()
        .map(|schedule| schedule.follow_ups().len())
}

/// Splits `iter` and asserts how its candidates and follow-ups were shared.
///
/// With `n` candidates left in the current segment, the left half gets the first `floor(n / 2)`
/// and no follow-up, the right half gets the rest and every follow-up. Neither enters a segment.
fn split_halves<S: Shiperator + Clone>(iter: Shiper<S>) -> (Shiper<S>, Shiper<S>) {
    let len = iter.cursor.len();
    let local_index = iter.cursor.local_index();
    let follow_ups_before = follow_ups(&iter);

    let (left, right) = iter.split_current_segment();
    let right = right.expect("2 candidates or more split");

    assert_eq!(
        (left.cursor.local_index(), left.cursor.len()),
        (local_index, len / 2)
    );
    assert_eq!(
        (right.cursor.local_index(), right.cursor.len()),
        (local_index + len / 2, len - len / 2)
    );
    assert_eq!(follow_ups(&left), follow_ups_before.map(|_| 0));
    assert_eq!(follow_ups(&right), follow_ups_before);

    (left, right)
}

/// Asserts that `iter` doesn't split and comes back with its candidates and follow-ups untouched.
fn assert_no_split<S: Shiperator + Clone>(iter: Shiper<S>) -> Shiper<S> {
    let len = iter.cursor.len();
    let local_index = iter.cursor.local_index();
    let follow_ups_before = follow_ups(&iter);

    let (iter, right) = iter.split_current_segment();

    assert!(right.is_none());
    assert_eq!(
        (iter.cursor.local_index(), iter.cursor.len()),
        (local_index, len)
    );
    assert_eq!(follow_ups(&iter), follow_ups_before);

    iter
}

/// Splits `iter` until no piece can split, the pieces are pushed in traversal order.
pub(super) fn split_all<S: Shiperator + Clone>(iter: Shiper<S>, pieces: &mut Vec<Shiper<S>>) {
    match iter.split_current_segment() {
        (left, Some(right)) => {
            split_all(left, pieces);
            split_all(right, pieces);
        }
        (iter, None) => pieces.push(iter),
    }
}

/// `A` starts with an unrelated `A(100)`, then `e0` to `e7` own `A(i)` at index `i + 1`.
pub(super) fn window_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    world.add_entity(A(100));
    let e = (0..8).map(|i| world.add_entity(A(i))).collect();

    (world, e)
}

/// Single window iterator over `A` from index 1, cut in consecutive segments of `lens`.
///
/// No segment is entered yet.
pub(super) fn window_plan<'a>(a: &'a View<'_, A>, lens: &[usize]) -> Shiper<FullRawWindow<'a, A>> {
    let mut rows = Vec::new();
    let mut start = 1;
    for &len in lens {
        rows.push(([window(start)], start, len));
        start += len;
    }
    let segments: Vec<(&[EntityId], &[NodeState])> = rows
        .iter()
        .map(|(row, start, len)| (&a.dense[*start..*start + *len], &row[..]))
        .collect();

    planned(
        FullRawWindow::from_view(a),
        &[NodeKind::Window],
        &segments,
        &[],
    )
}

/// Same as [`window_plan`] with the first segment entered, the others are follow-ups.
pub(super) fn window_entered<'a>(
    a: &'a View<'_, A>,
    lens: &[usize],
) -> Shiper<FullRawWindow<'a, A>> {
    let mut iter = window_plan(a, lens);
    assert!(iter.enter_next_segment());

    iter
}

fn window_items<'a, S: Shiperator<Out = &'a A>>(iter: Shiper<S>) -> Vec<(EntityId, u32)> {
    iter.with_id().map(|(id, a)| (id, a.0)).collect()
}

/// Items `e{start}` to `e{end - 1}` of `window_world`.
pub(super) fn window_expected(e: &[EntityId], start: usize, end: usize) -> Vec<(EntityId, u32)> {
    (start..end).map(|i| (e[i], i as u32)).collect()
}

/// Asserts that a current segment without any candidate doesn't split, even with long
/// follow-ups, and that the follow-ups are still all walked afterwards.
#[test]
fn split_zero() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let iter = assert_no_split(window_plan(&a, &[3, 5]));

    assert_eq!(follow_ups(&iter), Some(2));
    assert_eq!(window_items(iter), window_expected(&e, 0, 8));
}

/// Asserts that an exhausted iterator without plan doesn't split.
#[test]
fn split_zero_ordinary() {
    let (world, _) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let mut iter = (&a).iter();
    iter.by_ref().for_each(drop);

    let iter = assert_no_split(iter);

    assert_eq!(iter.count(), 0);
}

/// Asserts that a single candidate doesn't split even with a 7 candidates follow-up,
/// and that nothing is entered or lost.
#[test]
fn split_one() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let iter = assert_no_split(window_entered(&a, &[1, 7]));

    assert_eq!(window_items(iter), window_expected(&e, 0, 8));
}

/// Asserts that 2 candidates split in two single candidate halves,
/// the right one keeping the follow-up.
#[test]
fn split_two() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = split_halves(window_entered(&a, &[2, 6]));

    assert_eq!(window_items(left), window_expected(&e, 0, 1));
    assert_eq!(window_items(right), window_expected(&e, 1, 8));
}

/// Asserts that 4 candidates split 2 and 2, the right half keeping the follow-up.
#[test]
fn split_even() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = split_halves(window_entered(&a, &[4, 4]));

    assert_eq!(window_items(left), window_expected(&e, 0, 2));
    assert_eq!(window_items(right), window_expected(&e, 2, 8));
}

/// Asserts that 5 candidates split 2 and 3, the right half keeping both follow-ups.
#[test]
fn split_odd() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = split_halves(window_entered(&a, &[5, 2, 1]));

    assert_eq!(window_items(left), window_expected(&e, 0, 2));
    assert_eq!(window_items(right), window_expected(&e, 2, 8));
}

/// Asserts that an iterator without plan splits without gaining one.
#[test]
fn split_ordinary() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = split_halves((&a).iter());

    // `A(100)` is part of an ordinary traversal.
    let left = window_items(left);
    assert_eq!(left[0].1, 100);
    assert_eq!(left[1..], window_expected(&e, 0, 3));
    assert_eq!(window_items(right), window_expected(&e, 3, 8));
}

/// Asserts that a plan's only segment splits with both halves keeping the plan
/// and neither getting a follow-up.
#[test]
fn split_zero_follow_ups() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = split_halves(window_entered(&a, &[7]));

    assert_eq!(follow_ups(&left), Some(0));
    assert_eq!(follow_ups(&right), Some(0));
    assert_eq!(window_items(left), window_expected(&e, 0, 3));
    assert_eq!(window_items(right), window_expected(&e, 3, 7));
}

/// Asserts that after a 2 items prefix in a segment starting at index 1, the right half
/// resolves its local index 4 to index 5 of `A`, `e4`, applying the window's start only once.
#[test]
fn split_resolves_start() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let mut iter = window_entered(&a, &[6, 2]);
    assert_eq!(iter.by_ref().take(2).count(), 2);

    let (_, right) = split_halves(iter);

    assert_eq!(right.shiperator.segment_start, 1);
    assert_eq!(
        right.with_id().next().map(|(id, a)| (id, a.0)),
        Some((e[4], 4))
    );
}

/// Asserts that recursively splitting after a prefix gives single candidate pieces whose
/// items, in order, are exactly the original candidates: no overlap and no gap.
#[test]
fn split_recursive() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let mut iter = window_entered(&a, &[7, 1]);
    assert_eq!(iter.by_ref().take(2).count(), 2);

    let mut pieces = Vec::new();
    split_all(iter, &mut pieces);

    assert_eq!(pieces.len(), 5);
    let (last, firsts) = pieces.split_last().unwrap();
    assert!(firsts.iter().all(|piece| follow_ups(piece) == Some(0)));
    assert_eq!(follow_ups(last), Some(1));

    let items: Vec<_> = pieces.into_iter().flat_map(window_items).collect();
    assert_eq!(items, window_expected(&e, 2, 8));
}

/// Asserts that splitting at index 0 leaves the left half without candidate or follow-up
/// and gives everything to the right half.
#[test]
fn split_at_start() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = window_entered(&a, &[4, 2]).split_current_segment_at(0);

    assert_eq!(follow_ups(&left), Some(0));
    assert!(window_items(left).is_empty());
    assert_eq!(window_items(right), window_expected(&e, 0, 6));
}

/// Asserts that splitting at the end of the current segment leaves the right half without
/// candidate in it, it only walks the follow-up.
#[test]
fn split_at_end() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = window_entered(&a, &[4, 2]).split_current_segment_at(4);

    assert_eq!((right.cursor.len(), follow_ups(&right)), (0, Some(1)));
    assert_eq!(window_items(left), window_expected(&e, 0, 4));
    assert_eq!(window_items(right), window_expected(&e, 4, 6));
}

/// Asserts that splitting at an index that isn't the middle, after a 2 items prefix,
/// keeps the original local indices: the right half starts at local index 3, `e3`.
#[test]
fn split_at_uneven() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let mut iter = window_entered(&a, &[7]);
    assert_eq!(iter.by_ref().take(2).count(), 2);

    let (left, right) = iter.split_current_segment_at(1);

    assert_eq!(right.cursor.local_index(), 3);
    assert_eq!(window_items(left), window_expected(&e, 2, 3));
    assert_eq!(window_items(right), window_expected(&e, 3, 7));
}

/// `e0` to `e5` own `A(i)` at index `i` of `A` and `B(10 + i)`.
///
/// `B` starts with two entities without `A`, then holds `e3`, `e0`, `e4`, `e1`, `e5`, `e2`.
pub(super) fn coverage_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..6).map(|i| world.add_entity(A(i))).collect();
    world.add_entity(B(100));
    world.add_entity(B(101));
    for i in [3, 0, 4, 1, 5, 2] {
        world.add_component(e[i], B(10 + i as u32));
    }

    (world, e)
}

/// `A` drives `e0` to `e2`, `B` is looked up.
const A_WALK: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b01,
        coverage: CoverageSpan::EMPTY,
    },
    NodeState::Window { segment_start: 0 },
    NodeState::Sailor,
];

/// `B` drives `e3`, `e0`, `e4`, `e1`, `e5`, `e2`. The region of `A` walked by `A_WALK` is covered,
/// only `e3`, `e4` and `e5` are accepted.
const B_WALK: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b10,
        coverage: CoverageSpan { start: 0, len: 1 },
    },
    NodeState::Sailor,
    NodeState::Window { segment_start: 2 },
];

/// `A`'s region `0..3`, walked by `A_WALK`.
const A_WALKED: [Coverage; 1] = [Coverage {
    child: 0,
    start: 0,
    end: 3,
}];

/// Iterator over `A_WALK`, `B_WALK` then `B`'s two entities without `A`, a segment where
/// every candidate is rejected. No segment is entered yet.
///
/// `windows` must be a pair over `A` and `B`, whose entities are `a_dense` and `b_dense`.
pub(super) fn coverage_plan<S: Shiperator>(
    windows: S,
    a_dense: &[EntityId],
    b_dense: &[EntityId],
) -> Shiper<S> {
    let no_match = [mixed(0b10), NodeState::Sailor, window(0)];
    let segments: [(&[EntityId], &[NodeState]); 3] = [
        (&a_dense[..3], &A_WALK),
        (&b_dense[2..], &B_WALK),
        (&b_dense[..2], &no_match),
    ];

    planned(windows, &PAIR_LAYOUT, &segments, &A_WALKED)
}

pub(super) fn pair<'a>(
    a: &'a View<'_, A>,
    b: &'a View<'_, B>,
) -> Mixed<(FullRawWindow<'a, A>, FullRawWindow<'a, B>)> {
    Mixed::new(
        (FullRawWindow::from_view(a), FullRawWindow::from_view(b)),
        0b01,
    )
}

pub(super) type PairIter<'a> = Shiper<Mixed<(FullRawWindow<'a, A>, FullRawWindow<'a, B>)>>;

fn pair_ids(iter: PairIter<'_>) -> Vec<EntityId> {
    iter.with_id()
        .map(|(id, (a, b))| {
            assert_eq!(a.0 + 10, b.0);
            id
        })
        .collect()
}

/// Splits `A_WALK` once entered: the left half keeps `e0`, the right half gets `e1`, `e2`
/// and every follow-up.
fn split_a_walk<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> (PairIter<'a>, PairIter<'a>) {
    let mut iter = coverage_plan(pair(a, b), &a.dense, &b.dense);
    assert!(iter.enter_next_segment());

    split_halves(iter)
}

/// Splits `B_WALK` after `e3`: the left half keeps `e0` and `e4`, the right half gets `e1`, `e5`,
/// `e2` and the segment without match. Both halves read the shared coverage.
fn split_b_walk<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> (PairIter<'a>, PairIter<'a>) {
    let mut iter = coverage_plan(pair(a, b), &a.dense, &b.dense);
    let walked: Vec<_> = iter.by_ref().take(4).map(|(a, _)| a.0).collect();
    assert_eq!(walked, [0, 1, 2, 3]);
    assert_eq!(iter.shiperator.coverage.len(), 1);

    split_halves(iter)
}

/// Asserts that the right half can enter `B_WALK` before the left half yields `e0`,
/// and still rejects it: coverage is about the original walk, not about what the left half
/// already did. Every entity is yielded once.
#[test]
fn right_first() {
    let (world, e) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (left, right) = split_a_walk(&a, &b);
    let plan = left.schedule.as_ref().unwrap().downgrade();

    assert_eq!(pair_ids(right), [e[1], e[2], e[3], e[4], e[5]]);
    assert!(plan.upgrade().is_some());

    assert_eq!(pair_ids(left), [e[0]]);
    assert!(plan.upgrade().is_none());
}

/// Asserts that consuming the left half first gives the same entities as the right half first.
#[test]
fn left_first() {
    let (world, e) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (left, right) = split_a_walk(&a, &b);
    let plan = left.schedule.as_ref().unwrap().downgrade();

    assert_eq!(pair_ids(left), [e[0]]);
    assert!(plan.upgrade().is_some());

    assert_eq!(pair_ids(right), [e[1], e[2], e[3], e[4], e[5]]);
    assert!(plan.upgrade().is_none());
}

/// Asserts that the right half keeps the plan and reads the current coverage once the left half
/// is dropped: it rejects `e1` and `e2`, yields `e5` and nothing in the segment without match.
#[test]
fn left_dropped() {
    let (world, e) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (left, right) = split_b_walk(&a, &b);
    let plan = left.schedule.as_ref().unwrap().downgrade();

    drop(left);

    assert_eq!(plan.strong_count(), 1);
    assert_eq!(pair_ids(right), [e[5]]);
    assert!(plan.upgrade().is_none());
}

/// Asserts that the left half keeps the plan without any follow-up and reads the current
/// coverage once the right half is dropped: it rejects `e0` and yields `e4`.
#[test]
fn right_dropped() {
    let (world, e) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (left, right) = split_b_walk(&a, &b);
    let plan = right.schedule.as_ref().unwrap().downgrade();

    drop(right);

    assert_eq!(plan.strong_count(), 1);
    assert_eq!(pair_ids(left), [e[4]]);
    assert!(plan.upgrade().is_none());
}

/// Asserts that a plan's only segment, with coverage, splits into halves that both keep the plan
/// and read its records: the right half outlives the left and only yields `e5`.
#[test]
fn coverage_zero_follow_ups() {
    let (world, e) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = planned(
        pair(&a, &b),
        &PAIR_LAYOUT,
        &[(&b.dense[2..], &B_WALK)],
        &A_WALKED,
    );
    assert!(iter.enter_next_segment());

    let (left, right) = split_halves(iter);
    let plan = left.schedule.as_ref().unwrap().downgrade();
    assert_eq!(plan.strong_count(), 2);

    assert_eq!(pair_ids(left), [e[3], e[4]]);
    assert_eq!(plan.strong_count(), 1);
    assert_eq!(pair_ids(right), [e[5]]);
}

/// Asserts that both halves can hold their mutable outputs at the same time: the right half
/// rejects `e0` while the left half's `&mut A` to it is alive, without creating a second one.
/// Every entity is then written exactly once.
#[test]
fn mutable_both_sides() {
    let (world, e) = coverage_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let a_dense = a.dense.clone();
    let b_dense = b.dense.clone();
    let windows = Mixed::new(
        (FullRawWindowMut::new(&mut a), FullRawWindowMut::new(&mut b)),
        0b01,
    );
    let mut iter = coverage_plan(windows, &a_dense, &b_dense);
    assert!(iter.enter_next_segment());

    let (left, right) = split_halves(iter);
    let plan = left.schedule.as_ref().unwrap().downgrade();

    let left_outputs: Vec<_> = left.with_id().collect();
    assert!(plan.upgrade().is_some());
    let right_outputs: Vec<_> = right.with_id().collect();

    for (_, (a, b)) in left_outputs.into_iter().chain(right_outputs) {
        a.0 += 100;
        b.0 += 100;
    }

    for (i, &id) in e.iter().enumerate() {
        assert_eq!((&a).get(id).unwrap(), &A(100 + i as u32));
        assert_eq!((&b).get(id).unwrap(), &B(110 + i as u32));
    }
}

/// Asserts that tracked outputs from both halves only flag what's written through them:
/// `e0` from the left half, `e1` and `e4` from the right half, `e4` being in its follow-up.
#[test]
fn tracked_mutation() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..6).map(|i| world.add_entity(Modif(i))).collect();
    let mut modif = world
        .borrow::<ViewMut<'_, Modif, track::Modification>>()
        .unwrap();
    let dense = modif.dense.clone();
    let segments: [(&[EntityId], &[NodeState]); 2] =
        [(&dense[..3], &[window(0)]), (&dense[3..], &[window(3)])];
    let mut iter = planned(
        FullRawWindowMut::new(&mut modif),
        &[NodeKind::Window],
        &segments,
        &[],
    );
    assert!(iter.enter_next_segment());

    let (left, right) = split_halves(iter);
    let outputs: Vec<_> = left.with_id().chain(right.with_id()).collect();
    assert_eq!(outputs.len(), 6);

    for (i, (id, mut modif)) in outputs.into_iter().enumerate() {
        assert_eq!(id, e[i]);

        if [0, 1, 4].contains(&i) {
            modif.0 += 100;
        } else {
            assert_eq!(modif.0, i as u32);
        }
    }

    for (i, &id) in e.iter().enumerate() {
        let is_written = [0, 1, 4].contains(&i);

        assert_eq!(modif.is_modified(id), is_written);
        assert_eq!((&modif).get(id).unwrap().0 >= 100, is_written);
    }
}
