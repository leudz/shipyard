//! Wrappers driving or answering lookups in segments that don't start at index 0.
//!
//! Public constructors can't produce such segments yet, plans here are written by hand
//! over real storages like in `grouping_bulk`, whose helpers we reuse.

use super::grouping_bulk::{assert_prefix, every_way, mixed, planned, window, PAIR_LAYOUT};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::mixed::BulkStrategy;
use crate::iter::plan::{CoverageSpan, NodeKind, NodeState, OrBranch};
use crate::iter::{Mixed, Shiper, Shiperator};
use crate::not::Not;
use crate::optional::Optional;
use crate::or::{OneOfTwo, OrWindow};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::{Inserted, InsertedOrModified, Modified};
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
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
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::InsertionAndModification;
}

/// `A` starts with an unrelated `A(100)`, then `e0` to `e3` own `A(i)` at index `i + 1`.\
/// They also own `Tracked(i)` at index `i` of `Tracked`.
///
/// `e0` and `e1` were inserted before the last clear of `Tracked`, `e2` and `e3` after it.
/// Only `e1` was modified since.
fn tracking_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    world.add_entity(A(100));
    let mut e: Vec<EntityId> = (0..2)
        .map(|i| world.add_entity((A(i), Tracked(i))))
        .collect();
    world
        .borrow::<ViewMut<'_, Tracked, track::InsertionAndModification>>()
        .unwrap()
        .clear_all_inserted();
    e.extend((2..4).map(|i| world.add_entity((A(i), Tracked(i)))));

    let mut tracked = world
        .borrow::<ViewMut<'_, Tracked, track::InsertionAndModification>>()
        .unwrap();
    (&mut tracked).get(e[1]).unwrap().0 += 10;
    drop(tracked);

    (world, e)
}

/// `Tracked` and `A` both drive `e1` to `e3`, from starts 1 and 2.
const TRACKED_ROW: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b11,
        coverage: CoverageSpan::EMPTY,
    },
    NodeState::Window { segment_start: 1 },
    NodeState::Window { segment_start: 2 },
];

/// Iterator over `e1` to `e3` with `filter` wrapping `Tracked`'s window and `A` both driving.
///
/// Ignoring the starts would read `e0`'s timestamps and `A(100)`.
fn tracked_segment<'a, F: Shiperator>(
    filter: F,
    a: &'a View<'_, A>,
    tracked_dense: &[EntityId],
) -> Shiper<Mixed<(F, FullRawWindow<'a, A, track::Untracked>)>> {
    let windows = Mixed::new((filter, FullRawWindow::from_view(a)), 0b01);

    planned(
        windows,
        &PAIR_LAYOUT,
        &[(&tracked_dense[1..4], &TRACKED_ROW)],
        &[],
    )
}

fn tracked_value((tracked, a): (&Tracked, &A)) -> (u32, u32) {
    (tracked.0, a.0)
}

