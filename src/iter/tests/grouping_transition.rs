//! Segment entry and `Or` transitions, checked on the iterator's internal state.
//!
//! Output checks alone could pass with skipped transitions, so these tests also assert
//! the state each node holds after entering a segment.

use crate::component::Component;
use crate::iter::plan::{CoverageSpan, NodeState, OrBranch};
use crate::iter::{IntoIter, IntoShiperator};
use crate::or::OneOfTwo;
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
struct C(u32);
impl Component for C {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct Ins(u32);
impl Component for Ins {
    type Tracking = track::Insertion;
}

fn window(segment_start: usize) -> NodeState {
    NodeState::Window { segment_start }
}

fn mixed(captain_mask: u32) -> NodeState {
    NodeState::Mixed {
        captain_mask,
        coverage: CoverageSpan::EMPTY,
    }
}

fn or(branch: OrBranch) -> NodeState {
    NodeState::Or { branch }
}

/// Entities `e0` to `e{a_len - 1}` own `A(i)`, then `f0` to `f{b_len - 1}` own `B(10 + i)`.
fn or_world(a_len: u32, b_len: u32) -> World {
    let mut world = World::new_for_test();

    for i in 0..a_len {
        world.add_entity(A(i));
    }
    for i in 0..b_len {
        world.add_entity(B(10 + i));
    }

    world
}

fn or_value(item: OneOfTwo<&A, &B>) -> u32 {
    match item {
        OneOfTwo::One(a) => a.0,
        OneOfTwo::Two(b) => b.0,
    }
}

/// Asserts that an `Or` whose branches both have candidates starts with the first one
/// and queues the second one, with its branch installed by the follow-up's row.
#[test]
fn or_both() {
    let world = or_world(2, 1);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (or_window, len, entities) = (&a | &b).into_shiperator();

    assert_eq!(or_window.branch, OrBranch::A);
    assert_eq!(len, 2);

    let schedule = entities.schedule.unwrap();
    let follow_ups = schedule.follow_ups();
    assert_eq!(follow_ups.len(), 1);
    assert_eq!(follow_ups[0].len(), 1);
    assert_eq!(
        schedule.plan().row(&follow_ups[0]),
        [or(OrBranch::B), NodeState::Sailor, window(0)]
    );
}

/// Asserts that an `Or` whose second branch has no candidate doesn't queue it.
#[test]
fn or_first_only() {
    let world = or_world(2, 0);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (or_window, len, entities) = (&a | &b).into_shiperator();

    assert_eq!(or_window.branch, OrBranch::A);
    assert_eq!(len, 2);
    assert!(entities.schedule.is_none());
}

/// Asserts that an `Or` whose first branch has no candidate starts on the second one,
/// with its branch already installed and nothing queued.
#[test]
fn or_second_only() {
    let world = or_world(0, 3);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (or_window, len, entities) = (&a | &b).into_shiperator();

    assert_eq!(or_window.branch, OrBranch::B);
    assert_eq!(len, 3);
    assert!(entities.schedule.is_none());
}

/// Asserts that an `Or` without any candidate has an exhausted cursor and nothing queued.
#[test]
fn or_none() {
    let world = or_world(0, 0);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (_, len, entities) = (&a | &b).into_shiperator();

    assert_eq!(len, 0);
    assert!(entities.cursor.is_empty());
    assert!(entities.schedule.is_none());
}

/// Asserts that the iterator doesn't enter the next segment after yielding the last item
/// of the current one, only when another candidate is requested.
#[test]
fn no_eager_entry() {
    let world = or_world(1, 1);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = (&a | &b).iter();

    assert_eq!(iter.next().map(or_value), Some(0));
    assert!(iter.cursor.is_empty());
    assert_eq!(iter.shiperator.branch, OrBranch::A);
    assert_eq!(iter.schedule.as_ref().unwrap().follow_ups().len(), 1);

    assert_eq!(iter.next().map(or_value), Some(10));
    assert_eq!(iter.shiperator.branch, OrBranch::B);
    assert!(iter.schedule.as_ref().unwrap().follow_ups().is_empty());
}

/// Asserts that a first branch with candidates that all fail their lookups still
/// moves on to the second branch.
#[test]
fn rejected_segment() {
    let mut world = World::new_for_test();
    world.add_entity(Ins(0));
    world.add_entity(Ins(1));
    let f = world.add_entity(B(20));
    world
        .borrow::<ViewMut<'_, Ins>>()
        .unwrap()
        .clear_all_inserted();
    let (ins, b) = world
        .borrow::<(View<'_, Ins, track::Insertion>, View<'_, B>)>()
        .unwrap();

    let mut iter = (ins.inserted() | &b).iter().with_id();

    let first = iter.next().map(|(id, item)| match item {
        OneOfTwo::One(ins) => (id, ins.0),
        OneOfTwo::Two(b) => (id, b.0),
    });
    assert_eq!(first, Some((f, 20)));
    assert_eq!(iter.0.shiperator.branch, OrBranch::B);
    assert!(iter.next().is_none());
}

/// Asserts that an `Or` nested in the second branch of another keeps its own follow-up,
/// embedded under the outer `Or`'s second branch, and that both `Or`s end on their second branch.
#[test]
fn nested_or() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0)));
    world.add_entity((B(1), C(1)));
    world.add_entity(C(2));
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let mut iter = (&a | (&b | &c)).iter();

    let schedule = iter.schedule.as_ref().unwrap();
    let rows: Vec<_> = schedule
        .follow_ups()
        .iter()
        .map(|segment| schedule.plan().row(segment).to_vec())
        .collect();
    assert_eq!(
        rows,
        [
            [
                or(OrBranch::B),
                NodeState::Sailor,
                or(OrBranch::A),
                window(0),
                NodeState::Sailor
            ],
            [
                or(OrBranch::B),
                NodeState::Sailor,
                or(OrBranch::B),
                NodeState::Sailor,
                window(0)
            ],
        ]
    );

    let items: Vec<_> = iter
        .by_ref()
        .map(|item| match item {
            OneOfTwo::One(a) => (0, a.0),
            OneOfTwo::Two(OneOfTwo::One(b)) => (1, b.0),
            OneOfTwo::Two(OneOfTwo::Two(c)) => (2, c.0),
        })
        .collect();
    assert_eq!(items, [(0, 0), (1, 1), (2, 2)]);

    assert_eq!(iter.shiperator.branch, OrBranch::B);
    assert_eq!(iter.shiperator.storages.1.branch, OrBranch::B);
}

