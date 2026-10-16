//! Segment bulk traversal: `fold` and `for_each` folding one segment at a time.
//!
//! Plans here are written by hand over real storages, like in `grouping_resolution`.
//! Every traversal is compared against `WithId::next`, item by item and in order.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::mixed::BulkStrategy;
use crate::iter::plan::{
    Coverage, CoverageSpan, NodeKind, NodeState, Plan, SegmentDescriptor, SegmentSchedule,
};
use crate::iter::{EntityCursor, IntoIter, Mixed, Shiper, Shiperator};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::Inserted;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::fmt::{Debug, Write};
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

#[derive(Debug, PartialEq)]
struct Modif(u32);
impl Component for Modif {
    type Tracking = track::Modification;
}

#[derive(Debug, PartialEq)]
struct Zst;
impl Component for Zst {
    type Tracking = track::Untracked;
}

const PAIR_LAYOUT: [NodeKind; 3] = [
    NodeKind::Mixed { children: 2 },
    NodeKind::Window,
    NodeKind::Window,
];

const TRIPLE_LAYOUT: [NodeKind; 4] = [
    NodeKind::Mixed { children: 3 },
    NodeKind::Window,
    NodeKind::Window,
    NodeKind::Window,
];

/// Layout of `(A, (B, C))`.
const NESTED_LAYOUT: [NodeKind; 5] = [
    NodeKind::Mixed { children: 2 },
    NodeKind::Window,
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

/// Consumes the iterators `iter` builds in every way and asserts they all yield
/// the same items in the same order.
///
/// Returns the items with their ids, as `WithId::next` yields them.
fn every_way<S, T>(iter: impl Fn() -> Shiper<S>, value: impl Fn(S::Out) -> T) -> Vec<(EntityId, T)>
where
    S: Shiperator,
    T: PartialEq + Debug + Clone,
{
    // `for` loops only call `next`.
    let mut with_ids = Vec::new();
    for (id, item) in iter().with_id() {
        with_ids.push((id, value(item)));
    }
    let ids: Vec<EntityId> = with_ids.iter().map(|(id, _)| *id).collect();
    let values: Vec<T> = with_ids.iter().map(|(_, value)| value.clone()).collect();

    let mut next = Vec::new();
    for item in iter() {
        next.push(value(item));
    }
    assert_eq!(next, values, "next");

    let fold = iter().fold(Vec::new(), |mut acc, item| {
        acc.push(value(item));
        acc
    });
    assert_eq!(fold, values, "fold");

    let mut for_each = Vec::new();
    iter().for_each(|item| for_each.push(value(item)));
    assert_eq!(for_each, values, "for_each");

    let id_fold = iter().with_id().fold(Vec::new(), |mut acc, (id, item)| {
        acc.push((id, value(item)));
        acc
    });
    assert_eq!(id_fold, with_ids, "WithId fold");

    let mut id_for_each = Vec::new();
    iter()
        .with_id()
        .for_each(|(id, item)| id_for_each.push((id, value(item))));
    assert_eq!(id_for_each, with_ids, "WithId for_each");

    let mut only_ids = Vec::new();
    iter().ids().for_each(|id| only_ids.push(id));
    assert_eq!(only_ids, ids, "ids");

    with_ids
}

/// Takes `prefix` items with `next` then consumes the rest in bulk, in every way.
///
/// Asserts each way ends up with exactly the items of a full traversal, in order.
fn assert_prefix<S, T>(iter: impl Fn() -> Shiper<S>, value: impl Fn(S::Out) -> T, prefix: usize)
where
    S: Shiperator,
    T: PartialEq + Debug + Clone,
{
    let all = every_way(&iter, &value);
    let all_values: Vec<T> = all.iter().map(|(_, value)| value.clone()).collect();

    let head_with_ids = || {
        let mut iter = iter().with_id();
        let head: Vec<(EntityId, T)> = (&mut iter)
            .take(prefix)
            .map(|(id, item)| (id, value(item)))
            .collect();

        (head, iter)
    };
    let head_values = || {
        let mut iter = iter();
        let head: Vec<T> = (&mut iter).take(prefix).map(&value).collect();

        (head, iter)
    };

    let (head, rest) = head_values();
    let fold = rest.fold(head, |mut acc, item| {
        acc.push(value(item));
        acc
    });
    assert_eq!(fold, all_values, "fold after {prefix}");

    let (mut for_each, rest) = head_values();
    rest.for_each(|item| for_each.push(value(item)));
    assert_eq!(for_each, all_values, "for_each after {prefix}");

    let (head, rest) = head_with_ids();
    let id_fold = rest.fold(head, |mut acc, (id, item)| {
        acc.push((id, value(item)));
        acc
    });
    assert_eq!(id_fold, all, "WithId fold after {prefix}");

    let (mut id_for_each, rest) = head_with_ids();
    rest.for_each(|(id, item)| id_for_each.push((id, value(item))));
    assert_eq!(id_for_each, all, "WithId for_each after {prefix}");
}

/// Entity `e(i)` owns `A(i)` at index `i` of `A`, from `e0` to `e5`.
/// `e0` reuses the index of a deleted entity, its generation isn't 0.
///
/// `B` starts with two entities without `A`, then holds `e1` to `e4` at indices 2 to 5
/// and `e0` at index 6. `e(i)` owns `B(10 + i)`, `e5` has no `B`.\
/// Only `e2` owns a `C`, `C(22)`.
fn world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let deleted = world.add_entity(());
    world.delete_entity(deleted);

    let mut e = Vec::new();
    e.push(world.add_entity(A(0)));
    world.add_entity(B(100));
    world.add_entity(B(101));
    e.extend((1..6).map(|i| world.add_entity(A(i))));
    for i in [1, 2, 3, 4, 0] {
        world.add_component(e[i], B(10 + i as u32));
    }
    world.add_component(e[2], C(22));

    assert_eq!(e[0].index(), deleted.index());
    assert_ne!(e[0].gen(), deleted.gen());

    (world, e)
}

