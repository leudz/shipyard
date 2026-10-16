//! Segments the region builder emits for grouped alternatives, on real worlds.
//!
//! Queries are built through their real constructors, then their grouped alternative is
//! installed with `grouped_entities` whether or not it's the cheaper walk.\
//! Expected matches come from looking each entity up, not from the regions under test.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::plan::{Coverage, NodeState, SegmentDescriptor};
use crate::iter::planner::regions::grouped_entities;
use crate::iter::planner::NodeMetadata;
use crate::iter::{IntoShiperator, Mixed, RawEntityIdAccess, SegmentRow, Shiper, Shiperator};
use crate::sparse_set::{FullRawWindow, Groups};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::vec::Vec;
use core::ops::Range;
use core::ptr;

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

/// Iterator over the pair `(A, B)`.
type PairIter<'a> = Shiper<Mixed<(FullRawWindow<'a, A>, FullRawWindow<'a, B>)>>;

/// Builds the iterator walking the grouped alternative `query`'s constructor reported,
/// whether or not it's cheaper than the ordinary walk.
fn grouped<S: Shiperator>(query: (S, usize, RawEntityIdAccess)) -> Shiper<S> {
    let (mut mixed, _, entities) = query;
    let NodeMetadata::Mixed(selection) = entities.metadata else {
        panic!("the query has a grouped alternative");
    };
    let candidate = selection.grouped.as_ref().unwrap();

    let entities = grouped_entities(&mut mixed, candidate);

    Shiper::new(mixed, entities)
}

/// A segment as tests compare it.
#[derive(Debug, PartialEq)]
pub(super) struct Segment {
    /// Query child whose storage holds the walked entities, and their indices in it.
    walks: (usize, Range<usize>),
    captain_mask: u32,
    /// Segment start of each child, `None` for sailors.
    starts: Vec<Option<usize>>,
    coverage: Vec<Coverage>,
}

pub(super) fn segment(
    walks: (usize, Range<usize>),
    captain_mask: u32,
    starts: &[Option<usize>],
    coverage: &[Coverage],
) -> Segment {
    Segment {
        walks,
        captain_mask,
        starts: starts.to_vec(),
        coverage: coverage.to_vec(),
    }
}

pub(super) fn record(child: u32, start: usize, end: usize) -> Coverage {
    Coverage { child, start, end }
}

/// Describes every segment of the plan of `iter`, a flat query over `storages`.
///
/// Also asserts that `iter` was installed for the first segment.
pub(super) fn segments<S: Shiperator>(iter: &Shiper<S>, storages: &[&[EntityId]]) -> Vec<Segment> {
    let Some(schedule) = &iter.schedule else {
        return Vec::new();
    };
    let plan = schedule.plan();
    let first = &plan.segments()[0];

    // The first segment was entered: our state is its row and our cursor walks its entities.
    assert_eq!(SegmentRow::of(&iter.shiperator).states(), plan.row(first));
    assert_eq!(iter.cursor.as_ptr(), first.cursor().as_ptr());
    assert_eq!(
        (iter.cursor.local_index(), iter.cursor.len()),
        (0, first.len())
    );
    assert_eq!(schedule.follow_ups().len(), plan.segments().len() - 1);

    plan.segments()
        .iter()
        .map(|segment| {
            let row = plan.row(segment);
            let NodeState::Mixed {
                captain_mask,
                coverage,
            } = row[0]
            else {
                panic!("grouped walks drive a Mixed");
            };

            Segment {
                walks: walked(segment, storages),
                captain_mask,
                starts: row[1..]
                    .iter()
                    .map(|state| match *state {
                        NodeState::Window { segment_start } => Some(segment_start),
                        NodeState::Sailor => None,
                        _ => unreachable!("flat queries only have windows"),
                    })
                    .collect(),
                coverage: plan.coverage(coverage).to_vec(),
            }
        })
        .collect()
}

/// Returns the child whose storage holds `segment`'s entities and their indices in it.
fn walked(segment: &SegmentDescriptor, storages: &[&[EntityId]]) -> (usize, Range<usize>) {
    let first: *const EntityId = segment.cursor().as_ptr().as_ptr();

    storages
        .iter()
        .enumerate()
        .find_map(|(child, storage)| {
            let start = storage.iter().position(|entity| ptr::eq(entity, first))?;

            Some((child, start..start + segment.len()))
        })
        .expect("segments walk one of the query's storages")
}