/// Asserts that a `Mixed` picking an `Or` as captain embeds the `Or`'s follow-up under its
/// own state and routes the entry to it.
#[test]
fn mixed_or_captain() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), C(0)));
    world.add_entity((B(1), C(1)));
    for i in 2..8 {
        world.add_entity(C(i));
    }
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let mut iter = (&a | &b, &c).iter();

    assert_eq!(iter.shiperator.captain_mask, 0b01);
    let schedule = iter.schedule.as_ref().unwrap();
    let follow_up = schedule.follow_ups()[0];
    assert_eq!(
        schedule.plan().row(&follow_up),
        [
            mixed(0b01),
            or(OrBranch::B),
            NodeState::Sailor,
            window(0),
            NodeState::Sailor
        ]
    );

    let items: Vec<_> = iter
        .by_ref()
        .map(|(a_or_b, c)| (or_value(a_or_b), c.0))
        .collect();
    assert_eq!(items, [(0, 0), (1, 1)]);
    assert_eq!(iter.shiperator.shiperator.0.branch, OrBranch::B);
}

/// Replaying an owned grouped iterator must reinstall the initial state and all follow-ups.
#[test]
fn replay() {
    use super::grouping_fixtures::{coverage_world, A, B};
    let (world, ids) = coverage_world();
    let mut iter_ref = world.iter::<(&A, &B)>();
    for _ in 0..2 {
        let iter = iter_ref.iter();
        assert_eq!(iter.schedule.as_ref().unwrap().follow_ups().len(), 2);
        assert_eq!(iter.with_id().map(|(id, _)| id).collect::<Vec<_>>(), ids);
    }
}

/// Asserts that size hints count the candidates of the current segment and of all follow-ups.
#[test]
fn size_hint() {
    let world = or_world(3, 4);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = (&a | &b).iter();

    assert_eq!(iter.size_hint(), (0, Some(7)));

    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(6)));
}

#[cfg(feature = "parallel")]
mod split {
    use super::*;
    use rayon::iter::plumbing::UnindexedProducer;

