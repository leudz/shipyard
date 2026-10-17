//! Grouped walks the normal constructors pick and install, on worlds grouped by `group`
//! and `regroup`.
//!
//! Iterators come from `iter` and `World::iter`, nothing is installed by hand.\
//! We check which walk won and the segments it installed, then compare the items with
//! the entities we know own every component.

use super::grouping_regions::{assert_skipped, record, segment, segments, Segment};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::mixed::BulkStrategy;
use crate::iter::{IntoIter, Shiper, Shiperator};
use crate::sparse_set::{GroupRange, Groups};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::sync::Arc;
use alloc::vec;
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
struct D(u32);
impl Component for D {
    type Tracking = track::Untracked;
}

struct Zst0;
impl Component for Zst0 {
    type Tracking = track::Untracked;
}

struct Zst1;
impl Component for Zst1 {
    type Tracking = track::Untracked;
}

/// Adds `count` entities owning `A(i)` and `B(i)`.
fn add_ab(world: &mut World, count: u32) -> Vec<EntityId> {
    (0..count).map(|i| world.add_entity((A(i), B(i)))).collect()
}

fn sorted(mut ids: Vec<EntityId>) -> Vec<EntityId> {
    ids.sort_unstable();
    ids
}

/// Returns the sorted ids `iter` yields through `next`.
fn next_ids<S: Shiperator>(iter: Shiper<S>) -> Vec<EntityId> {
    let mut ids = Vec::new();
    // `for` calls `next` for each item, it doesn't go through `fold`.
    for (id, _) in iter.with_id() {
        ids.push(id);
    }

    sorted(ids)
}

/// Returns the sorted ids `iter` yields through `fold`.
fn fold_ids<S: Shiperator>(iter: Shiper<S>) -> Vec<EntityId> {
    let ids = iter.with_id().fold(Vec::new(), |mut ids, (id, _)| {
        ids.push(id);
        ids
    });

    sorted(ids)
}

/// 10 AB pairs and 2 A-only entities are grouped, then `late` gets an A and a B.
///
/// A holds the group range at `0..10`, the A-only entities evaluated as ungrouped at `10..12`
/// and `late` unevaluated at `12`. B holds the range at `0..10` and `late` unevaluated at `10`.\
/// The grouped walks cost 26 against 33 for B captaining alone.
///
/// Returns the pairs, `late` last.
fn follow_ups_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let mut pairs = add_ab(&mut world, 10);
    world.add_entity(A(100));
    world.add_entity(A(101));
    world.group::<(A, B)>();
    pairs.push(world.add_entity((A(10), B(10))));

    (world, pairs)
}

/// Asserts that a regrouped pair whose storages also hold unmatched components is walked
/// as a single aligned segment where both members drive, and yields exactly the pairs.
#[test]
fn aligned() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    world.add_entity(A(10));
    world.add_entity(B(10));
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert!(iter.is_exact_sized);
    assert_eq!(next_ids(iter), sorted(pairs.clone()));
    assert_eq!(fold_ids((&a, &b).iter()), sorted(pairs));
}

/// Asserts that mutable outputs of an aligned pair drive both members and that each write
/// lands on its own entity's components, leaving the unmatched ones alone.
#[test]
fn aligned_mut() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    let lone_a = world.add_entity(A(10));
    let lone_b = world.add_entity(B(10));
    world.group::<(A, B)>();

    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let iter = (&mut a, &mut b).iter();
    assert_eq!(iter.shiperator.captain_mask, 0b11);
    assert_eq!(iter.cursor.len(), 4);

    iter.for_each(|(a, b)| {
        a.0 += 100;
        b.0 += 200;
    });

    for (i, &id) in (0..).zip(&pairs) {
        assert_eq!((&a).get(id).unwrap().0, 100 + i);
        assert_eq!((&b).get(id).unwrap().0, 200 + i);
    }
    assert_eq!((&a).get(lone_a).unwrap().0, 10);
    assert_eq!((&b).get(lone_b).unwrap().0, 10);
}

/// Asserts that an ABC query with an AB group drives the aligned pair with C as a sailor
/// when C is larger, rejecting the pairs lacking C.
#[test]
fn rejecting_sailor() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    world.add_component(pairs[1], C(1));
    world.add_component(pairs[3], C(3));
    for i in 0..5 {
        world.add_entity(C(10 + i));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = (&a, &b, &c).iter();

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense, &c.dense]),
        [segment((0, 0..4), 0b011, &[Some(0), Some(0), None], &[])]
    );
    assert!(!iter.is_exact_sized);
    assert_eq!(next_ids(iter), sorted(vec![pairs[1], pairs[3]]));
}