/// `A` drives `e0` alone, `B` is looked up at index 6.
const SINGLE: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b01,
        coverage: CoverageSpan::EMPTY,
    },
    NodeState::Window { segment_start: 0 },
    NodeState::Sailor,
];

/// `A` and `B` both drive `e1` and `e2`, from different starts.
const ALIGNED: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b11,
        coverage: CoverageSpan::EMPTY,
    },
    NodeState::Window { segment_start: 1 },
    NodeState::Window { segment_start: 2 },
];

/// `B` drives `e1`, `e2`, `e3`, `e4` and `e0`, the region of `A` the segments above walked
/// is covered. Only `e3` and `e4` are accepted, `e0` is a rejected tail.
const CHECKED: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b10,
        coverage: CoverageSpan { start: 0, len: 1 },
    },
    NodeState::Sailor,
    NodeState::Window { segment_start: 2 },
];

/// `B` drives its two entities without `A`, both are rejected.
const REJECTED: [NodeState; 3] = [
    NodeState::Mixed {
        captain_mask: 0b10,
        coverage: CoverageSpan::EMPTY,
    },
    NodeState::Sailor,
    NodeState::Window { segment_start: 0 },
];

/// `A`'s region `0..3`, walked by `SINGLE` and `ALIGNED`.
const A_WALKED: [Coverage; 1] = [Coverage {
    child: 0,
    start: 0,
    end: 3,
}];

fn pair<'a>(
    a: &'a View<'_, A>,
    b: &'a View<'_, B>,
) -> Mixed<(FullRawWindow<'a, A>, FullRawWindow<'a, B>)> {
    Mixed::new(
        (FullRawWindow::from_view(a), FullRawWindow::from_view(b)),
        0b01,
    )
}

fn pair_value((a, b): (&A, &B)) -> (u32, u32) {
    (a.0, b.0)
}

/// Iterator over `SINGLE`, `ALIGNED`, `CHECKED` then `REJECTED`, yielding `e0` to `e4`.
fn full<'a>(
    a: &'a View<'_, A>,
    b: &'a View<'_, B>,
) -> Shiper<Mixed<(FullRawWindow<'a, A>, FullRawWindow<'a, B>)>> {
    let segments: [(&[EntityId], &[NodeState]); 4] = [
        (&a.dense[..1], &SINGLE),
        (&a.dense[1..3], &ALIGNED),
        (&b.dense[2..7], &CHECKED),
        (&b.dense[..2], &REJECTED),
    ];

    planned(pair(a, b), &PAIR_LAYOUT, &segments, &A_WALKED)
}

