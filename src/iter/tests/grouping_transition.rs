//! Segment entry and `Or` transitions, checked on the iterator's internal state.
//!
//! Output checks alone could pass with skipped transitions, so these tests also assert
//! the state each node holds after entering a segment.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::plan::{CoverageSpan, NodeKind, NodeState, OrBranch, PlanBuilder};
use crate::iter::{
    EntityCursor, IntoIter, IntoShiperator, Mixed, RawEntityIdAccess, Shiper, Shiperator,
};
use crate::or::{OneOfTwo, OrWindow};
use crate::sparse_set::FullRawWindow;
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
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

fn or(branch: OrBranch) -> NodeState {
    NodeState::Or { branch }
}

/// Returns an iterator walking `segments` in order, each installing its row.
///
/// The iterator starts without any candidate so even the first segment is entered.
fn planned<S: Shiperator>(
    shiperator: S,
    layout: &[NodeKind],
    segments: &[(&[EntityId], &[NodeState])],
) -> Shiper<S> {
    let mut plan = PlanBuilder::new(layout);
    for &(entities, row) in segments {
        plan.push_segment(cursor(entities), row);
    }

    Shiper {
        shiperator,
        cursor: EntityCursor::empty(),
        schedule: plan.finish(),
        is_exact_sized: false,
    }
}

const MIXED_LAYOUT: [NodeKind; 3] = [
    NodeKind::Mixed { children: 2 },
    NodeKind::Window,
    NodeKind::Window,
];

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

/// Entities `e0` to `e{a_len - 1}` own `A(i)`, then `f0` to `f{b_len - 1}` own `B(10 + i)`.
fn or_world(a_len: u32, b_len: u32) -> World {
    let mut world = World::new();

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

/// Asserts that entering a segment installs each captain window's own start with the cursor:
/// for the same entities, `A` starts at 2 and `B` at 1.
#[test]
fn unequal_starts() {
    let (world, e) = shifted_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let windows = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b01,
    );
    let row = [mixed(0b11), window(2), window(1)];
    let mut iter = planned(windows, &MIXED_LAYOUT, &[(&a.dense[2..6], &row)]);

    assert_eq!(iter.next().map(|(a, b)| (a.0, b.0)), Some((2, 12)));

    let state = &iter.shiperator;
    assert_eq!(state.captain_mask, 0b11);
    assert_eq!(
        (
            state.shiperator.0.segment_start,
            state.shiperator.1.segment_start
        ),
        (2, 1)
    );

    let rest: Vec<_> = iter.with_id().map(|(id, (a, b))| (id, a.0, b.0)).collect();
    assert_eq!(rest, [(e[3], 3, 13), (e[4], 4, 14), (e[5], 5, 15)]);
}

/// Asserts that a one segment plan is still owned after entering its segment,
/// with no follow-up left, and that the window resolves from the installed start.
#[test]
fn one_segment() {
    let (world, _) = shifted_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let row = [window(3)];
    let mut iter = planned(
        FullRawWindow::from_view(&a),
        &[NodeKind::Window],
        &[(&a.dense[3..6], &row)],
    );

    assert_eq!(iter.next(), Some(&A(3)));
    assert_eq!(iter.shiperator.segment_start, 3);

    let schedule = iter.schedule.as_ref().unwrap();
    assert!(schedule.follow_ups().is_empty());
    assert_eq!(schedule.remaining_len(), 0);

    assert_eq!(iter.by_ref().collect::<Vec<_>>(), [&A(4), &A(5)]);
    assert!(iter.next().is_none());
    assert!(iter.schedule.is_some());
}

/// Asserts that entering a segment leaves a sailor subtree untouched:
/// the sailor `Or` keeps its branch and its windows keep their segment starts.
#[test]
fn inactive_sailor() {
    let mut world = World::new();
    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(C(i))).collect();
    world.add_component(e[1], A(1));
    world.add_component(e[2], B(2));
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let mut a_window = FullRawWindow::from_view(&a);
    let mut b_window = FullRawWindow::from_view(&b);
    // SAFE: Both windows are only sailors, they never receive a local index.
    unsafe {
        a_window.set_segment_start(7);
        b_window.set_segment_start(8);
    }
    let windows = Mixed::new(
        (
            FullRawWindow::from_view(&c),
            OrWindow {
                storages: (a_window, b_window),
                branch: OrBranch::B,
            },
        ),
        0b01,
    );
    let layout = [
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Or,
        NodeKind::Window,
        NodeKind::Window,
    ];
    let row = [
        mixed(0b01),
        window(0),
        NodeState::Sailor,
        NodeState::Sailor,
        NodeState::Sailor,
    ];
    let mut iter = planned(windows, &layout, &[(&c.dense[..], &row)]);

    let first = iter.next().map(|(c, a_or_b)| (c.0, a_or_b));
    assert_eq!(first, Some((1, OneOfTwo::One(&A(1)))));

    let sailor = &iter.shiperator.shiperator.1;
    assert_eq!(sailor.branch, OrBranch::B);
    assert_eq!(
        (
            sailor.storages.0.segment_start,
            sailor.storages.1.segment_start
        ),
        (7, 8)
    );

    assert_eq!(
        iter.next().map(|(c, a_or_b)| (c.0, a_or_b)),
        Some((2, OneOfTwo::Two(&B(2))))
    );
    assert!(iter.next().is_none());
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
    let mut world = World::new();
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
    let mut world = World::new();
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
    let mut world = World::new();
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

/// Asserts that every iterator obtained from an `IntoIterRef` starts from fresh positions
/// in the shared plan, and replays the same segments.
#[test]
fn replay() {
    let world = or_world(6, 0);
    let a = world.borrow::<View<'_, A>>().unwrap();
    let mut iter_ref = world.iter::<&A>();

    let mut plan = PlanBuilder::new(&[NodeKind::Window]);
    plan.push_segment(cursor(&a.dense[4..6]), &[window(4)]);
    iter_ref.entities = RawEntityIdAccess {
        cursor: cursor(&a.dense[..2]),
        schedule: plan.finish(),
        metadata: Default::default(),
    };

    let first: Vec<_> = iter_ref.iter().map(|a| a.0).collect();
    let second: Vec<_> = iter_ref.iter().map(|a| a.0).collect();

    assert_eq!(first, [0, 1, 4, 5]);
    assert_eq!(second, first);
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