/// Asserts that a single C is a cheaper captain than the aligned AB pair:
/// the ordinary walk is installed, without plan.
#[test]
fn small_captain() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    world.add_component(pairs[2], C(2));
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = (&a, &b, &c).iter();

    assert_eq!(iter.shiperator.captain_mask, 0b100);
    assert!(iter.schedule.is_none());
    assert_eq!(next_ids(iter), [pairs[2]]);
}

/// Asserts that with AB and CD groups in an ABCD query, the CD group wins when the AB group
/// has entities left to walk in its unevaluated regions.
#[test]
fn cd_over_ab() {
    let mut world = World::new();
    let all: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i))))
        .collect();
    world.group::<(A, B)>();
    world.group::<(C, D)>();
    add_ab(&mut world, 2);

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();
    let iter = (&a, &b, &c, &d).iter();

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense, &c.dense, &d.dense]),
        [segment(
            (2, 0..4),
            0b1100,
            &[None, None, Some(0), Some(0)],
            &[]
        )]
    );
    assert_eq!(next_ids(iter), sorted(all));
}

/// Asserts that with AB and CD groups in an ABCD query, the AB group wins when the CD group
/// has entities left to walk in its unevaluated regions.
#[test]
fn ab_over_cd() {
    let mut world = World::new();
    let all: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i))))
        .collect();
    world.group::<(A, B)>();
    world.group::<(C, D)>();
    world.add_entity((C(10), D(10)));
    world.add_entity((C(11), D(11)));

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();
    let iter = (&a, &b, &c, &d).iter();

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense, &c.dense, &d.dense]),
        [segment(
            (0, 0..4),
            0b0011,
            &[Some(0), Some(0), None, None],
            &[]
        )]
    );
    assert_eq!(next_ids(iter), sorted(all));
}

/// Asserts that a group of other storages doesn't make an AB query grouped.
#[test]
fn unrelated_group() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    world.add_entity(A(10));
    world.add_entity((C(0), D(0)));
    world.group::<(C, D)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();

    assert_eq!(iter.shiperator.captain_mask, 0b10);
    assert!(iter.schedule.is_none());
    assert_eq!(next_ids(iter), sorted(pairs));
}

/// Asserts that an AB query with only an ABC group stays ordinary and still yields the pairs
/// lacking C, which sit outside the group range.
#[test]
fn projection() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 5);
    for &id in &pairs[..3] {
        world.add_component(id, C(0));
    }
    world.group::<(A, B, C)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();

    assert!(iter.schedule.is_none());
    assert_eq!(next_ids(iter), sorted(pairs));
}

/// AB and AC groups after regroup promoted grouped entities into ABC, which rotates rings:
/// - AB holds 6 entities, two of its first 8 gained a C
/// - AC holds 7 entities, one of its first 8 gained a B
/// - ABC holds 7 entities, the 4 owning A, B and C from the start and the 3 promoted
/// - 3 entities own a B alone
///
/// Returns every entity.
fn overlap_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let ab = add_ab(&mut world, 8);
    let ac: Vec<EntityId> = (0..8).map(|i| world.add_entity((A(i), C(i)))).collect();
    let abc: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C(i))))
        .collect();
    let b_only: Vec<EntityId> = (0..3).map(|i| world.add_entity(B(i))).collect();
    world.group::<(A, B)>().group::<(A, C)>();

    world.add_component(ab[0], C(0));
    world.add_component(ab[1], C(1));
    world.add_component(ac[0], B(0));
    world.regroup();

    let all = [ab, ac, abc, b_only].concat();

    (world, all)
}

/// AB and ACD groups holding 6 AB entities, 6 ACD entities and 5 entities owning all four
/// components in their ABCD union.
///
/// Returns every entity.
fn extra_world() -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let mut all = add_ab(&mut world, 6);
    all.extend((0..6).map(|i| world.add_entity((A(i), C(i), D(i)))));
    all.extend((0..5).map(|i| world.add_entity((A(i), B(i), C(i), D(i)))));
    world.group::<(A, B)>().group::<(A, C, D)>();

    (world, all)
}

/// Returns the sorted entities of `all` `is_match` accepts.
fn matching(all: &[EntityId], is_match: impl Fn(EntityId) -> bool) -> Vec<EntityId> {
    sorted(all.iter().copied().filter(|&id| is_match(id)).collect())
}