fn full_expected(e: &[EntityId]) -> Vec<(EntityId, (u32, u32))> {
    (0..5).map(|i| (e[i], (i as u32, 10 + i as u32))).collect()
}

/// Asserts that a query without plan folds its only segment like `next`,
/// with the single captain loop: `A` drives and `e5` is rejected.
#[test]
fn ordinary() {
    let (world, e) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let iter = (&a, &b).iter();
    assert!(iter.schedule.is_none());
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::SingleCaptain);

    let items = every_way(|| (&a, &b).iter(), pair_value);
    assert_eq!(items, full_expected(&e));
}

/// Asserts that a single window folds with the default loop like `next`.
#[test]
fn single_window() {
    let (world, e) = world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let items = every_way(|| a.iter(), |a| a.0);
    let expected: Vec<_> = (0..6).map(|i| (e[i], i as u32)).collect();
    assert_eq!(items, expected);
}

/// Asserts that a segment where both windows drive, from different starts,
/// selects the aligned loop and folds like `next`.
#[test]
fn aligned() {
    let (world, e) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = || {
        planned(
            pair(&a, &b),
            &PAIR_LAYOUT,
            &[(&a.dense[1..3], &ALIGNED)],
            &[],
        )
    };

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.bulk_strategy(), BulkStrategy::Aligned);

    let items = every_way(iter, pair_value);
    assert_eq!(items, [(e[1], (1, 11)), (e[2], (2, 12))]);
}

/// Asserts that a segment with a single captain selects its loop and folds like `next`.
#[test]
fn single_captain() {
    let (world, e) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let row = [mixed(0b01), window(3), NodeState::Sailor];
    let iter = || planned(pair(&a, &b), &PAIR_LAYOUT, &[(&a.dense[3..], &row)], &[]);

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert_eq!(
        entered.shiperator.bulk_strategy(),
        BulkStrategy::SingleCaptain
    );

    // `e5` has no `B`.
    let items = every_way(iter, pair_value);
    assert_eq!(items, [(e[3], (3, 13)), (e[4], (4, 14))]);
}

/// Asserts that a segment with two captains and a sailor selects the partial loop
/// and folds like `next`: `C` rejects `e1`.
#[test]
fn partial() {
    let (world, e) = world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let row = [mixed(0b011), window(1), window(2), NodeState::Sailor];
    let iter = || {
        let windows = Mixed::new(
            (
                FullRawWindow::from_view(&a),
                FullRawWindow::from_view(&b),
                FullRawWindow::from_view(&c),
            ),
            0b001,
        );

        planned(windows, &TRIPLE_LAYOUT, &[(&a.dense[1..3], &row)], &[])
    };

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.bulk_strategy(), BulkStrategy::Partial);

    let items = every_way(iter, |(a, b, c)| (a.0, b.0, c.0));
    assert_eq!(items, [(e[2], (2, 12, 22))]);
}

/// Asserts that a segment with coverage selects the checked loop and folds like `next`:
/// `e1`, `e2` and `e0` are covered.
#[test]
fn checked() {
    let (world, e) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = || {
        let segments: [(&[EntityId], &[NodeState]); 2] =
            [(&a.dense[1..3], &ALIGNED), (&b.dense[2..7], &CHECKED)];

        planned(pair(&a, &b), &PAIR_LAYOUT, &segments, &A_WALKED)
    };

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.bulk_strategy(), BulkStrategy::Checked);

    let items = every_way(iter, pair_value);
    assert_eq!(
        items,
        [
            (e[1], (1, 11)),
            (e[2], (2, 12)),
            (e[3], (3, 13)),
            (e[4], (4, 14))
        ]
    );
}

/// Asserts that every loop kind in a single plan folds like `next`, with each id matching
/// its components across storages starting at different indices and with `e0`'s
/// reused generation.
#[test]
fn every_segment() {
    let (world, e) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let items = every_way(|| full(&a, &b), pair_value);
    assert_eq!(items, full_expected(&e));

    for (id, (a_value, b_value)) in items {
        assert_eq!((&a).get(id).unwrap().0, a_value);
        assert_eq!((&b).get(id).unwrap().0, b_value);
    }
}

