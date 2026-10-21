//! Grouped walks making a single segment, installed without plan.
//!
//! The walk covers A's unevaluated region, which starts after its evaluated components:
//! A's window drives from 3 while B is looked up.\
//! Iterators come from the normal constructors, we check the state they installed then compare
//! their items with the entities we know own every component.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::plan::{CoverageSpan, NodeState, SegmentRow};
use crate::iter::{IntoIter, Shiper, Shiperator};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::vec::Vec;

#[derive(Debug, PartialEq)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct B(u32);
impl Component for B {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct E(u32);
impl Component for E {
    type Tracking = track::Untracked;
}

/// 3 A-only entities and 9 B-only entities `B(200 + i)` grouped as AB, then the first 4 B-only
/// entities gain `A(100 + i)`.
///
/// A holds the A-only entities evaluated as ungrouped at `0..3` and the pairs unevaluated at
/// `3..7`. B holds all its components evaluated as ungrouped.\
/// The pairs are a single walk over A's unevaluated region, costing 12 against 21 for A
/// captaining alone.
///
/// Returns the pairs.
fn offset_world() -> (World, Vec<EntityId>) {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    for i in 0..3 {
        world.add_entity(A(i));
    }
    let b_only: Vec<EntityId> = (0..9).map(|i| world.add_entity(B(200 + i))).collect();
    world.regroup();

    let pairs = b_only[..4].to_vec();
    for (i, &id) in (0..).zip(&pairs) {
        world.add_component(id, A(100 + i));
    }

    (world, pairs)
}

/// Returns the pairs in A's storage order, the order the walk visits them, with their values.
fn expected(a: &View<'_, A>, b: &View<'_, B>) -> Vec<(EntityId, u32, u32)> {
    a.dense[3..7]
        .iter()
        .map(|&id| (id, a.get(id).unwrap().0, b.get(id).unwrap().0))
        .collect()
}

/// Returns the ids `iter` yields through `next`, in order.
fn next_ids<S: Shiperator>(iter: Shiper<S>) -> Vec<EntityId> {
    let mut ids = Vec::new();
    // `for` calls `next` for each item, it doesn't go through `fold`.
    for (id, _) in iter.with_id() {
        ids.push(id);
    }

    ids
}

fn sorted(mut ids: Vec<EntityId>) -> Vec<EntityId> {
    ids.sort_unstable();
    ids
}

/// Asserts that the pairs' walk is installed without plan: A drives from the start of its
/// unevaluated region, B is looked up and the cursor walks A's pairs.
#[test]
fn offset_state() {
    let (world, pairs) = offset_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();

    assert!(iter.schedule.is_none());
    assert_eq!(
        SegmentRow::of(&iter.shiperator).states(),
        [
            NodeState::Mixed {
                captain_mask: 0b01,
                coverage: CoverageSpan::EMPTY
            },
            NodeState::Window { segment_start: 3 },
            NodeState::Sailor,
        ]
    );
    assert_eq!(
        iter.cursor.as_ptr().as_ptr().cast_const(),
        a.dense[3..].as_ptr()
    );
    assert_eq!((iter.cursor.local_index(), iter.cursor.len()), (0, 4));
    // B's lookups could reject, the size is only bounded.
    assert!(!iter.is_exact_sized);
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(sorted(a.dense[3..7].to_vec()), sorted(pairs));
}

/// Asserts that `next` yields every pair once, in A's storage order, with its own values,
/// the size hint's bound counting down to 0.
#[test]
fn offset_next() {
    let (world, _) = offset_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = (&a, &b).iter().with_id();

    let mut items = Vec::new();
    for left in (0..4).rev() {
        let (id, (a, b)) = iter.next().unwrap();
        items.push((id, a.0, b.0));
        assert_eq!(iter.size_hint(), (0, Some(left)));
    }

    assert!(iter.next().is_none());
    assert_eq!(items, expected(&a, &b));
}

/// Asserts that `fold` yields the same items as `next`, in the same order.
#[test]
fn offset_fold() {
    let (world, _) = offset_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let items = (&a, &b)
        .iter()
        .with_id()
        .fold(Vec::new(), |mut items, (id, (a, b))| {
            items.push((id, a.0, b.0));
            items
        });

    assert_eq!(items, expected(&a, &b));
}

/// Asserts that mutable outputs each write their own entity's components and leave the A-only
/// entities alone.
#[test]
fn offset_mut() {
    let (world, pairs) = offset_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();

    let iter = (&mut a, &mut b).iter();
    assert!(iter.schedule.is_none());
    iter.for_each(|(a, b)| {
        a.0 += 1000;
        b.0 += 1000;
    });

    for (i, &id) in (0..).zip(&pairs) {
        assert_eq!((&a).get(id).unwrap().0, 1100 + i);
        assert_eq!((&b).get(id).unwrap().0, 1200 + i);
    }
    for (i, &id) in (0..).zip(&a.dense[..3]) {
        assert_eq!((&a).get(id).unwrap().0, i);
    }
}

/// Asserts that a nested pair driving `((A, B), E)` is installed without plan: the root drives
/// through the pair, whose A window drives from its offset, and E is looked up.
#[test]
fn nested_offset() {
    let (mut world, pairs) = offset_world();
    for (i, &id) in (0..).zip(&pairs) {
        world.add_component(id, E(300 + i));
    }
    // Lone E components keep E from being the cheapest captain.
    world.add_entity(E(0));
    world.add_entity(E(1));

    let (a, b, e) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, E>)>()
        .unwrap();
    let iter = ((&a, &b), &e).iter();