/// Returns every index of child 0's storage `segments` walk, sorted.
fn walked_indices(segments: &[Segment]) -> Vec<usize> {
    let mut indices: Vec<usize> = segments
        .iter()
        .flat_map(|segment| {
            let (child, range) = segment.walks.clone();
            assert_eq!(child, 0);

            range
        })
        .collect();
    indices.sort_unstable();

    indices
}

/// Returns every index of the ranges of `groups` whose group every one of `others` holds too,
/// sorted.
fn shared_indices(groups: &Groups, others: &[&Groups]) -> Vec<usize> {
    let is_shared = |range: &&GroupRange| {
        others.iter().all(|other| {
            other
                .ranges
                .iter()
                .any(|other_range| Arc::ptr_eq(&other_range.group, &range.group))
        })
    };

    // Ranges are in physical order, so are their indices.
    groups
        .ranges
        .iter()
        .filter(is_shared)
        .flat_map(|range| range.start..range.end())
        .collect()
}

/// Asserts that an `(A, B)` query over AB and AC installs aligned walks over A's AB and ABC
/// ranges, cut where regroup left a wrap point, and yields every pair once.
#[test]
fn overlapping_pairs() {
    let (world, all) = overlap_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();
    let segments = segments(&iter, &[&a.dense, &b.dense]);

    // AB and ABC are fully trusted and nothing is unevaluated, so 2 walks without coverage.
    // Any other segment comes from a cut at a wrap point.
    assert!(segments.len() > 2);
    for segment in &segments {
        assert_eq!(segment.captain_mask, 0b11);
        assert!(segment.coverage.is_empty());
    }
    assert_eq!(
        walked_indices(&segments),
        shared_indices(&a.groups, &[&b.groups])
    );

    // Several segments aren't an exact traversal. B captaining alone would count 16.
    assert_eq!(iter.size_hint(), (0, Some(13)));

    let expected = matching(&all, |id| a.contains(id) && b.contains(id));
    assert_eq!(next_ids((&a, &b).iter()), expected);
    assert_eq!(fold_ids((&a, &b).iter()), expected);
}

/// Asserts that an `(A, B, C)` query over AB and AC installs walks over the ABC union alone,
/// never walking nor covering A's AB and AC ranges.
#[test]
fn union_triples() {
    let (world, all) = overlap_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = (&a, &b, &c).iter();
    let segments = segments(&iter, &[&a.dense, &b.dense, &c.dense]);

    for segment in &segments {
        assert_eq!(segment.captain_mask, 0b111);
        assert!(segment.coverage.is_empty());
    }
    assert_eq!(
        walked_indices(&segments),
        shared_indices(&a.groups, &[&b.groups, &c.groups])
    );
    for range in &a.groups.ranges {
        if range.group.len() == 2 {
            assert_skipped(&segments, 0, range.start..range.end());
        }
    }

    let expected = matching(&all, |id| {
        a.contains(id) && b.contains(id) && c.contains(id)
    });
    assert_eq!(next_ids((&a, &b, &c).iter()), expected);
    assert_eq!(fold_ids((&a, &b, &c).iter()), expected);
}

/// Asserts that with AB and AC, an ABC entity added after the last regroup is found by the
/// unevaluated walks alone, before any ABC union exists: A's walk yields it, B's and C's
/// reject it thanks to the records of the walks before them.
#[test]
fn absent_union() {
    let mut world = World::new();
    let mut all = Vec::new();
    for i in 0..3 {
        all.push(world.add_entity((A(i), B(i))));
        all.push(world.add_entity((A(i), C(i))));
    }
    world.group::<(A, B)>().group::<(A, C)>();
    let late = world.add_entity((A(10), B(10), C(10)));
    all.push(late);

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = (&a, &b, &c).iter();

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense, &c.dense]),
        [
            segment((0, 6..7), 0b001, &[Some(6), None, None], &[]),
            segment((1, 3..4), 0b010, &[None, Some(3), None], &[record(0, 6, 7)]),
            segment(
                (2, 3..4),
                0b100,
                &[None, None, Some(3)],
                &[record(0, 6, 7), record(1, 3, 4)]
            ),
        ]
    );
    assert_eq!(next_ids(iter), [late]);
    assert_eq!(fold_ids((&a, &b, &c).iter()), [late]);
}