/// Returns the segments of the grouped walks of `(&a, &b)`.
fn pair_segments(a: &View<'_, A>, b: &View<'_, B>) -> Vec<Segment> {
    let iter = grouped((a, b).into_shiperator());

    segments(&iter, &[&a.dense, &b.dense])
}

/// Asserts that no segment walks `span` of child `child`'s storage and no record covers it.
pub(super) fn assert_skipped(segments: &[Segment], child: usize, span: Range<usize>) {
    let overlaps = |range: &Range<usize>| range.start < span.end && span.start < range.end;

    for segment in segments {
        let (walked_child, walked) = &segment.walks;
        assert!(*walked_child != child || !overlaps(walked));

        for record in &segment.coverage {
            let covered = record.start..record.end;
            assert!(record.child as usize != child || !overlaps(&covered));
        }
    }
}

/// Returns the sorted entities of `all` owning both A and B, looked up one by one.
fn owners(all: &[EntityId], a: &View<'_, A>, b: &View<'_, B>) -> Vec<EntityId> {
    let mut owners: Vec<EntityId> = all
        .iter()
        .copied()
        .filter(|&entity| a.contains(entity) && b.contains(entity))
        .collect();
    owners.sort_unstable();

    owners
}

/// Asserts that `id`'s item holds its own components.
fn assert_item(id: EntityId, (a_item, b_item): (&A, &B), a: &View<'_, A>, b: &View<'_, B>) {
    assert!(ptr::eq(a_item, a.get(id).unwrap()), "{id:?}'s A");
    assert!(ptr::eq(b_item, b.get(id).unwrap()), "{id:?}'s B");
}

/// Returns the sorted ids `iter` yields through `next`.
fn next_ids(iter: PairIter<'_>, a: &View<'_, A>, b: &View<'_, B>) -> Vec<EntityId> {
    let mut ids = Vec::new();
    // `for` calls `next` for each item, it doesn't go through `fold`.
    for (id, item) in iter.with_id() {
        assert_item(id, item, a, b);
        ids.push(id);
    }
    ids.sort_unstable();

    ids
}

/// Returns the sorted ids `iter` yields through `fold`.
fn fold_ids(iter: PairIter<'_>, a: &View<'_, A>, b: &View<'_, B>) -> Vec<EntityId> {
    let mut ids = iter.with_id().fold(Vec::new(), |mut ids, (id, item)| {
        assert_item(id, item, a, b);
        ids.push(id);
        ids
    });
    ids.sort_unstable();

    ids
}

/// Asserts that the grouped walks of `(&a, &b)` yield each entity of `all` owning A and B
/// exactly once, through `next` and through `fold`.
fn assert_pairs(a: &View<'_, A>, b: &View<'_, B>, all: &[EntityId]) {
    let expected = owners(all, a, b);

    assert_eq!(next_ids(grouped((a, b).into_shiperator()), a, b), expected);
    assert_eq!(fold_ids(grouped((a, b).into_shiperator()), a, b), expected);
}

/// Overwrites the regions of a storage's single group range.
///
/// Fixtures use it for states the base can't produce yet and document why their state is valid.
fn set_regions(groups: &mut Groups, range: Range<usize>, trust: usize, unevaluated_start: usize) {
    let group_range = &mut groups.ranges[0];
    group_range.start = range.start;
    group_range.len = range.len();
    group_range.trust = trust;
    groups.unevaluated_start = unevaluated_start;
}

/// Asserts that a regrouped pair is a single aligned segment where both members drive,
/// skipping the components each storage evaluated as ungrouped.
#[test]
fn aligned() {
    let mut world = World::new();
    let mut all: Vec<EntityId> = (0..4).map(|i| world.add_entity((A(i), B(i)))).collect();
    all.push(world.add_entity(A(10)));
    all.push(world.add_entity(B(10)));
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let segments = pair_segments(&a, &b);

    assert_eq!(
        segments,
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_skipped(&segments, 0, 4..5);
    assert_skipped(&segments, 1, 4..5);
    assert!(grouped((&a, &b).into_shiperator()).is_exact_sized);
    assert_pairs(&a, &b, &all);
}

/// Asserts that with an AB group in an ABC query, the aligned segment has C as a sailor
/// which rejects the entities lacking C, so the traversal isn't exact.
#[test]
fn rejecting_sailor() {
    let mut world = World::new();
    let all: Vec<EntityId> = (0..4).map(|i| world.add_entity((A(i), B(i)))).collect();
    world.add_component(all[1], C(1));
    world.add_component(all[3], C(3));
    world.add_entity(C(10));
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = grouped((&a, &b, &c).into_shiperator());

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense, &c.dense]),
        [segment((0, 0..4), 0b011, &[Some(0), Some(0), None], &[])]
    );
    assert!(!iter.is_exact_sized);

    let mut expected: Vec<EntityId> = all
        .iter()
        .copied()
        .filter(|&entity| a.contains(entity) && b.contains(entity) && c.contains(entity))
        .collect();
    expected.sort_unstable();

    let mut ids: Vec<EntityId> = iter.with_id().map(|(id, _)| id).collect();
    ids.sort_unstable();
    assert_eq!(ids, expected);

    let mut folded =
        grouped((&a, &b, &c).into_shiperator())
            .with_id()
            .fold(Vec::new(), |mut ids, (id, _)| {
                ids.push(id);
                ids
            });
    folded.sort_unstable();
    assert_eq!(folded, expected);
}

