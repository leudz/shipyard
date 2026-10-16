//! `Mixed` resolution and coverage, driven by internally built plans over real storages.
//!
//! Plans here aren't built by a planner, each test lays out the regions it pretends are
//! unevaluated, damaged or evaluated ungrouped and writes the segments walking them.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::mixed::NO_SINGLE_CAPTAIN;
use crate::iter::plan::{
    Coverage, CoverageSpan, NodeKind, NodeState, Plan, SegmentDescriptor, SegmentSchedule,
};
use crate::iter::{is_exact_traversal, EntityCursor, Mixed, RawEntityIdAccess, Shiper, Shiperator};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::Inserted;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::vec::Vec;
use core::ptr::NonNull;

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

/// Iterator over the pair `(A, B)`.
type PairIter<'a> = Shiper<Mixed<(FullRawWindow<'a, A>, FullRawWindow<'a, B>)>>;

const PAIR_LAYOUT: [NodeKind; 3] = [
    NodeKind::Mixed { children: 2 },
    NodeKind::Window,
    NodeKind::Window,
];

fn cursor(entities: &[EntityId]) -> EntityCursor {
    // SAFE: Every test keeps the storage `entities` belongs to borrowed while its iterators are used.
    unsafe { EntityCursor::new(NonNull::from(entities).cast(), entities.len()) }
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

/// A `Mixed` state reading the first `len` records of the plan's coverage.
fn covered_mixed(captain_mask: u32, len: usize) -> NodeState {
    NodeState::Mixed {
        captain_mask,
        coverage: CoverageSpan { start: 0, len },
    }
}

fn record(child: u32, start: usize, end: usize) -> Coverage {
    Coverage { child, start, end }
}

/// Returns an iterator walking `segments` in order, each installing its row.
///
/// The iterator starts without any candidate so even the first segment is entered.
fn planned<S: Shiperator>(
    shiperator: S,
    layout: &[NodeKind],
    segments: &[(&[EntityId], &[NodeState])],
    coverage: &[Coverage],
) -> Shiper<S> {
    let descriptors = segments
        .iter()
        .enumerate()
        .map(|(row, &(entities, _))| SegmentDescriptor::from_cursor(cursor(entities), row))
        .collect();
    let rows = segments
        .iter()
        .flat_map(|&(_, row)| row.iter().copied())
        .collect();
    let plan = Plan::new(layout, descriptors, rows, coverage.to_vec()).unwrap();

    Shiper {
        shiperator,
        cursor: EntityCursor::empty(),
        schedule: Some(SegmentSchedule::new(plan)),
        is_exact_sized: false,
    }
}

fn pair_values<'a>(iter: impl Iterator<Item = (&'a A, &'a B)>) -> Vec<(u32, u32)> {
    iter.map(|(a, b)| (a.0, b.0)).collect()
}

fn ids<S: Shiperator>(iter: Shiper<S>) -> Vec<EntityId> {
    iter.with_id().map(|(id, _)| id).collect()
}

/// Entity `i` owns `A(i)` at index `i` of `A`.
/// Entities 2 to 5 also own `B(10 + i)` at index `i - 1` of `B`, after an unrelated `B(100)`.
fn shifted_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    world.add_entity(B(100));
    let e: Vec<EntityId> = (0..6).map(|i| world.add_entity(A(i))).collect();
    for (i, &entity) in e.iter().enumerate().skip(2) {
        world.add_component(entity, B(10 + i as u32));
    }

    (world, e)
}

/// Asserts that a full captain mask resolves every child from the local index,
/// each window applying its own start: `A` starts at 2 and `B` at 1 for the same entities.
#[test]
fn aligned() {
    let (world, e) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let row = [mixed(0b11), window(2), window(1)];
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );

        planned(windows, &PAIR_LAYOUT, &[(&a.dense[2..6], &row)], &[])
    };

    let mut values = iter();
    assert_eq!(values.next().map(|(a, b)| (a.0, b.0)), Some((2, 12)));
    assert_eq!(values.shiperator.captain_mask, 0b11);
    assert_eq!(values.shiperator.single_captain, NO_SINGLE_CAPTAIN);
    assert_eq!(pair_values(values), [(3, 13), (4, 14), (5, 15)]);

    assert_eq!(ids(iter()), &e[2..6]);
}