/// Asserts that with AB and ACD, an `(A, B, C)` query installs segments whose masks reach
/// the bulk strategy: ABCD segments have every child as captain and fold without lookups,
/// AB segments keep looking C up.
#[test]
fn extra_captain() {
    let (world, all) = extra_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let mut iter = (&a, &b, &c).iter();
    let segments = segments(&iter, &[&a.dense, &b.dense, &c.dense]);

    let mut strategies = Vec::new();
    loop {
        strategies.push(iter.shiperator.bulk_strategy());
        if !iter.enter_next_segment() {
            break;
        }
    }

    // A's only range shared with B and C is ABCD's.
    let abcd = shared_indices(&a.groups, &[&b.groups, &c.groups]);
    let mut kinds = Vec::new();
    for (segment, strategy) in segments.iter().zip(strategies) {
        let in_abcd = abcd.contains(&segment.walks.1.start);
        kinds.push(in_abcd);

        if in_abcd {
            assert_eq!(
                (segment.captain_mask, strategy),
                (0b111, BulkStrategy::Aligned)
            );
        } else {
            assert_eq!(
                (segment.captain_mask, strategy),
                (0b011, BulkStrategy::Partial)
            );
        }
    }
    assert!(kinds.contains(&true) && kinds.contains(&false));

    let expected = matching(&all, |id| {
        a.contains(id) && b.contains(id) && c.contains(id)
    });
    assert_eq!(expected.len(), 5);
    assert_eq!(next_ids((&a, &b, &c).iter()), expected);
    assert_eq!(fold_ids((&a, &b, &c).iter()), expected);
}

/// Asserts that a single D is a cheaper captain than walks over AB and ABC:
/// the ordinary walk is installed, without plan.
#[test]
fn overlap_small_captain() {
    let (mut world, _) = overlap_world();
    let lone = world.add_entity((A(100), B(100), D(100)));

    let (a, b, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, D>)>()
        .unwrap();
    let iter = (&a, &b, &d).iter();

    assert_eq!(iter.shiperator.captain_mask, 0b100);
    assert!(iter.schedule.is_none());
    assert_eq!(next_ids(iter), [lone]);
}

/// Asserts that entities added after a regroup are installed as follow-ups: A's unevaluated
/// walk, then B's which A's region covers. The A-only entities are never walked.
#[test]
fn follow_ups() {
    let (world, pairs) = follow_ups_world();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();
    let segments = segments(&iter, &[&a.dense, &b.dense]);

    assert_eq!(
        segments,
        [
            segment((0, 0..10), 0b11, &[Some(0), Some(0)], &[]),
            segment((0, 12..13), 0b01, &[Some(12), None], &[]),
            segment((1, 10..11), 0b10, &[None, Some(10)], &[record(0, 12, 13)]),
        ]
    );
    assert_skipped(&segments, 0, 10..12);
    assert!(!iter.is_exact_sized);
    assert_eq!(next_ids(iter), sorted(pairs.clone()));
    assert_eq!(fold_ids((&a, &b).iter()), sorted(pairs));
}

/// Asserts that an entity whose A was evaluated as ungrouped before it gained B is found
/// by B's unevaluated walk, while A's ungrouped region is skipped.
#[test]
fn ungrouped_member() {
    let mut world = World::new();
    let mut pairs = add_ab(&mut world, 10);
    let late = world.add_entity(A(10));
    world.group::<(A, B)>();
    world.add_component(late, B(10));
    pairs.push(late);

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();
    let segments = segments(&iter, &[&a.dense, &b.dense]);

    assert_eq!(
        segments,
        [
            segment((0, 0..10), 0b11, &[Some(0), Some(0)], &[]),
            segment((1, 10..11), 0b10, &[None, Some(10)], &[]),
        ]
    );
    assert_skipped(&segments, 0, 10..11);
    assert_eq!(next_ids(iter), sorted(pairs));
}

