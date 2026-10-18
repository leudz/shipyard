//! Deterministic producer splitting on publicly constructed iterators.
//!
//! We call the same private split primitive as Rayon to choose the order of
//! consumption/drop and to run aliasing and lifetime checks under Miri without Rayon.
//! Worlds and iterators are built through the public API; no plans are injected.

use super::grouping_fixtures::{
    coverage_world, late_iter, ring_world, trust_world, PairIter, A, B, C,
};
use super::grouping_regions::grouped;
use crate::iter::{IntoIter, IntoShiperator, Shiper, Shiperator};
use crate::or::OneOfTwo;
use crate::{track, Component, EntityId, Get, View, ViewMut, World};
use alloc::vec::Vec;

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
fn split_all<S: Shiperator + Clone>(iter: Shiper<S>, pieces: &mut Vec<Shiper<S>>) {
    match iter.split_current_segment() {
        (left, Some(right)) => {
            split_all(left, pieces);
            split_all(right, pieces);
        }
        (iter, None) => pieces.push(iter),
    }
}

fn sorted(mut ids: Vec<EntityId>) -> Vec<EntityId> {
    ids.sort_unstable();
    ids
}

fn or_value(item: OneOfTwo<&A, &B>) -> u32 {
    match item {
        OneOfTwo::One(a) => a.0,
        OneOfTwo::Two(b) => b.0,
    }
}