/// Asserts that consecutive aligned segments each install their own pair of starts.
#[test]
fn different_starts() {
    let (world, e) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let first = [mixed(0b11), window(2), window(1)];
    let second = [mixed(0b11), window(4), window(3)];
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );
        let segments: [(&[EntityId], &[NodeState]); 2] =
            [(&a.dense[2..4], &first), (&a.dense[4..6], &second)];

        planned(windows, &PAIR_LAYOUT, &segments, &[])
    };

    assert_eq!(pair_values(iter()), [(2, 12), (3, 13), (4, 14), (5, 15)]);
    assert_eq!(ids(iter()), &e[2..6]);
}

/// Asserts that a partial mask looks its sailors up: `B` rejects entities 0 and 1,
/// and that a single captain selects its fixed-role resolver.
#[test]
fn partial() {
    let (world, e) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let row = [mixed(0b01), window(0), NodeState::Sailor];
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b11,
        );

        planned(windows, &PAIR_LAYOUT, &[(&a.dense[..], &row)], &[])
    };

    let mut values = iter();
    assert_eq!(values.next().map(|(a, b)| (a.0, b.0)), Some((2, 12)));
    assert_eq!(values.shiperator.single_captain, 0);
    assert_eq!(pair_values(values), [(3, 13), (4, 14), (5, 15)]);

    assert_eq!(ids(iter()), &e[2..6]);
}

/// Asserts that a captain can reject on its own: with both children captains,
/// the insertion filter still rejects the components inserted before the last clear.
#[test]
fn rejecting_captain() {
    let mut world = World::new();
    for i in 0..4 {
        world.add_entity((Ins(i), A(i)));
    }
    world
        .borrow::<ViewMut<'_, Ins>>()
        .unwrap()
        .clear_all_inserted();
    let fresh: Vec<EntityId> = (4..6).map(|i| world.add_entity((Ins(i), A(i)))).collect();
    let (ins, a) = world
        .borrow::<(View<'_, Ins, track::Insertion>, View<'_, A>)>()
        .unwrap();
    let row = [mixed(0b11), window(0), window(0)];
    let iter = || {
        let windows = Mixed::new(
            (
                Inserted(FullRawWindow::from_view(&ins)),
                FullRawWindow::from_view(&a),
            ),
            0b01,
        );

        planned(windows, &PAIR_LAYOUT, &[(&ins.dense[..], &row)], &[])
    };

    let mut values = iter();
    let first = values.next().map(|(ins, a)| (ins.0, a.0));
    assert_eq!(first, Some((4, 4)));
    assert!(!values.shiperator.is_exact_sized());

    assert_eq!(ids(iter()), fresh);
}

/// Asserts that a segment whose candidates all fail their lookups yields nothing
/// and that the iterator still moves on to the next segment.
#[test]
fn rejected_segment() {
    let (world, e) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let rejected = [mixed(0b01), window(0), NodeState::Sailor];
    let aligned = [mixed(0b11), window(2), window(1)];
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );
        let segments: [(&[EntityId], &[NodeState]); 2] =
            [(&a.dense[..2], &rejected), (&a.dense[2..6], &aligned)];

        planned(windows, &PAIR_LAYOUT, &segments, &[])
    };

    assert_eq!(pair_values(iter()), [(2, 12), (3, 13), (4, 14), (5, 15)]);
    assert_eq!(ids(iter()), &e[2..6]);
}

/// Asserts that a plan whose only segment rejects every candidate yields nothing.
#[test]
fn only_rejected() {
    let (world, _) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let rejected = [mixed(0b01), window(0), NodeState::Sailor];
    let windows = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b01,
    );

    let mut iter = planned(windows, &PAIR_LAYOUT, &[(&a.dense[..2], &rejected)], &[]);

    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