/// Asserts that bulk consumption after an empty prefix yields every item.
#[test]
fn prefix_zero() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_prefix(|| full(&a, &b), pair_value, 0);
}

/// Asserts that bulk consumption resumes inside a segment: `next` took `e0` and `e1`,
/// `e2` is left in `ALIGNED`.
#[test]
fn prefix_inside() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_prefix(|| full(&a, &b), pair_value, 2);
}

/// Asserts that bulk consumption resumes exactly at a segment boundary:
/// `next` exhausted `ALIGNED` without entering `CHECKED`.
#[test]
fn prefix_boundary() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_prefix(|| full(&a, &b), pair_value, 3);
}

/// Asserts that bulk consumption after the last item only consumes rejected candidates:
/// `CHECKED`'s covered `e0` and `REJECTED`'s whole segment.
#[test]
fn prefix_rejected_tail() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_prefix(|| full(&a, &b), pair_value, 5);
}

/// Asserts that bulk consumption of an exhausted iterator yields nothing,
/// after `next` returned `None` twice.
#[test]
fn prefix_beyond() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_prefix(|| full(&a, &b), pair_value, 7);
}

/// Asserts that a fold keeps its closure's state and a non-`Copy` accumulator
/// across every segment.
#[test]
fn stateful_fold() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut calls = 0;

    let text = full(&a, &b).fold(String::new(), |mut acc, (a, b)| {
        calls += 1;
        write!(acc, "{}:{} ", a.0, b.0).unwrap();
        acc
    });

    assert_eq!(calls, 5);
    assert_eq!(text, "0:10 1:11 2:12 3:13 4:14 ");
}

/// Asserts that a `for_each` closure keeps its state across every segment:
/// each call sees the value the previous one stored.
#[test]
fn stateful_for_each() {
    let (world, e) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut previous = None;
    let mut pairs = Vec::new();

    full(&a, &b).with_id().for_each(|(id, (a, _))| {
        pairs.push((previous, id));
        previous = Some(a.0);
    });

    let expected: Vec<_> = (0..5)
        .map(|i: usize| (i.checked_sub(1).map(|p| p as u32), e[i]))
        .collect();
    assert_eq!(pairs, expected);
}

/// Increments its counter when dropped.
struct Dropped<'a>(&'a Cell<u32>);

impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// Asserts that a fold moving its accumulator and callback through every segment
/// drops neither until it's done, then each exactly once.
#[test]
fn fold_drops() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let item_drops = Cell::new(0);
    let guard_drops = Cell::new(0);
    let guard = Dropped(&guard_drops);
    let item_drops_ref = &item_drops;

    let items = full(&a, &b).fold(Vec::new(), move |mut acc, _| {
        let _ = &guard;
        acc.push(Dropped(item_drops_ref));
        acc
    });

    assert_eq!(item_drops.get(), 0);
    assert_eq!(guard_drops.get(), 1);
    assert_eq!(items.len(), 5);

    drop(items);
    assert_eq!(item_drops.get(), 5);
}