    /// Asserts that splitting keeps the plan on both halves, without follow-ups on the left one
    /// and with all of them on the right one.
    #[test]
    fn follow_ups() {
        let world = or_world(4, 3);
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

        let (left, right) = (&a | &b).iter().split();
        let right = right.unwrap();

        assert_eq!(left.cursor.len(), 2);
        assert!(left.schedule.as_ref().unwrap().follow_ups().is_empty());
        assert_eq!(right.cursor.len(), 2);
        assert_eq!(right.schedule.as_ref().unwrap().follow_ups().len(), 1);
    }

    /// Asserts that the right half keeps working once the left half is dropped.
    #[test]
    fn drop_left_first() {
        let world = or_world(4, 3);
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

        let (left, right) = (&a | &b).iter().split();
        drop(left);

        let values: Vec<_> = right.unwrap().map(or_value).collect();
        assert_eq!(values, [2, 3, 10, 11, 12]);
    }

    /// Asserts that the left half keeps working once the right half is dropped.
    #[test]
    fn drop_right_first() {
        let world = or_world(4, 3);
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

        let (left, right) = (&a | &b).iter().split();
        drop(right);

        let values: Vec<_> = left.map(or_value).collect();
        assert_eq!(values, [0, 1]);
    }
}

/// Asserts that entering each segment of `ring_world`'s pair walks installs exactly its masks,
/// coverage, window starts and candidates, captains starting at their own index on both sides of every wrap point,
/// whether segments are entered by `next` or by `fold`.
#[test]
fn ring_entry() {
    use super::grouping_fixtures::{ring_world, A, B};

    let (world, _) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let mut iter = (&a, &b).iter();
    let plan = iter
        .schedule
        .as_ref()
        .unwrap()
        .downgrade()
        .upgrade()
        .unwrap();
    let segments = plan.segments();
    assert_eq!(segments.len(), 6);

    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            assert!(iter.enter_next_segment());
        }

        // A `Mixed` holding coverage can't describe itself as a row, so we read its state.
        let mixed = &iter.shiperator;
        let row = plan.row(segment);
        let NodeState::Mixed {
            captain_mask,
            coverage,
        } = row[0]
        else {
            panic!("grouped walks drive a Mixed");
        };
        // SAFE: `mixed.coverage` points into the plan, which `plan` keeps alive.
        let installed_coverage = unsafe { mixed.coverage.as_ref() };
        assert_eq!(
            (mixed.captain_mask, installed_coverage),
            (captain_mask, plan.coverage(coverage)),
            "segment {index}"
        );

        // Sailors keep whatever start they had, only captains are installed.
        let starts = [
            mixed.shiperator.0.segment_start,
            mixed.shiperator.1.segment_start,
        ];
        for (child, installed) in starts.into_iter().enumerate() {
            if let NodeState::Window { segment_start } = row[1 + child] {
                assert_eq!(installed, segment_start, "segment {index}, child {child}");
            }
        }
        assert_eq!(iter.cursor.as_ptr(), segment.cursor().as_ptr());
        assert_eq!(iter.cursor.len(), segment.len());
    }
    assert!(!iter.enter_next_segment());

    // `next` and `fold` go through the same entries, every segment's items included.
    let next: Vec<u32> = (&a, &b).iter().map(|(a, _)| a.0).collect();
    let fold = (&a, &b).iter().fold(Vec::new(), |mut acc, (a, _)| {
        acc.push(a.0);
        acc
    });
    assert_eq!(next, fold);
    assert_eq!(next.len(), 15);
}

/// Asserts that C stops captaining at its own trust endpoint within a damaged ring,
/// while A keeps driving and B's later unevaluated candidate is rejected by coverage.
#[test]
fn trust_entry() {
    use super::grouping_fixtures::{trust_world, A, B, C};
    use super::grouping_regions::grouped;

    let (world, ids) = trust_world();
    world.borrow::<ViewMut<'_, C>>().unwrap().groups.ranges[0].trust = 2;
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let mut iter = grouped((&a, &b, &c).into_shiperator()).with_id();
    for (i, mask) in [0b111, 0b101, 0b001, 0b001].into_iter().enumerate() {
        let (id, (a, b, c)) = iter.next().unwrap();
        assert_eq!(id, ids[i]);
        assert_eq!((a.0, b.0, c.0), (i as u32, 100 + i as u32, 200 + i as u32));
        assert_eq!(iter.0.shiperator.captain_mask, mask);
    }
    assert!(iter.next().is_none());
    assert_eq!(iter.0.shiperator.captain_mask, 0b010);
}