/// Asserts that every position of a triple can be the single captain:
/// the cached position matches the mask and the other two are looked up.
///
/// `C` lacks entity 1, so it's rejected whoever drives.
#[test]
fn captain_positions() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    for i in [0, 2, 3] {
        world.add_component(e[i], C(20 + i as u32));
    }
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let layout = [
        NodeKind::Mixed { children: 3 },
        NodeKind::Window,
        NodeKind::Window,
        NodeKind::Window,
    ];
    let dense = [&a.dense[..], &b.dense[..], &c.dense[..]];

    for captain in 0..3 {
        let mut row = [
            mixed(1 << captain),
            NodeState::Sailor,
            NodeState::Sailor,
            NodeState::Sailor,
        ];
        row[1 + captain] = window(0);
        let iter = || {
            let windows = Mixed::new(
                (
                    FullRawWindow::from_view(&a),
                    FullRawWindow::from_view(&b),
                    FullRawWindow::from_view(&c),
                ),
                0b001,
            );

            planned(windows, &layout, &[(dense[captain], &row)], &[])
        };

        let mut values = iter();
        let first = values.next().map(|(a, b, c)| (a.0, b.0, c.0));
        assert_eq!(first, Some((0, 10, 20)));
        assert_eq!(values.shiperator.single_captain, captain as u32);

        let rest: Vec<_> = values.map(|(a, b, c)| (a.0, b.0, c.0)).collect();
        assert_eq!(rest, [(2, 12, 22), (3, 13, 23)]);
        assert_eq!(ids(iter()), [e[0], e[2], e[3]]);
    }
}

/// Asserts that the last of 32 children can be the single captain and that a full 32 bits
/// mask resolves every child directly, without any shift overflow.
#[cfg(feature = "extended_tuple")]
#[test]
fn captain_32() {
    use alloc::vec;

    let mut world = World::new();
    let e: Vec<EntityId> = (0..3).map(|i| world.add_entity(A(i))).collect();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let w = FullRawWindow::from_view(&a);
    let windows = || {
        Mixed::new(
            (
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
                w.clone(),
            ),
            0b1,
        )
    };
    let mut layout = vec![NodeKind::Mixed { children: 32 }];
    layout.extend([NodeKind::Window; 32]);

    let mut last = vec![mixed(1 << 31)];
    last.extend([NodeState::Sailor; 31]);
    last.push(window(0));
    let mut iter = planned(windows(), &layout, &[(&a.dense[..], &last[..])], &[]);
    assert_eq!(
        iter.next().map(|items| (items.0 .0, items.31 .0)),
        Some((0, 0))
    );
    assert_eq!(iter.shiperator.single_captain, 31);
    assert_eq!(ids(iter), &e[1..]);

    let mut full = vec![mixed(u32::MAX)];
    full.extend([window(1); 32]);
    let mut iter = planned(windows(), &layout, &[(&a.dense[1..], &full[..])], &[]);
    assert_eq!(
        iter.next().map(|items| (items.0 .0, items.31 .0)),
        Some((1, 1))
    );
    assert_eq!(iter.shiperator.single_captain, NO_SINGLE_CAPTAIN);
    assert!(iter.shiperator.is_exact_sized());
    assert_eq!(ids(iter), &e[2..]);
}

/// Storages laid out like a grouped pair `(A, B)` after some insertions:
///
/// - `A` holds `e0` and `e1` in its evaluated ungrouped region `0..2`,
///   then `e2`, `e3` and `e5` in its unevaluated region `2..5`
/// - `B` holds `e1`, `e2`, `e4` and `e5`, all in its unevaluated region `0..4`
///
/// Entity `i` owns `A(i)` and `B(10 + i)`.
fn unevaluated_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let mut e: Vec<EntityId> = (0..4).map(|i| world.add_entity(A(i))).collect();
    world.add_component(e[1], B(11));
    world.add_component(e[2], B(12));
    e.push(world.add_entity(B(14)));
    e.push(world.add_entity((A(5), B(15))));

    (world, e)
}

/// `A`'s walk of its unevaluated region, `B` checked.
const A_WALK: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b01,
        coverage: CoverageSpan::EMPTY,
    },
    NodeState::Window { segment_start: 2 },
    NodeState::Sailor,
];

/// `B`'s walk of its unevaluated region, `A` checked against the region `A` walked.
const B_WALK: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b10,
        coverage: CoverageSpan { start: 0, len: 1 },
    },
    NodeState::Sailor,
    NodeState::Window { segment_start: 0 },
];

const A_UNEVALUATED: [Coverage; 1] = [Coverage {
    child: 0,
    start: 2,
    end: 5,
}];

/// Asserts that a candidate whose `A` component lies in the region an earlier walk owns
/// is rejected: `e2` and `e5` are yielded by `A`'s walk only, not again by `B`'s.
#[test]
fn covered() {
    let (world, e) = unevaluated_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );
        let segments: [(&[EntityId], &[NodeState]); 2] =
            [(&a.dense[2..5], &A_WALK), (&b.dense[..], &B_WALK)];

        planned(windows, &PAIR_LAYOUT, &segments, &A_UNEVALUATED)
    };

    assert_eq!(pair_values(iter()), [(2, 12), (5, 15), (1, 11)]);
    assert_eq!(ids(iter()), [e[2], e[5], e[1]]);
}