/// Asserts that a callback panicking in the second segment drops the accumulator,
/// the callback and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn fold_panic() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let item_drops = Cell::new(0);
    let guard_drops = Cell::new(0);
    let iter = full(&a, &b);
    let plan = iter.schedule.as_ref().unwrap().downgrade();

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let guard = Dropped(&guard_drops);
        let item_drops = &item_drops;

        iter.fold(Vec::new(), move |mut acc, (a, _)| {
            let _ = &guard;
            if a.0 == 2 {
                panic!("callback panic on e2");
            }
            acc.push(Dropped(item_drops));
            acc
        })
    }));

    assert!(result.is_err());
    assert_eq!(item_drops.get(), 2);
    assert_eq!(guard_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that a `WithId::for_each` callback panicking in the second segment
/// drops the callback and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn for_each_panic() {
    let (world, _) = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let guard_drops = Cell::new(0);
    let iter = full(&a, &b);
    let plan = iter.schedule.as_ref().unwrap().downgrade();

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let guard = Dropped(&guard_drops);

        iter.with_id().for_each(move |(_, (a, _))| {
            let _ = &guard;
            if a.0 == 2 {
                panic!("callback panic on e2");
            }
        });
    }));

    assert!(result.is_err());
    assert_eq!(guard_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that mutable outputs of every segment can all be held at once,
/// then written through.
#[test]
fn mutable_held() {
    let (world, e) = world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let a_dense = a.dense.clone();
    let b_dense = b.dense.clone();
    let windows = Mixed::new(
        (FullRawWindowMut::new(&mut a), FullRawWindowMut::new(&mut b)),
        0b01,
    );
    let segments: [(&[EntityId], &[NodeState]); 4] = [
        (&a_dense[..1], &SINGLE),
        (&a_dense[1..3], &ALIGNED),
        (&b_dense[2..7], &CHECKED),
        (&b_dense[..2], &REJECTED),
    ];
    let iter = planned(windows, &PAIR_LAYOUT, &segments, &A_WALKED);

    let outputs = iter.with_id().fold(Vec::new(), |mut acc, item| {
        acc.push(item);
        acc
    });
    for (_, (a, b)) in outputs {
        a.0 += 100;
        b.0 += 100;
    }

    for (i, &id) in e[..5].iter().enumerate() {
        assert_eq!((&a).get(id).unwrap().0, 100 + i as u32);
        assert_eq!((&b).get(id).unwrap().0, 110 + i as u32);
    }
    assert_eq!((&a).get(e[5]).unwrap().0, 5);
}

/// Asserts that mutable outputs fold like `next` in every way.
#[test]
fn mutable_every_way() {
    let (world, e) = world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let a_dense = a.dense.clone();
    let b_dense = b.dense.clone();
    let windows = Mixed::new(
        (FullRawWindowMut::new(&mut a), FullRawWindowMut::new(&mut b)),
        0b01,
    );
    let iter = || {
        let segments: [(&[EntityId], &[NodeState]); 4] = [
            (&a_dense[..1], &SINGLE),
            (&a_dense[1..3], &ALIGNED),
            (&b_dense[2..7], &CHECKED),
            (&b_dense[..2], &REJECTED),
        ];

        planned(windows.clone(), &PAIR_LAYOUT, &segments, &A_WALKED)
    };

    let items = every_way(iter, |(a, b): (&mut A, &mut B)| (a.0, b.0));
    assert_eq!(items, full_expected(&e));
}

/// Asserts that tracked mutable outputs are `Mut`s flagging only what's written through them:
/// `e1` and `e3` are written, `e0` and `e2` only read, and the rejected `e4` never yielded.
#[test]
fn tracked() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..5).map(|i| world.add_entity((Modif(i), A(i)))).collect();
    world.delete_component::<A>(e[4]);
    let (mut modif, mut a) = world
        .borrow::<(ViewMut<'_, Modif, track::Modification>, ViewMut<'_, A>)>()
        .unwrap();
    let modif_dense = modif.dense.clone();
    let windows = Mixed::new(
        (
            FullRawWindowMut::new(&mut modif),
            FullRawWindowMut::new(&mut a),
        ),
        0b01,
    );
    let aligned = [mixed(0b11), window(0), window(0)];
    let single = [mixed(0b01), window(2), NodeState::Sailor];
    let segments: [(&[EntityId], &[NodeState]); 2] =
        [(&modif_dense[..2], &aligned), (&modif_dense[2..], &single)];
    let iter = planned(windows, &PAIR_LAYOUT, &segments, &[]);

    let held = iter.fold(Vec::new(), |mut acc, (modif, a)| {
        acc.push((modif, a));
        acc
    });
    assert_eq!(held.len(), 4);
    for (mut modif, a) in held {
        if modif.0 % 2 == 1 {
            modif.0 += 100;
            a.0 += 100;
        } else {
            assert_eq!(modif.0, a.0);
        }
    }

    for i in [1, 3] {
        assert!(modif.is_modified(e[i]));
        assert_eq!((&modif).get(e[i]).unwrap().0, 100 + i as u32);
    }
    for i in [0, 2, 4] {
        assert!(!modif.is_modified(e[i]));
    }
}