/// Asserts that entities added after a regroup are walked once through the unevaluated
/// regions: the A-only and B-only ones are rejected, the AB one is yielded by A's walk and
/// rejected by B's thanks to the record of A's unevaluated region.
#[test]
fn new_entities() {
    let mut world = World::new();
    let mut all: Vec<EntityId> = (0..3).map(|i| world.add_entity((A(i), B(i)))).collect();
    world.group::<(A, B)>();
    all.push(world.add_entity(A(10)));
    all.push(world.add_entity(B(10)));
    all.push(world.add_entity((A(11), B(11))));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        pair_segments(&a, &b),
        [
            segment((0, 0..3), 0b11, &[Some(0), Some(0)], &[]),
            segment((0, 3..5), 0b01, &[Some(3), None], &[]),
            segment((1, 3..5), 0b10, &[None, Some(3)], &[record(0, 3, 5)]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that an entity whose A was evaluated as ungrouped before it gained B is found
/// by B's unevaluated walk: A's ungrouped region is neither walked nor covered.
#[test]
fn a_ungrouped_then_b() {
    let mut world = World::new();
    let mut all: Vec<EntityId> = (0..2).map(|i| world.add_entity((A(i), B(i)))).collect();
    let late = world.add_entity(A(10));
    all.push(late);
    world.group::<(A, B)>();
    world.add_component(late, B(10));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let segments = pair_segments(&a, &b);

    assert_eq!(
        segments,
        [
            segment((0, 0..2), 0b11, &[Some(0), Some(0)], &[]),
            segment((1, 2..3), 0b10, &[None, Some(2)], &[]),
        ]
    );
    assert_skipped(&segments, 0, 2..3);
    assert_pairs(&a, &b, &all);
}

/// Asserts that an entity whose B was evaluated as ungrouped before it gained A is found
/// by A's unevaluated walk: B's ungrouped region is neither walked nor covered.
#[test]
fn b_ungrouped_then_a() {
    let mut world = World::new();
    let mut all: Vec<EntityId> = (0..2).map(|i| world.add_entity((A(i), B(i)))).collect();
    let late = world.add_entity(B(10));
    all.push(late);
    world.group::<(A, B)>();
    world.add_component(late, A(10));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let segments = pair_segments(&a, &b);

    assert_eq!(
        segments,
        [
            segment((0, 0..2), 0b11, &[Some(0), Some(0)], &[]),
            segment((0, 2..3), 0b01, &[Some(2), None], &[]),
        ]
    );
    assert_skipped(&segments, 1, 2..3);
    assert_pairs(&a, &b, &all);
}

/// Asserts that an entity with both members unevaluated is walked by both unevaluated
/// walks and yielded once, by the first.
#[test]
fn both_unevaluated() {
    let mut world = World::new();
    world.group::<(A, B)>();
    let all = [world.add_entity((A(0), B(0)))];

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        pair_segments(&a, &b),
        [
            segment((0, 0..1), 0b01, &[Some(0), None], &[]),
            segment((1, 0..1), 0b10, &[None, Some(0)], &[record(0, 0, 1)]),
        ]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that after a removal resets A while B keeps its 4 long range, A's unevaluated
/// walk alone finds every match. A regroup then makes the pair aligned again.
#[test]
fn reset() {
    let mut world = World::new();
    let all: Vec<EntityId> = (0..4).map(|i| world.add_entity((A(i), B(i)))).collect();
    world.group::<(A, B)>();
    world.remove::<A>(all[0]);

    {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        assert_eq!((a.groups.ranges[0].len, b.groups.ranges[0].len), (0, 4));

        assert_eq!(
            pair_segments(&a, &b),
            [segment((0, 0..3), 0b01, &[Some(0), None], &[])]
        );
        assert_pairs(&a, &b, &all);
    }

    world.regroup();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        pair_segments(&a, &b),
        [segment((0, 0..3), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that a group without any component has no segment: the iterator is exhausted
/// without a plan.
#[test]
fn empty_group() {
    let mut world = World::new();
    world.group::<(A, B)>();
    world.add_entity(C(0));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = grouped((&a, &b).into_shiperator());

    assert!(iter.schedule.is_none());
    assert!(iter.cursor.is_empty());
    assert!(iter.next().is_none());
    assert_pairs(&a, &b, &[]);
}

/// Asserts that A's unevaluated candidates are walked even when B rejects them all,
/// a nonempty segment can yield nothing.
#[test]
fn unmatched_candidates() {
    let mut world = World::new();
    world.group::<(A, B)>();
    let all = [world.add_entity(A(0)), world.add_entity(A(1))];

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        pair_segments(&a, &b),
        [segment((0, 0..2), 0b01, &[Some(0), None], &[])]
    );
    assert_pairs(&a, &b, &all);
}

/// Asserts that with no prefix trusted by both members, a damaged walk owned by the first
/// member finds every match, looking the other one up.
///
/// Both ranges hold both entities, in different orders. A trusts none of its range,
/// B trusts all of it, the unevaluated regions are empty.
#[test]
fn zero_trust() {
    let mut world = World::new();
    world.group::<(A, B)>();
    let first = world.add_entity(B(0));
    let second = world.add_entity((A(1), B(1)));
    world.add_component(first, A(0));
    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        set_regions(&mut a.groups, 0..2, 0, 2);
        set_regions(&mut b.groups, 0..2, 2, 2);
    }

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        pair_segments(&a, &b),
        [segment((0, 0..2), 0b01, &[Some(0), None], &[])]
    );
    assert_pairs(&a, &b, &[first, second]);
}

/// Entities of `partial_world`.
struct Partial {
    world: World,
    all: Vec<EntityId>,
    /// In both ranges, trusted by both members.
    aligned: [EntityId; 2],
    /// In both ranges, trusted by A only.
    damaged: EntityId,
    /// In A's range and B's unevaluated region.
    late_b: EntityId,
    /// Unevaluated in both.
    unevaluated: EntityId,
    /// Evaluated as ungrouped in B, then gained A.
    late_a: EntityId,
}

/// A world with an AB group whose ranges don't start at the same index and are partially trusted.
///
/// ```text
///          range                   ungrouped  unevaluated
/// A: [x | e1 e2 e3 late_b]         [g]        [unevaluated late_a]
///       trust 3
/// B:     [e1 e2 e3]                [late_a]   [late_b unevaluated]
///        trust 2
/// ```
///
/// `x` stands for components before A's range, another group's range once storages hold
/// several. Neither `x` nor `g` own B, so they aren't matches.\
/// Members hold the same entities over the 2 positions they both trust and every match
/// with both components evaluated is in both ranges, as group metadata guarantees.
fn partial_world() -> Partial {
    let mut world = World::new();
    world.group::<(A, B)>();

    // Every component lands at the end of its storage, the regions are set afterwards.
    let x = world.add_entity(A(0));
    let e1 = world.add_entity((A(1), B(1)));
    let e2 = world.add_entity((A(2), B(2)));
    let damaged = world.add_entity((A(3), B(3)));
    let late_a = world.add_entity(B(4));
    let late_b = world.add_entity((A(5), B(5)));
    let g = world.add_entity(A(6));
    let unevaluated = world.add_entity((A(7), B(7)));
    world.add_component(late_a, A(4));

    {
        let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
        set_regions(&mut a.groups, 1..5, 3, 6);
        set_regions(&mut b.groups, 0..3, 2, 4);
    }

    Partial {
        world,
        all: Vec::from([x, e1, e2, damaged, late_a, late_b, g, unevaluated]),
        aligned: [e1, e2],
        damaged,
        late_b,
        unevaluated,
        late_a,
    }
}

/// Asserts the segments of `partial_world`:
/// - the aligned walk starts at each member's own range start
/// - B has the shorter damaged suffix and walks it alone
/// - A's unevaluated walk comes first on equal lengths and rejects what B's damaged walk owns
/// - B's unevaluated walk rejects what A's unevaluated walk owns
///
/// The ungrouped regions are never walked nor covered.
#[test]
fn partial_trust() {
    let partial = partial_world();
    let (a, b) = partial
        .world
        .borrow::<(View<'_, A>, View<'_, B>)>()
        .unwrap();
    let segments = pair_segments(&a, &b);

    assert_eq!(
        segments,
        [
            segment((0, 1..3), 0b11, &[Some(1), Some(0)], &[]),
            segment((1, 2..3), 0b10, &[None, Some(2)], &[]),
            segment((0, 6..8), 0b01, &[Some(6), None], &[record(1, 2, 3)]),
            segment((1, 4..6), 0b10, &[None, Some(4)], &[record(0, 6, 8)]),
        ]
    );
    assert_skipped(&segments, 0, 5..6);
    assert_skipped(&segments, 1, 3..4);
    assert_pairs(&a, &b, &partial.all);
}

/// Asserts that consuming any prefix with `next` then folding the rest yields every match once.
#[test]
fn prefixes() {
    let partial = partial_world();
    let (a, b) = partial
        .world
        .borrow::<(View<'_, A>, View<'_, B>)>()
        .unwrap();
    let expected = owners(&partial.all, &a, &b);

    for prefix in 0..=expected.len() {
        let mut iter = grouped((&a, &b).into_shiperator()).with_id();
        let mut ids: Vec<EntityId> = iter.by_ref().take(prefix).map(|(id, _)| id).collect();
        ids = iter.fold(ids, |mut ids, (id, item)| {
            assert_item(id, item, &a, &b);
            ids.push(id);
            ids
        });
        ids.sort_unstable();

        assert_eq!(ids, expected, "prefix {prefix}");
    }
}

/// Asserts that mutable items of every segment can be held together: each match is yielded
/// once, its components written once.
#[test]
fn held_mut() {
    let partial = partial_world();
    let (mut a, mut b) = partial
        .world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>()
        .unwrap();

    let mut items: Vec<(&mut A, &mut B)> = grouped((&mut a, &mut b).into_shiperator()).collect();
    for (a, b) in &mut items {
        a.0 += 100;
        b.0 += 100;
    }
    assert_eq!(items.len(), 6);
    drop(items);

    for &entity in &partial.all {
        let is_match = a.contains(entity) && b.contains(entity);
        let written = (&a).get(entity).is_ok_and(|a| a.0 >= 100);

        assert_eq!(written, is_match, "{entity:?}");
        if is_match {
            assert!((&a).get(entity).unwrap().0 < 200);
            assert!((100..200).contains(&(&b).get(entity).unwrap().0));
        }
    }
}

/// Returns `partial_world`'s grouped iterator after yielding its first 3 items,
/// entered in A's unevaluated segment which has coverage.
fn in_covered_segment<'a>(
    partial: &Partial,
    a: &'a View<'_, A>,
    b: &'a View<'_, B>,
) -> PairIter<'a> {
    let mut iter = grouped((a, b).into_shiperator());

    for entity in [partial.aligned[0], partial.aligned[1], partial.damaged] {
        let (a_item, _) = iter.next().unwrap();

        assert!(ptr::eq(a_item, a.get(entity).unwrap()));
    }
    assert!(iter.enter_next_segment());
    assert_eq!(iter.cursor.len(), 2);
    assert_ne!(iter.shiperator.coverage.len(), 0);

    iter
}

/// Asserts that after splitting a covered segment, consuming and dropping the right half
/// first leaves the left half's coverage readable, and the halves yield every match once.
#[test]
fn split_right_first() {
    let partial = partial_world();
    let (a, b) = partial
        .world
        .borrow::<(View<'_, A>, View<'_, B>)>()
        .unwrap();
    let (left, right) = in_covered_segment(&partial, &a, &b).split_current_segment_at(1);

    assert_eq!(
        next_ids(right, &a, &b),
        owners(&[partial.late_a, partial.late_b], &a, &b)
    );
    assert_eq!(next_ids(left, &a, &b), [partial.unevaluated]);
}

/// Asserts that after splitting a covered segment, dropping the left half first leaves the
/// right half's coverage readable, the right half still rejecting what earlier walks own.
#[test]
fn split_left_dropped() {
    let partial = partial_world();
    let (a, b) = partial
        .world
        .borrow::<(View<'_, A>, View<'_, B>)>()
        .unwrap();
    let (left, right) = in_covered_segment(&partial, &a, &b).split_current_segment_at(1);
    drop(left);

    assert_eq!(
        fold_ids(right, &a, &b),
        owners(&[partial.late_a, partial.late_b], &a, &b)
    );
}