/// Asserts that a candidate present in an earlier storage only in its evaluated ungrouped
/// region is accepted when `B`'s unevaluated walk reaches it: `e1` owns an `A`,
/// but no walk owns the region it's in. A presence check, like `Or`'s, would lose it.
#[test]
fn ungrouped() {
    let (world, e) = unevaluated_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );

        planned(
            windows,
            &PAIR_LAYOUT,
            &[(&b.dense[..2], &B_WALK)],
            &A_UNEVALUATED,
        )
    };

    let mut values = iter();
    assert_eq!(values.next().map(|(a, b)| (a.0, b.0)), Some((1, 11)));
    assert_eq!(values.shiperator.coverage.len(), 1);
    assert_eq!(values.shiperator.single_captain, NO_SINGLE_CAPTAIN);
    assert!(values.next().is_none());

    assert_eq!(ids(iter()), [e[1]]);
}

/// Asserts that a candidate whose `A` component lies in `A`'s damaged suffix,
/// walked by `A` earlier, is rejected when `B`'s unevaluated walk reaches it.
///
/// `e0` to `e2` own `A` at `0..3`. `B` holds `e0` in its trusted prefix and `e2` in its
/// unevaluated region, so the pair's aligned part is `0..1` and `A`'s damaged suffix `1..3`.
#[test]
fn damaged() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..3).map(|i| world.add_entity(A(i))).collect();
    world.add_component(e[0], B(10));
    world.add_component(e[2], B(12));
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let aligned = [mixed(0b11), window(0), window(0)];
    let damaged = [mixed(0b01), window(1), NodeState::Sailor];
    let unevaluated = [covered_mixed(0b10, 1), NodeState::Sailor, window(1)];
    let a_damaged = [record(0, 1, 3)];
    let iter = || {
        let windows = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );
        let segments: [(&[EntityId], &[NodeState]); 3] = [
            (&a.dense[..1], &aligned),
            (&a.dense[1..], &damaged),
            (&b.dense[1..], &unevaluated),
        ];

        planned(windows, &PAIR_LAYOUT, &segments, &a_damaged)
    };

    assert_eq!(pair_values(iter()), [(0, 10), (2, 12)]);
    assert_eq!(ids(iter()), [e[0], e[2]]);
}

/// Asserts that a nested `Mixed` answers membership normally while it's a sailor,
/// even though the coverage it installed as a captain would reject that entity.
///
/// The query is `(C, (A, B))`. The inner pair first drives `B`'s walk with `A`'s
/// unevaluated region covered, then `C` drives and checks the pair for `e2`,
/// whose `A` is in that region.
#[test]
fn nested_sailor() {
    let (mut world, e) = unevaluated_world();
    world.add_component(e[1], C(21));
    world.add_component(e[2], C(22));
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let layout = [
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Window,
    ];
    let inner_walk = [
        mixed(0b10),
        NodeState::Sailor,
        covered_mixed(0b10, 1),
        NodeState::Sailor,
        window(0),
    ];
    let c_walk = [
        mixed(0b01),
        window(1),
        NodeState::Sailor,
        NodeState::Sailor,
        NodeState::Sailor,
    ];
    let iter = || {
        let inner = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b01,
        );
        let windows = Mixed::new((FullRawWindow::from_view(&c), inner), 0b01);
        let segments: [(&[EntityId], &[NodeState]); 2] =
            [(&b.dense[..], &inner_walk), (&c.dense[1..], &c_walk)];

        planned(windows, &layout, &segments, &A_UNEVALUATED)
    };

    let mut values = iter();
    let first = values.next().map(|(c, (a, b))| (c.0, a.0, b.0));
    assert_eq!(first, Some((21, 1, 11)));

    let second = values.next().map(|(c, (a, b))| (c.0, a.0, b.0));
    assert_eq!(second, Some((22, 2, 12)));
    // The inner pair is a sailor now, it kept the coverage of the segment it drove.
    assert_eq!(values.shiperator.captain_mask, 0b01);
    assert_eq!(values.shiperator.shiperator.1.coverage.len(), 1);
    assert!(values.next().is_none());

    assert_eq!(ids(iter()), [e[1], e[2]]);
}