/// Asserts that `Inserted` driving a segment starting at 1 reads the insertion timestamps
/// at its absolute indices: `e1` was inserted before the clear and is rejected.
#[test]
fn inserted_offset() {
    let (world, e) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let window = || FullRawWindow::from_view(&tracked);

    let items = every_way(
        || tracked_segment(Inserted(window()), &a, &tracked.dense),
        tracked_value,
    );

    assert_eq!(items, [(e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that `Not<Inserted>` driving a segment starting at 1 only accepts `e1`,
/// the only old component of the segment. `e0` is old too but outside of it.
#[test]
fn not_inserted_offset() {
    let (world, e) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let window = || FullRawWindow::from_view(&tracked);

    let items = every_way(
        || tracked_segment(Not(Inserted(window())), &a, &tracked.dense),
        tracked_value,
    );

    assert_eq!(items, [(e[1], (11, 1))]);
}

/// Asserts that `Modified` driving a segment starting at 1 reads the modification timestamps
/// at its absolute indices: only `e1` is accepted.
#[test]
fn modified_offset() {
    let (world, e) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let window = || FullRawWindow::from_view(&tracked);

    let items = every_way(
        || tracked_segment(Modified(window()), &a, &tracked.dense),
        tracked_value,
    );

    assert_eq!(items, [(e[1], (11, 1))]);
}

/// Asserts that `Not<Modified>` driving a segment starting at 1 rejects the modified `e1`.
#[test]
fn not_modified_offset() {
    let (world, e) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let window = || FullRawWindow::from_view(&tracked);

    let items = every_way(
        || tracked_segment(Not(Modified(window())), &a, &tracked.dense),
        tracked_value,
    );

    assert_eq!(items, [(e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that `InsertedOrModified` driving a segment starting at 1 accepts all of it:
/// `e1` was modified, `e2` and `e3` inserted.
#[test]
fn inserted_or_modified_offset() {
    let (world, e) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let window = || FullRawWindow::from_view(&tracked);

    let items = every_way(
        || tracked_segment(InsertedOrModified(window()), &a, &tracked.dense),
        tracked_value,
    );

    assert_eq!(items, [(e[1], (11, 1)), (e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that `Not<InsertedOrModified>` driving a segment starting at 1 yields nothing.
/// `e0` is neither inserted nor modified but it's outside the segment.
#[test]
fn not_inserted_or_modified_offset() {
    let (world, _) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let window = || FullRawWindow::from_view(&tracked);

    let items = every_way(
        || tracked_segment(Not(InsertedOrModified(window())), &a, &tracked.dense),
        tracked_value,
    );

    assert_eq!(items, []);
}

/// Asserts that `Inserted` as a sailor of `A` driving from start 2 looks entities up
/// without applying any start: `e2` and `e3` are accepted.
#[test]
fn inserted_sailor_offset() {
    let (world, e) = tracking_world();
    let (tracked, a) = world
        .borrow::<(
            View<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let row = [mixed(0b10), NodeState::Sailor, window(2)];
    let iter = || {
        let windows = Mixed::new(
            (
                Inserted(FullRawWindow::from_view(&tracked)),
                FullRawWindow::from_view(&a),
            ),
            0b10,
        );

        planned(windows, &PAIR_LAYOUT, &[(&a.dense[2..5], &row)], &[])
    };

    let items = every_way(iter, tracked_value);

    assert_eq!(items, [(e[2], (2, 2)), (e[3], (3, 3))]);
}

/// Asserts that mutable `Tracked` outputs driving from start 1 are `Mut`s flagging
/// the absolute index of their component, and only once written through.
///
/// Reading every item in every way flags nothing, writing through `e2`'s flags `e2` alone.
#[test]
fn mut_flags_offset() {
    let (world, e) = tracking_world();
    world
        .borrow::<ViewMut<'_, Tracked, track::InsertionAndModification>>()
        .unwrap()
        .clear_all_modified();
    let (mut tracked, a) = world
        .borrow::<(
            ViewMut<'_, Tracked, track::InsertionAndModification>,
            View<'_, A>,
        )>()
        .unwrap();
    let tracked_dense = tracked.dense.clone();

    {
        let windows = Mixed::new(
            (
                FullRawWindowMut::new(&mut tracked),
                FullRawWindow::from_view(&a),
            ),
            0b01,
        );
        let iter = || {
            planned(
                windows.clone(),
                &PAIR_LAYOUT,
                &[(&tracked_dense[1..4], &TRACKED_ROW)],
                &[],
            )
        };

        let items = every_way(iter, |(tracked, a)| (tracked.0, a.0));
        assert_eq!(items, [(e[1], (11, 1)), (e[2], (2, 2)), (e[3], (3, 3))]);

        iter().with_id().for_each(|(id, (mut tracked, _))| {
            if id == e[2] {
                tracked.0 += 100;
            }
        });
    }

    assert!(tracked.is_modified(e[2]));
    assert_eq!((&tracked).get(e[2]).unwrap().0, 102);
    for i in [0, 1, 3] {
        assert!(!tracked.is_modified(e[i]));
    }
}

/// `e0` to `e3` own `A(i)` at index `i + 1` of `A`, after an unrelated `A(100)`.
/// Only `e2` owns a `B`, `B(12)`.
fn optional_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    world.add_entity(A(100));
    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(A(i))).collect();
    world.add_component(e[2], B(12));

    (world, e)
}

/// Asserts that `Optional` sailors of `A` driving from start 2 yield the present `B`
/// and `None` for the entities without one, without dropping them.
#[test]
fn optional_offset() {
    let (world, e) = optional_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let row = [mixed(0b01), window(2), NodeState::Sailor];
    let iter = || {
        let windows = Mixed::new(
            (
                FullRawWindow::from_view(&a),
                Optional(FullRawWindow::from_view(&b)),
            ),
            0b01,
        );

        planned(windows, &PAIR_LAYOUT, &[(&a.dense[2..5], &row)], &[])
    };

    let items = every_way(iter, |(a, b)| (a.0, b.map(|b| b.0)));

    assert_eq!(
        items,
        [(e[1], (1, None)), (e[2], (2, Some(12))), (e[3], (3, None))]
    );
}

/// Asserts that mutable `Optional` sailors of `A` driving from start 2 can be written
/// through when present.
#[test]
fn optional_mut_offset() {
    let (world, e) = optional_world();
    let (a, mut b) = world.borrow::<(View<'_, A>, ViewMut<'_, B>)>().unwrap();
    let row = [mixed(0b01), window(2), NodeState::Sailor];

    {
        let windows = Mixed::new(
            (
                FullRawWindow::from_view(&a),
                Optional(FullRawWindowMut::new(&mut b)),
            ),
            0b01,
        );
        let iter = planned(windows, &PAIR_LAYOUT, &[(&a.dense[2..5], &row)], &[]);

        let absent = iter.fold(0, |absent, (_, b): (&A, Option<&mut B>)| match b {
            Some(b) => {
                b.0 += 100;
                absent
            }
            None => absent + 1,
        });
        assert_eq!(absent, 2);
    }

    assert_eq!((&b).get(e[2]).unwrap(), &B(112));
}

/// Layout of `Or<((A, B), C)>`.
const OR_LAYOUT: [NodeKind; 5] = [
    NodeKind::Or,
    NodeKind::Mixed { children: 2 },
    NodeKind::Window,
    NodeKind::Window,
    NodeKind::Window,
];

/// `A` starts with an unrelated `A(100)`, then `e0` to `e3` own `A(i)` at index `i + 1`
/// and `B(10 + i)` at index `i`.\
/// `e1` also owns `C(21)`, then `f0` only owns `C(30)`.
fn or_world() -> (World, Vec<EntityId>, EntityId) {
    let mut world = World::new();

    world.add_entity(A(100));
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    world.add_component(e[1], C(21));
    let f0 = world.add_entity(C(30));

    (world, e, f0)
}

type OrPair<'a> = OrWindow<(
    Mixed<(
        FullRawWindow<'a, A, track::Untracked>,
        FullRawWindow<'a, B, track::Untracked>,
    )>,
    FullRawWindow<'a, C, track::Untracked>,
)>;

/// Iterator where the pair `(A, B)` drives `e1` to `e3` from starts 2 and 1,
/// then `C` drives its whole storage.
fn or_iter<'a>(a: &'a View<'_, A>, b: &'a View<'_, B>, c: &'a View<'_, C>) -> Shiper<OrPair<'a>> {
    let first = [
        NodeState::Or {
            branch: OrBranch::A,
        },
        mixed(0b11),
        window(2),
        window(1),
        NodeState::Sailor,
    ];
    let second = [
        NodeState::Or {
            branch: OrBranch::B,
        },
        NodeState::Sailor,
        NodeState::Sailor,
        NodeState::Sailor,
        window(0),
    ];
    let or = OrWindow {
        storages: (
            Mixed::new(
                (FullRawWindow::from_view(a), FullRawWindow::from_view(b)),
                0b01,
            ),
            FullRawWindow::from_view(c),
        ),
        branch: OrBranch::A,
    };

    planned(
        or,
        &OR_LAYOUT,
        &[(&a.dense[2..5], &first), (&c.dense[..], &second)],
        &[],
    )
}

fn or_value(item: OneOfTwo<(&A, &B), &C>) -> u32 {
    match item {
        OneOfTwo::One((a, b)) => a.0 + b.0,
        OneOfTwo::Two(c) => c.0,
    }
}

/// Asserts that an `Or` whose first branch is an aligned pair folds that branch with
/// the pair's own aligned loop, then rejects `e1` in the second branch: every way yields
/// the same items, `f0` last.
#[test]
fn or_first_branch_bulk() {
    let (world, e, f0) = or_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let mut entered = or_iter(&a, &b, &c);
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.branch, OrBranch::A);
    assert_eq!(
        entered.shiperator.storages.0.bulk_strategy(),
        BulkStrategy::Aligned
    );

    let items = every_way(|| or_iter(&a, &b, &c), or_value);

    assert_eq!(items, [(e[1], 12), (e[2], 14), (e[3], 16), (f0, 30)]);
}

/// Asserts that bulk consumption through an `Or` resumes correctly after a `next` prefix
/// inside the first branch, at the boundary and inside the second branch.
#[test]
fn or_prefixes() {
    let (world, _, _) = or_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    for prefix in [0, 1, 3, 4, 5] {
        assert_prefix(|| or_iter(&a, &b, &c), or_value, prefix);
    }
}