    assert!(iter.schedule.is_none());
    assert_eq!(
        SegmentRow::of(&iter.shiperator).states(),
        [
            NodeState::Mixed {
                captain_mask: 0b01,
                coverage: CoverageSpan::EMPTY
            },
            NodeState::Mixed {
                captain_mask: 0b01,
                coverage: CoverageSpan::EMPTY
            },
            NodeState::Window { segment_start: 3 },
            NodeState::Sailor,
            NodeState::Sailor,
        ]
    );

    let items: Vec<_> = iter
        .with_id()
        .map(|(id, ((a, b), e))| (id, a.0, b.0, e.0))
        .collect();
    let expected: Vec<_> = expected(&a, &b)
        .into_iter()
        .map(|(id, a, b)| (id, a, b, e.get(id).unwrap().0))
        .collect();
    assert_eq!(items, expected);
}

/// Asserts that halves split after a partial `next` walk the rest of the segment with its offset,
/// whichever half is consumed first.
#[test]
fn split_orders() {
    let (world, _) = offset_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let ids = a.dense[3..7].to_vec();

    for left_first in [true, false] {
        let mut iter = (&a, &b).iter();
        iter.next().unwrap();
        let (left, right) = iter.split_current_segment();
        let right = right.unwrap();
        assert!(left.schedule.is_none() && right.schedule.is_none());

        let (left, right) = if left_first {
            let left = next_ids(left);
            (left, next_ids(right))
        } else {
            let right = next_ids(right);
            (next_ids(left), right)
        };

        assert_eq!(left, ids[1..2]);
        assert_eq!(right, ids[2..]);
    }
}

/// Asserts that dropping either split half leaves the other one walking its own part.
#[test]
fn split_drop() {
    let (world, _) = offset_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let ids = a.dense[3..7].to_vec();

    let (left, right) = (&a, &b).iter().split_current_segment();
    drop(left);
    assert_eq!(next_ids(right.unwrap()), ids[2..]);

    let (left, right) = (&a, &b).iter().split_current_segment();
    drop(right);
    assert_eq!(next_ids(left), ids[..2]);
}

/// Asserts that the mutable outputs of an item taken with `next` and of both split halves can be
/// held together, each one writing its own entity's components.
#[test]
fn split_held_mut() {
    let (world, pairs) = offset_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();

    let mut iter = (&mut a, &mut b).iter();
    let first = iter.next().unwrap();
    let (left, right) = iter.split_current_segment();
    let right: Vec<_> = right.unwrap().collect();
    let left: Vec<_> = left.collect();

    assert_eq!((left.len(), right.len()), (1, 2));
    for (a, b) in [first].into_iter().chain(left).chain(right) {
        a.0 += 1000;
        b.0 += 1000;
    }

    for (i, &id) in (0..).zip(&pairs) {
        assert_eq!((&a).get(id).unwrap().0, 1100 + i);
        assert_eq!((&b).get(id).unwrap().0, 1200 + i);
    }
}

/// Asserts that a parallel walk of the plan-free segment yields every pair once, with its own
/// values, and that its mutable outputs write their own entity's components.
#[cfg(feature = "parallel")]
#[test]
fn offset_parallel() {
    use rayon::iter::ParallelIterator;

    let (world, pairs) = offset_world();
    {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let mut items: Vec<_> = (&a, &b)
            .par_iter()
            .with_id()
            .map(|(id, (a, b))| (id, a.0, b.0))
            .collect();
        items.sort_unstable();

        let mut expected = expected(&a, &b);
        expected.sort_unstable();
        assert_eq!(items, expected);
    }

    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    (&mut a, &mut b).par_iter().for_each(|(a, b)| {
        a.0 += 1000;
        b.0 += 1000;
    });

    for (i, &id) in (0..).zip(&pairs) {
        assert_eq!((&a).get(id).unwrap().0, 1100 + i);
        assert_eq!((&b).get(id).unwrap().0, 1200 + i);
    }
}