/// Asserts that rejecting covered candidates doesn't touch components already handed out:
/// every mutable output stays usable after `B`'s walk rejects `e2` and `e5`.
#[test]
fn held_mut() {
    let (world, e) = unevaluated_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let a_dense = a.dense.clone();
    let b_dense = b.dense.clone();
    let windows = Mixed::new(
        (FullRawWindowMut::new(&mut a), FullRawWindowMut::new(&mut b)),
        0b01,
    );
    let segments: [(&[EntityId], &[NodeState]); 2] =
        [(&a_dense[2..5], &A_WALK), (&b_dense[..], &B_WALK)];
    let iter = planned(windows, &PAIR_LAYOUT, &segments, &A_UNEVALUATED);

    let outputs: Vec<(EntityId, (&mut A, &mut B))> = iter.with_id().collect();
    for (_, (a, b)) in outputs {
        a.0 += 100;
        b.0 += 100;
    }

    for (i, a_value, b_value) in [(1, 101, 111), (2, 102, 112), (5, 105, 115)] {
        assert_eq!((&a).get(e[i]).unwrap(), &A(a_value));
        assert_eq!((&b).get(e[i]).unwrap(), &B(b_value));
    }
    assert_eq!((&a).get(e[3]).unwrap(), &A(3));
}

/// Asserts that the coverage of a plan's only segment stays readable after entering it,
/// once no follow-up is left.
#[test]
fn zero_follow_ups() {
    let (world, _) = unevaluated_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let windows = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b01,
    );
    let mut iter = planned(
        windows,
        &PAIR_LAYOUT,
        &[(&b.dense[..], &B_WALK)],
        &A_UNEVALUATED,
    );

    assert_eq!(iter.next().map(|(a, b)| (a.0, b.0)), Some((1, 11)));
    assert!(iter.schedule.as_ref().unwrap().follow_ups().is_empty());

    // `e2` and `e5` read the records to be rejected.
    let rest: Vec<_> = iter.with_id().map(|(id, _)| id).collect();
    assert!(rest.is_empty());
}

/// Splits an iterator in `B`'s walk like `par_iter` does: the left half keeps `e2`
/// without follow-ups, the right half gets `e4` and `e5` with the rest of the schedule.
///
/// Both halves read the shared coverage, `e2` and `e5` are covered.
fn split_in_coverage<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> (PairIter<'a>, PairIter<'a>) {
    let windows = Mixed::new(
        (FullRawWindow::from_view(a), FullRawWindow::from_view(b)),
        0b01,
    );
    let segments: [(&[EntityId], &[NodeState]); 2] =
        [(&a.dense[2..5], &A_WALK), (&b.dense[..], &B_WALK)];
    let mut left = planned(windows, &PAIR_LAYOUT, &segments, &A_UNEVALUATED);

    // `e2` and `e5` from `A`'s walk, then `e1` once in `B`'s.
    let values: Vec<_> = left.by_ref().take(3).map(|(a, b)| (a.0, b.0)).collect();
    assert_eq!(values, [(2, 12), (5, 15), (1, 11)]);

    let (left_cursor, right_cursor) = left.cursor.split().unwrap();
    let right_schedule = left.schedule.as_mut().unwrap().split_follow_ups();
    left.cursor = left_cursor;
    let right = Shiper {
        shiperator: left.shiperator.clone(),
        cursor: right_cursor,
        schedule: Some(right_schedule),
        is_exact_sized: left.is_exact_sized,
    };

    (left, right)
}

/// Asserts that the right half of a split still reads the current coverage
/// once the left half is dropped.
#[test]
fn split_left_dropped() {
    let (world, _) = unevaluated_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (left, right) = split_in_coverage(&a, &b);

    drop(left);

    assert_eq!(right.count(), 0);
}

/// Asserts that the left half of a split still reads the current coverage
/// once the right half is dropped.
#[test]
fn split_right_dropped() {
    let (world, _) = unevaluated_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (left, right) = split_in_coverage(&a, &b);

    drop(right);

    assert_eq!(left.count(), 0);
}