/// Even a long real Or follow-up cannot make a zero/one-candidate segment split.
#[test]
fn segment_lengths() {
    for len in [0, 1, 2, 4, 5] {
        let mut world = World::new_for_test();
        for i in 0..len.max(1) {
            world.add_entity(A(i));
        }
        for i in 100..108 {
            world.add_entity(B(i));
        }
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let mut iter = (&a | &b).iter();
        if len == 0 {
            assert!(iter.next().is_some());
        }
        assert_eq!(iter.cursor.len(), len as usize);
        assert_eq!(follow_ups(&iter), Some(1));
        if len < 2 {
            let values: Vec<_> = assert_no_split(iter).map(or_value).collect();
            assert_eq!(values, (0..len).chain(100..108).collect::<Vec<_>>());
        } else {
            let (left, right) = split_halves(iter);
            assert_eq!(
                left.map(or_value).collect::<Vec<_>>(),
                (0..len / 2).collect::<Vec<_>>()
            );
            assert_eq!(
                right.map(or_value).collect::<Vec<_>>(),
                (len / 2..len).chain(100..108).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn ordinary_and_exact_boundaries() {
    let mut world = World::new_for_test();
    for i in 0..7 {
        world.add_entity(A(i));
    }
    let a = world.borrow::<View<'_, A>>().unwrap();
    let (left, right) = split_halves(a.iter());
    assert!(left.schedule.is_none() && right.schedule.is_none());
    assert_eq!(left.count(), 3);
    assert_eq!(right.count(), 4);
    let mut empty = a.iter();
    empty.by_ref().for_each(drop);
    assert_eq!(assert_no_split(empty).count(), 0);
    for index in [0, 1, 5] {
        let mut iter = a.iter();
        iter.by_ref().take(2).for_each(drop);
        let (left, right) = iter.split_current_segment_at(index);
        assert_eq!(right.cursor.local_index(), 2 + index);
        assert_eq!(
            left.map(|a| a.0).collect::<Vec<_>>(),
            (2..2 + index as u32).collect::<Vec<_>>()
        );
        assert_eq!(
            right.map(|a| a.0).collect::<Vec<_>>(),
            (2 + index as u32..7).collect::<Vec<_>>()
        );
    }
}

#[test]
fn recursive_and_boundary_splits() {
    let (world, ids) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = late_iter(&a, &b);
    assert_eq!(iter.next().unwrap().0 .0, 20);
    let mut pieces = Vec::new();
    split_all(iter, &mut pieces);
    assert_eq!(pieces.len(), 5);
    assert_eq!(follow_ups(&pieces[0]), Some(0));
    assert!(pieces[..4].iter().all(|piece| follow_ups(piece) == Some(0)));
    assert_eq!(follow_ups(&pieces[4]), Some(1));
    // The late A segment starts at 23 (after three evaluated ungrouped A's).
    assert_eq!(pieces[1].shiperator.shiperator.0.segment_start, 23);
    assert_eq!(pieces[1].cursor.local_index(), 2);
    assert_eq!(
        pieces.into_iter().flat_map(pair_ids).collect::<Vec<_>>(),
        ids[21..]
    );

    for index in [0, 1, 6] {
        let (left, right) = late_iter(&a, &b).split_current_segment_at(index);
        assert_eq!(follow_ups(&left), Some(0));
        assert_eq!(follow_ups(&right), Some(1));
        assert_eq!(pair_ids(left), ids[20..20 + index.min(3)]);
        assert_eq!(pair_ids(right), ids[20 + index.min(3)..]);
    }
}

fn pair_ids(iter: PairIter<'_>) -> Vec<EntityId> {
    iter.with_id()
        .map(|(id, (a, b))| {
            assert_eq!(a.0 + 10, b.0);
            id
        })
        .collect()
}

/// The final B walk interleaves covered and fresh candidates. Stop after its
/// first match so both halves must still read coverage, with no follow-ups left.
fn covered_split<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> (PairIter<'a>, PairIter<'a>) {
    let mut iter = late_iter(a, b);
    assert_eq!(
        iter.by_ref().take(4).map(|(a, _)| a.0).collect::<Vec<_>>(),
        [20, 21, 22, 23]
    );
    assert_eq!(iter.shiperator.coverage.len(), 1);
    assert_eq!(follow_ups(&iter), Some(0));
    split_halves(iter)
}

#[test]
fn consumption_order() {
    let (world, ids) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    for right_first in [false, true] {
        let (left, right) = split_halves(late_iter(&a, &b));
        let plan = left.schedule.as_ref().unwrap().downgrade();
        let (first, second, expected_first, expected_second) = if right_first {
            (right, left, &ids[23..], &ids[20..23])
        } else {
            (left, right, &ids[20..23], &ids[23..])
        };
        assert_eq!(pair_ids(first), expected_first);
        assert_eq!(plan.strong_count(), 1);
        assert_eq!(pair_ids(second), expected_second);
        assert!(plan.upgrade().is_none());
    }
}

#[test]
fn coverage_outlives_either_half() {
    let (world, ids) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    for drop_left in [false, true] {
        let (left, right) = covered_split(&a, &b);
        let plan = left.schedule.as_ref().unwrap().downgrade();
        assert_eq!(plan.strong_count(), 2);
        let (discarded, survivor, expected) = if drop_left {
            (left, right, ids[25])
        } else {
            (right, left, ids[24])
        };
        drop(discarded);
        assert_eq!(plan.strong_count(), 1);
        assert_eq!(pair_ids(survivor), [expected]);
        assert!(plan.upgrade().is_none());
    }
}

#[test]
fn mutable_both_sides() {
    let (world, ids) = coverage_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let mut iter = (&mut a, &mut b).iter();
    // Keep every earlier mutable output alive while later coverage rejects it.
    let prefix: Vec<_> = iter.by_ref().take(20).collect();
    assert!(iter.enter_next_segment());
    let (left, right) = split_halves(iter);
    let left_outputs: Vec<_> = left.collect();
    let right_outputs: Vec<_> = right.collect();
    for (a, b) in prefix.into_iter().chain(left_outputs).chain(right_outputs) {
        a.0 += 100;
        b.0 += 100;
    }
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!((&a).get(id).unwrap().0, 100 + i as u32);
        assert_eq!((&b).get(id).unwrap().0, 110 + i as u32);
    }
}

struct Modif(u32);
impl Component for Modif {
    type Tracking = track::Modification;
}

#[test]
fn tracked_mutation() {
    let mut world = World::new_for_test();
    let mut ids: Vec<_> = (0..10)
        .map(|i| world.add_entity((Modif(i), A(i))))
        .collect();
    world.group::<(Modif, A)>();
    ids.push(world.add_entity((Modif(10), A(10))));
    let (mut m, a) = world.borrow::<(ViewMut<'_, Modif>, View<'_, A>)>().unwrap();
    let iter = (&mut m, &a).iter();
    assert_eq!(iter.schedule.as_ref().unwrap().follow_ups().len(), 2);
    let (left, right) = split_halves(iter);
    let outputs: Vec<_> = left.chain(right).collect();
    assert_eq!(outputs.len(), ids.len());
    for (mut m, _) in outputs {
        if [0, 6, 10].contains(&m.0) {
            m.0 += 100;
        }
    }
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(m.is_modified(id), [0, 6, 10].contains(&i));
        assert_eq!((&m).get(id).unwrap().0 >= 100, [0, 6, 10].contains(&i));
    }
}

/// Splits `iter` inside each of its segments until no piece can split.
///
/// The current segment is peeled off and split, then the follow-ups are entered one by one and
/// split the same way. Pieces are returned in traversal order, each holds at most one candidate
/// and none of them has follow-ups.
fn split_every_segment<S: Shiperator + Clone>(iter: Shiper<S>) -> Vec<Shiper<S>> {
    let mut pieces = Vec::new();
    let mut iter = iter;

    loop {
        let len = iter.cursor.len();
        let (current, mut rest) = iter.split_current_segment_at(len);
        split_all(current, &mut pieces);

        if !rest.enter_next_segment() {
            return pieces;
        }
        iter = rest;
    }
}

/// Returns the ids `iter` yields, asserting each item holds a `ring_world` pair's components.
fn ring_ids(iter: PairIter<'_>) -> Vec<EntityId> {
    iter.with_id()
        .map(|(id, (a, b))| {
            assert_eq!(a.0 + 100, b.0);
            id
        })
        .collect()
}

/// Asserts that splitting `ring_world`'s pair walks inside every segment, the ABC pieces between
/// wrap points and the covered B walk included, gives pieces that yield every pair once
/// whichever order they're consumed in.
#[test]
fn ring_pieces_order() {
    let (world, pairs) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let expected = sorted(pairs.clone());

    let pieces = split_every_segment((&a, &b).iter());
    assert!(pieces.iter().all(|piece| piece.cursor.len() <= 1));
    let in_order: Vec<EntityId> = pieces.into_iter().flat_map(ring_ids).collect();
    assert_eq!(sorted(in_order), expected);

    let pieces = split_every_segment((&a, &b).iter());
    let reversed: Vec<EntityId> = pieces.into_iter().rev().flat_map(ring_ids).collect();
    assert_eq!(sorted(reversed), expected);
}

/// Asserts that dropping every other piece of `ring_world`'s pair walks first leaves the others
/// yielding exactly what they yield on their own: coverage they read outlives the dropped pieces.
#[test]
fn ring_pieces_dropped() {
    let (world, _) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let alone: Vec<Vec<EntityId>> = split_every_segment((&a, &b).iter())
        .into_iter()
        .map(ring_ids)
        .collect();

    for kept in [0, 1] {
        let mut pieces = split_every_segment((&a, &b).iter());
        let plan = pieces[0].schedule.as_ref().unwrap().downgrade();

        let mut index = 0;
        pieces.retain(|_| {
            index += 1;
            (index - 1) % 2 == kept
        });
        let survivors: Vec<Vec<EntityId>> = pieces.into_iter().map(ring_ids).collect();

        let expected: Vec<Vec<EntityId>> = alone.iter().skip(kept).step_by(2).cloned().collect();
        assert_eq!(survivors, expected, "kept {kept}");
        assert!(plan.upgrade().is_none());
    }
}

/// Asserts that mutable outputs of every piece of `ring_world`'s pair walks can be held all
/// at once: each pair is written exactly once and nothing else is.
#[test]
fn ring_pieces_mut() {
    let (world, pairs) = ring_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let a_len = a.len();

    let outputs: Vec<(&mut A, &mut B)> = split_every_segment((&mut a, &mut b).iter())
        .into_iter()
        .rev()
        .flatten()
        .collect();
    assert_eq!(outputs.len(), pairs.len());
    for (a, b) in outputs {
        a.0 += 1000;
        b.0 += 1000;
    }

    for (i, &id) in (0..).zip(&pairs) {
        assert_eq!((&a).get(id).unwrap().0, 1000 + i);
        assert_eq!((&b).get(id).unwrap().0, 1100 + i);
    }
    let written = a.iter().filter(|a| a.0 >= 1000).count();
    assert_eq!((written, a_len), (pairs.len(), pairs.len() + 7));
}

/// Asserts that mutable outputs of every piece of `trust_world`'s walks, where the damaged walk
/// has two captains and B's unevaluated walk rejects what it owns, can be held all at once:
/// each match is written exactly once.
#[test]
fn trust_pieces_mut() {
    let (world, e) = trust_world();
    let (mut a, mut b, mut c) = world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
        .unwrap();

    let iter = grouped((&mut a, &mut b, &mut c).into_shiperator());
    let outputs: Vec<(&mut A, &mut B, &mut C)> =
        split_every_segment(iter).into_iter().flatten().collect();
    assert_eq!(outputs.len(), e.len());
    for (a, b, c) in outputs {
        a.0 += 1000;
        b.0 += 1000;
        c.0 += 1000;
    }

    for (i, &id) in (0..).zip(&e) {
        assert_eq!((&a).get(id).unwrap().0, 1000 + i);
        assert_eq!((&b).get(id).unwrap().0, 1100 + i);
        assert_eq!((&c).get(id).unwrap().0, 1200 + i);
    }
}