/// Asserts that zero-sized outputs fold like `next` and that their mutable references
/// can all be held at once.
#[test]
fn zero_sized() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..3).map(|i| world.add_entity((Zst, A(i)))).collect();
    let (mut zst, mut a) = world
        .borrow::<(ViewMut<'_, Zst>, ViewMut<'_, A>)>()
        .unwrap();
    let zst_dense = zst.dense.clone();
    let row = [mixed(0b11), window(0), window(0)];
    let windows = Mixed::new(
        (
            FullRawWindowMut::new(&mut zst),
            FullRawWindowMut::new(&mut a),
        ),
        0b01,
    );
    let iter = || {
        planned(
            windows.clone(),
            &PAIR_LAYOUT,
            &[(&zst_dense[..], &row)],
            &[],
        )
    };

    let items = every_way(iter, |(zst, a): (&mut Zst, &mut A)| (*zst == Zst, a.0));
    let expected: Vec<_> = (0..3).map(|i| (e[i], (true, i as u32))).collect();
    assert_eq!(items, expected);

    let held = iter().fold(Vec::new(), |mut acc, item| {
        acc.push(item);
        acc
    });
    for (zst, a) in held {
        *zst = Zst;
        a.0 += 10;
    }
    assert_eq!((&a).get(e[2]).unwrap().0, 12);
}

/// `e0` to `e3` own `A(i)` and `B(10 + i)` at index `i`, only `e1` and `e3` own `C(20 + i)`.
fn nested_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    for i in [1, 3] {
        world.add_component(e[i], C(20 + i as u32));
    }

    (world, e)
}

/// Asserts that a nested `Mixed` that can reject keeps its parent out of the aligned loop,
/// even with every child of the parent driving: the inner pair looks `C` up.
#[test]
fn nested_rejecting() {
    let (world, e) = nested_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let row = [
        mixed(0b11),
        window(0),
        mixed(0b01),
        window(0),
        NodeState::Sailor,
    ];
    let iter = || {
        let inner = Mixed::new(
            (FullRawWindow::from_view(&b), FullRawWindow::from_view(&c)),
            0b01,
        );
        let windows = Mixed::new((FullRawWindow::from_view(&a), inner), 0b01);

        planned(windows, &NESTED_LAYOUT, &[(&a.dense[..], &row)], &[])
    };

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.captain_mask, 0b11);
    assert_eq!(entered.shiperator.bulk_strategy(), BulkStrategy::Partial);

    let items = every_way(iter, |(a, (b, c))| (a.0, b.0, c.0));
    assert_eq!(items, [(e[1], (1, 11, 21)), (e[3], (3, 13, 23))]);
}

/// Asserts that a nested `Mixed` where every child drives keeps its parent in the aligned loop.
#[test]
fn nested_exact() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..2)
        .map(|i| world.add_entity((A(i), B(10 + i), C(20 + i))))
        .collect();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let row = [mixed(0b11), window(0), mixed(0b11), window(0), window(0)];
    let iter = || {
        let inner = Mixed::new(
            (FullRawWindow::from_view(&b), FullRawWindow::from_view(&c)),
            0b01,
        );
        let windows = Mixed::new((FullRawWindow::from_view(&a), inner), 0b01);

        planned(windows, &NESTED_LAYOUT, &[(&a.dense[..], &row)], &[])
    };

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.bulk_strategy(), BulkStrategy::Aligned);

    let items = every_way(iter, |(a, (b, c))| (a.0, b.0, c.0));
    assert_eq!(items, [(e[0], (0, 10, 20)), (e[1], (1, 11, 21))]);
}

/// Asserts that a tracking filter driving the segment keeps it out of the aligned loop:
/// components inserted before the last clear are rejected.
#[test]
fn tracking_captain() {
    let mut world = World::new();
    for i in 0..2 {
        world.add_entity((Ins(i), A(i)));
    }
    world
        .borrow::<ViewMut<'_, Ins>>()
        .unwrap()
        .clear_all_inserted();
    let fresh: Vec<EntityId> = (2..4).map(|i| world.add_entity((Ins(i), A(i)))).collect();
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

    let mut entered = iter();
    assert!(entered.enter_next_segment());
    assert_eq!(entered.shiperator.bulk_strategy(), BulkStrategy::Partial);

    let items = every_way(iter, |(ins, a)| (ins.0, a.0));
    assert_eq!(items, [(fresh[0], (2, 2)), (fresh[1], (3, 3))]);
}