/// Asserts that a `Mixed` is exact only when every child drives and is exact itself:
/// a partial mask, or a full mask over a nested pair with a sailor, can reject.
#[test]
fn exact_children() {
    let (world, _) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pair = |mask| {
        Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            mask,
        )
    };

    assert!(pair(0b11).is_exact_sized());
    assert!(!pair(0b01).is_exact_sized());

    let nested = |inner_mask| Mixed::new((FullRawWindow::from_view(&a), pair(inner_mask)), 0b11);
    assert!(nested(0b11).is_exact_sized());
    assert!(!nested(0b01).is_exact_sized());
}

/// Asserts that a segment with coverage isn't exact, even with every child driving.
#[test]
fn exact_coverage() {
    let (world, _) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let windows = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b01,
    );
    let row = [covered_mixed(0b11, 1), window(2), window(1)];
    let mut iter = planned(
        windows,
        &PAIR_LAYOUT,
        &[(&a.dense[2..6], &row)],
        &[record(0, 0, 1)],
    );

    assert_eq!(iter.next().map(|(a, b)| (a.0, b.0)), Some((2, 12)));
    assert_eq!(iter.shiperator.captain_mask, 0b11);
    assert!(!iter.shiperator.is_exact_sized());
}

/// Asserts that an exact `Mixed` with follow-ups isn't an exact traversal,
/// a later segment might reject, while a window with follow-ups stays exact.
#[test]
fn exact_follow_ups() {
    let (world, _) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let windows = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b11,
    );
    let follow_up = [mixed(0b01), window(0), NodeState::Sailor];
    let planned_pair = planned(
        windows.clone(),
        &PAIR_LAYOUT,
        &[(&a.dense[..], &follow_up)],
        &[],
    );
    let entities = RawEntityIdAccess {
        cursor: cursor(&a.dense[2..6]),
        schedule: planned_pair.schedule,
    };

    assert!(windows.is_exact_sized());
    assert!(!is_exact_traversal(&windows, &entities));

    let a_window = FullRawWindow::from_view(&a);
    let planned_window = planned(
        a_window.clone(),
        &[NodeKind::Window],
        &[(&a.dense[4..], &[window(4)])],
        &[],
    );
    let entities = RawEntityIdAccess {
        cursor: cursor(&a.dense[..4]),
        schedule: planned_window.schedule,
    };

    assert!(is_exact_traversal(&a_window, &entities));
}

/// Asserts that a planned `Mixed`'s size hint counts the current candidates and the follow-ups'
/// as an upper bound only.
#[test]
fn size_hint() {
    let (world, _) = unevaluated_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let windows = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b01,
    );
    let segments: [(&[EntityId], &[NodeState]); 2] =
        [(&a.dense[2..5], &A_WALK), (&b.dense[..], &B_WALK)];
    let mut iter = planned(windows, &PAIR_LAYOUT, &segments, &A_UNEVALUATED);

    assert_eq!(iter.size_hint(), (0, Some(7)));

    iter.next();
    assert_eq!(iter.size_hint(), (0, Some(6)));
}

#[cfg(feature = "parallel")]
mod split {
    use super::*;
    use rayon::iter::plumbing::UnindexedProducer;

    /// Enters `B`'s walk then splits it with the producer's own `split`.
    fn split_iter<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>) -> (PairIter<'a>, PairIter<'a>) {
        let windows = Mixed::new(
            (FullRawWindow::from_view(a), FullRawWindow::from_view(b)),
            0b01,
        );
        let segments: [(&[EntityId], &[NodeState]); 2] =
            [(&a.dense[2..5], &A_WALK), (&b.dense[..], &B_WALK)];
        let mut iter = planned(windows, &PAIR_LAYOUT, &segments, &A_UNEVALUATED);
        assert_eq!(iter.by_ref().take(3).count(), 3);

        let (left, right) = iter.split();

        (left, right.unwrap())
    }

    /// Asserts that the right half of a producer split reads the current coverage
    /// once the left half is dropped.
    #[test]
    fn left_dropped() {
        let (world, _) = unevaluated_world();
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let (left, right) = split_iter(&a, &b);

        drop(left);

        assert_eq!(right.with_id().count(), 0);
    }

    /// Asserts that the left half of a producer split reads the current coverage
    /// once the right half is dropped.
    #[test]
    fn right_dropped() {
        let (world, _) = unevaluated_world();
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let (left, right) = split_iter(&a, &b);

        drop(right);

        assert_eq!(left.with_id().count(), 0);
    }
}