/// Asserts that once a removal resets A, the grouped walk ties with the ordinary one and
/// the ordinary walk is installed. After a regroup, the pair is aligned again.
#[test]
fn reset_then_regroup() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 10);
    world.group::<(A, B)>();
    world.remove::<A>(pairs[0]);

    {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        assert_eq!((a.groups.ranges[0].len, b.groups.ranges[0].len), (0, 10));

        let iter = (&a, &b).iter();
        assert_eq!(iter.shiperator.captain_mask, 0b01);
        assert!(iter.schedule.is_none());
        assert_eq!(next_ids(iter), sorted(pairs[1..].to_vec()));
    }

    world.regroup();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();
    let segments = segments(&iter, &[&a.dense, &b.dense]);

    assert_eq!(
        segments,
        [segment((0, 0..9), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_skipped(&segments, 1, 9..10);
    assert_eq!(next_ids(iter), sorted(pairs[1..].to_vec()));
}

/// Asserts that an entity reusing a deleted pair's index is yielded with its new generation
/// once regrouped, and the deleted one isn't.
#[test]
fn generation_reuse() {
    let mut world = World::new();
    let mut pairs = add_ab(&mut world, 10);
    world.group::<(A, B)>();
    world.delete_entity(pairs[0]);
    let reused = world.add_entity((A(99), B(99)));
    assert_eq!(reused.index(), pairs[0].index());
    assert_ne!(reused, pairs[0]);
    pairs[0] = reused;
    world.regroup();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();

    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [segment((0, 0..10), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_eq!(next_ids(iter), sorted(pairs));
}

/// Asserts that an empty grouped pair ties with the ordinary walk, which yields nothing.
#[test]
fn empty() {
    let mut world = World::new();
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();

    assert!(iter.schedule.is_none());
    assert_eq!(next_ids(iter), []);
}

/// Asserts that a group of zero sized components is walked aligned like any other.
#[test]
fn zero_sized() {
    let mut world = World::new();
    let pairs: Vec<EntityId> = (0..4).map(|_| world.add_entity((Zst0, Zst1))).collect();
    world.add_entity(Zst0);
    world.add_entity(Zst1);
    world.group::<(Zst0, Zst1)>();

    let (z0, z1) = world.borrow::<(View<'_, Zst0>, View<'_, Zst1>)>().unwrap();
    let iter = (&z0, &z1).iter();

    assert_eq!(
        segments(&iter, &[&z0.dense, &z1.dense]),
        [segment((0, 0..4), 0b11, &[Some(0), Some(0)], &[])]
    );
    assert_eq!(next_ids(iter), sorted(pairs.clone()));
    assert_eq!(fold_ids((&z0, &z1).iter()), sorted(pairs));
}

/// Asserts that a grouped pair nested in a query drives it when that's cheaper:
/// the parent drives through the pair alone and looks C up. `grouping_nested` goes further.
#[test]
fn nested() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    world.add_entity(A(10));
    for &id in &pairs {
        world.add_component(id, C(0));
    }
    world.add_entity(C(10));
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = ((&a, &b), &c).iter();

    assert_eq!(iter.shiperator.captain_mask, 0b01);
    assert_eq!(iter.shiperator.shiperator.0.captain_mask, 0b11);
    assert_eq!(iter.cursor.len(), 4);
    assert!(!iter.is_exact_sized);
    assert_eq!(next_ids(iter), sorted(pairs));
}

/// Asserts that a filter or an `Or` in a query over a grouped pair keeps the ordinary walk.
#[test]
fn filtered() {
    let mut world = World::new();
    let pairs = add_ab(&mut world, 4);
    world.add_entity(A(10));
    world.add_component(pairs[0], C(0));
    world.add_entity(D(0));
    world.group::<(A, B)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let not = (&a, &b, !&c).iter();
    assert!(not.schedule.is_none());
    assert_eq!(not.shiperator.captain_mask.count_ones(), 1);
    assert_eq!(next_ids(not), sorted(pairs[1..].to_vec()));

    // The `Or` is the smallest captain. Its follow-up is its switch to D, not a grouped walk.
    let or = (&a, &b, &c | &d).iter();
    assert_eq!(or.shiperator.captain_mask, 0b100);
    assert_eq!(next_ids(or), [pairs[0]]);
}

/// Asserts that `World::iter` installs the grouped walks once and every iterator replays
/// them from the first segment.
#[test]
fn owned() {
    let (world, pairs) = follow_ups_world();
    let mut iter_ref = world.iter::<(&A, &B)>();

    assert_eq!(iter_ref.shiperator.captain_mask, 0b11);
    let schedule = iter_ref.entities.schedule.as_ref().unwrap();
    assert_eq!(schedule.follow_ups().len(), 2);

    assert_eq!(next_ids(iter_ref.iter()), sorted(pairs.clone()));
    assert_eq!(fold_ids(iter_ref.iter()), sorted(pairs.clone()));
    assert_eq!(next_ids(iter_ref.iter()), sorted(pairs));
}

/// Asserts that splitting an automatically installed plan and consuming the right half first
/// yields every pair once, the shared plan living until both halves are gone.
#[test]
fn split_right_first() {
    let (world, pairs) = follow_ups_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (left, right) = (&a, &b).iter().split_current_segment();
    let right = right.expect("the aligned segment has 10 candidates");
    let plan = left.schedule.as_ref().unwrap().downgrade();

    let right_ids = next_ids(right);
    assert_eq!(right_ids.len(), 6);
    assert!(plan.upgrade().is_some());

    let left_ids = next_ids(left);
    assert_eq!(left_ids.len(), 5);
    assert!(plan.upgrade().is_none());

    assert_eq!(sorted([left_ids, right_ids].concat()), sorted(pairs));
}
