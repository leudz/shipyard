//! Grouped walks the normal constructors pick and install, on worlds grouped by `group`
//! and `regroup`.
//!
//! Iterators come from `iter` and `World::iter`, nothing is installed by hand.\
//! We check which walk won and the segments it installed, then compare the items with
//! the entities we know own every component.

use super::grouping_regions::{assert_skipped, record, segment, segments};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{IntoIter, Shiper, Shiperator};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
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
