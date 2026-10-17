//! Deterministic producer splitting on publicly constructed iterators.
//!
//! We call the same private split primitive as Rayon to choose the order of
//! consumption/drop and to run aliasing and lifetime checks under Miri without Rayon.
//! Worlds and iterators are built through the public API; no plans are injected.

use super::grouping_fixtures::{coverage_world, late_iter, PairIter, A, B};
use crate::iter::{IntoIter, Shiper, Shiperator};
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
        let mut world = World::new();
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
    let mut world = World::new();
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
    let mut world = World::new();
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
