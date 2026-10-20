//! Grouped walks driven through nested `Mixed`s, on worlds grouped by `group` and `regroup`.
//!
//! Iterators come from `iter` and `World::iter`, nothing is installed by hand.\
//! We check which path won and the full rows it installed, then compare the items with
//! the entities we know own every component.

use super::grouping_regions::{record, walked};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::plan::{Coverage, NodeState, SegmentDescriptor};
use crate::iter::planner::{NodeMetadata, Route};
use crate::iter::{IntoIter, IntoShiperator, SegmentRow, Shiper, Shiperator};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

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

#[derive(Debug, PartialEq)]
struct Modif(u32);
impl Component for Modif {
    type Tracking = track::Modification;
}

/// State of a node as these tests compare it, coverage records resolved.
#[derive(Clone, Debug, PartialEq)]
enum State {
    Sailor,
    Window(usize),
    Mixed(u32, Vec<Coverage>),
}

use State::Sailor;

fn window(segment_start: usize) -> State {
    State::Window(segment_start)
}

fn mixed(captain_mask: u32) -> State {
    State::Mixed(captain_mask, Vec::new())
}

fn covered(captain_mask: u32, coverage: &[Coverage]) -> State {
    State::Mixed(captain_mask, coverage.to_vec())
}

/// A segment as these tests compare it.
#[derive(Debug, PartialEq)]
struct Segment {
    /// Position in `storages` of the storage holding the walked entities, and their indices in it.
    walks: (usize, Range<usize>),
    /// State of every node of the query, in preorder.
    row: Vec<State>,
}

fn segment(walks: (usize, Range<usize>), row: Vec<State>) -> Segment {
    Segment { walks, row }
}

/// Describes every segment of `iter`, walking one of `storages`.
///
/// A single segment is installed without plan, it's described from `iter`'s own state.
/// Otherwise, also asserts that `iter` was installed for the plan's first segment.
fn segments<S: Shiperator>(iter: &Shiper<S>, storages: &[&[EntityId]]) -> Vec<Segment> {
    let installed = SegmentRow::of(&iter.shiperator);
    let Some(schedule) = &iter.schedule else {
        // `describe` asserts that the installed state has no coverage.
        let segment = SegmentDescriptor::from_cursor(iter.cursor, 0);
        let row = installed.states().iter().map(|&state| state_of(state, &[]));

        return vec![Segment {
            walks: walked(&segment, storages),
            row: row.collect(),
        }];
    };
    let plan = schedule.plan();
    let first = &plan.segments()[0];

    assert_eq!(installed.states(), plan.row(first));
    assert_eq!(iter.cursor.as_ptr(), first.cursor().as_ptr());
    assert_eq!(schedule.follow_ups().len(), plan.segments().len() - 1);

    plan.segments()
        .iter()
        .map(|segment| Segment {
            walks: walked(segment, storages),
            row: plan
                .row(segment)
                .iter()
                .map(|&state| match state {
                    NodeState::Mixed { coverage, .. } => state_of(state, plan.coverage(coverage)),
                    state => state_of(state, &[]),
                })
                .collect(),
        })
        .collect()
}

/// Returns `state` as these tests compare it, a `Mixed` with the records of `coverage`.
fn state_of(state: NodeState, coverage: &[Coverage]) -> State {
    match state {
        NodeState::Sailor => Sailor,
        NodeState::Window { segment_start } => window(segment_start),
        NodeState::Mixed { captain_mask, .. } => covered(captain_mask, coverage),
        NodeState::Or { .. } => unreachable!("these queries don't have Or"),
    }
}

fn sorted<T: Ord>(mut items: Vec<T>) -> Vec<T> {
    items.sort_unstable();
    items
}

/// Asserts that every way of consuming the iterators `iter` builds yields `expected`,
/// each entity with its value, `next` prefixes followed by bulk consumption included.
///
/// `expected` is sorted by id, items can come in any order but all ways have to agree.
fn assert_every_way<S, T, F>(iter: F, value: fn(S::Out) -> T, expected: &[(EntityId, T)])
where
    S: Shiperator,
    F: Fn() -> Shiper<S>,
    T: Clone + Debug + Ord,
{
    let with_ids: Vec<(EntityId, T)> = iter()
        .with_id()
        .map(|(id, item)| (id, value(item)))
        .collect();
    let values: Vec<T> = with_ids.iter().map(|(_, value)| value.clone()).collect();
    assert_eq!(sorted(with_ids.clone()), expected);

    let mut next = Vec::new();
    for item in iter() {
        next.push(value(item));
    }
    assert_eq!(next, values, "next");

    let mut for_each = Vec::new();
    iter().for_each(|item| for_each.push(value(item)));
    assert_eq!(for_each, values, "for_each");

    let mut id_for_each = Vec::new();
    iter()
        .with_id()
        .for_each(|(id, item)| id_for_each.push((id, value(item))));
    assert_eq!(id_for_each, with_ids, "WithId for_each");

    // Prefixes end before, inside, at the end and past the end of the items.
    // Under Miri, every traversal is slow, one prefix of each kind is enough.
    let prefixes: Vec<usize> = if cfg!(miri) {
        vec![0, with_ids.len() / 2, with_ids.len(), with_ids.len() + 1]
    } else {
        (0..=with_ids.len() + 1).collect()
    };
    for prefix in prefixes {
        let mut plain = iter();
        let head: Vec<T> = (&mut plain).take(prefix).map(value).collect();
        let fold = plain.fold(head, |mut acc, item| {
            acc.push(value(item));
            acc
        });
        assert_eq!(fold, values, "fold after {prefix}");

        let mut ids = iter().with_id();
        let head: Vec<(EntityId, T)> = (&mut ids)
            .take(prefix)
            .map(|(id, item)| (id, value(item)))
            .collect();
        let fold = ids.fold(head, |mut acc, (id, item)| {
            acc.push((id, value(item)));
            acc
        });
        assert_eq!(fold, with_ids, "WithId fold after {prefix}");
    }
}

/// Adds `count` entities owning `A(start + i)` and `B(start + i)`.
fn add_ab(world: &mut World, start: u32, count: u32) -> Vec<EntityId> {
    (start..start + count)
        .map(|i| world.add_entity((A(i), B(i))))
        .collect()
}

/// Adds `C(i)` to the entity of each `(i, id)`.
fn add_c(world: &mut World, ids: &[EntityId]) {
    for (i, &id) in (0..).zip(ids) {
        world.add_component(id, C(i));
    }
}

/// Adds `count` entities only owning C.
fn add_lone_c(world: &mut World, count: u32) {
    for i in 0..count {
        world.add_entity(C(1000 + i));
    }
}

/// `grouped` pairs are grouped, then `late` more pairs are added.
///
/// The pairs' grouped walks are the aligned pairs, then A's and B's unevaluated regions,
/// B's covered by A's.\
/// Returns the pairs, the late ones last.
fn late_pairs_world(grouped: u32, late: u32) -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut pairs = add_ab(&mut world, 0, grouped);
    world.group::<(A, B)>();
    pairs.extend(add_ab(&mut world, grouped, late));

    (world, pairs)
}

/// Returns `ids` owning A, B and C with their values, looked up one by one and sorted by id.
fn abc_values(
    ids: &[EntityId],
    a: &View<'_, A>,
    b: &View<'_, B>,
    c: &View<'_, C>,
) -> Vec<(EntityId, (u32, u32, u32))> {
    let values = ids
        .iter()
        .filter(|&&id| a.contains(id) && b.contains(id) && c.contains(id))
        .map(|&id| {
            let values = (
                a.get(id).unwrap().0,
                b.get(id).unwrap().0,
                c.get(id).unwrap().0,
            );

            (id, values)
        })
        .collect();

    sorted(values)
}

/// 10 pairs all owning C, grouped, and 10 entities only owning C.
///
/// The pair's ordinary walk costs 30 and looking C up 10 more. Its aligned walk costs 20.\
/// Returns the pairs.
fn large_c_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 0, 10);
    add_c(&mut world, &pairs);
    add_lone_c(&mut world, 10);
    world.group::<(A, B)>();

    (world, pairs)
}

/// Asserts that with a large C, the grouped pair drives `((A, B), C)`: the root drives through
/// the pair, the pair drives with both members and C is looked up. The pair would also be
/// the root's ordinary captain, the grouped walk replaces its ordinary one.
#[test]
fn large_c() {
    let (world, pairs) = large_c_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let (_, _, metadata) = ((&a, &b), &c).into_shiperator();
    let NodeMetadata::GroupedMixed(selection) = &metadata.0 else {
        panic!("a Mixed with grouped alternatives reports its selection");
    };
    assert_eq!(selection.ordinary.captain, 0);
    assert_eq!(selection.ordinary.work, Some(40));
    let winner = selection.winner().unwrap();
    assert!(matches!(winner.route, Route::Nested { child: 0, .. }));
    assert_eq!((winner.candidates, winner.work), (10, 30));

    let iter = ((&a, &b), &c).iter();
    assert_eq!(
        segments(&iter, &[&a.dense]),
        [segment(
            (0, 0..10),
            vec![mixed(0b01), mixed(0b11), window(0), window(0), Sailor]
        )]
    );
    assert!(!iter.is_exact_sized);
    assert_eq!(iter.size_hint(), (0, Some(10)));

    assert_every_way(
        || ((&a, &b), &c).iter(),
        |((a, b), c)| (a.0, b.0, c.0),
        &abc_values(&pairs, &a, &b, &c),
    );
}

/// Asserts that with a tiny C, the ordinary walk of `((A, B), C)` wins: C drives and looks
/// the pair up, the pair keeping its ordinary state.
#[test]
fn tiny_c() {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 0, 10);
    add_c(&mut world, &pairs[3..5]);
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let (_, _, metadata) = ((&a, &b), &c).into_shiperator();
    let NodeMetadata::GroupedMixed(selection) = &metadata.0 else {
        panic!("a Mixed with grouped alternatives reports its selection");
    };
    // The pair still reported its aligned walk, which costs 30 here against 8.
    assert_eq!(selection.cheapest().unwrap().work, 30);
    assert_eq!(selection.ordinary.work, Some(8));
    assert!(selection.winner().is_none());

    let iter = ((&a, &b), &c).iter();
    assert_eq!(iter.shiperator.captain_mask, 0b10);
    assert_eq!(iter.shiperator.shiperator.0.captain_mask, 0b01);
    assert!(iter.schedule.is_none());

    assert_every_way(
        || ((&a, &b), &c).iter(),
        |((a, b), c)| (a.0, b.0, c.0),
        &abc_values(&pairs, &a, &b, &c),
    );
}

/// Asserts that the grouped pair drives `(C, (A, B))` from the second position and that
/// items keep the query's structure, each value belonging to the yielded entity.
#[test]
fn second_position() {
    let (world, pairs) = large_c_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let iter = (&c, (&a, &b)).iter();
    assert_eq!(
        segments(&iter, &[&a.dense]),
        [segment(
            (0, 0..10),
            vec![mixed(0b10), Sailor, mixed(0b11), window(0), window(0)]
        )]
    );

    assert_every_way(
        || (&c, (&a, &b)).iter(),
        |(c, (a, b))| (a.0, b.0, c.0),
        &abc_values(&pairs, &a, &b, &c),
    );
}

/// Asserts that a pair two levels down drives `(((B, A), C), D)` through every `Mixed` above it,
/// with its members in the reverse of the group's order.
#[test]
fn deeper() {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 0, 10);
    for (i, &id) in (0..).zip(&pairs) {
        world.add_component(id, (C(i), D(i)));
    }
    add_lone_c(&mut world, 10);
    for i in 0..10 {
        world.add_entity(D(1000 + i));
    }
    world.group::<(A, B)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let iter = (((&b, &a), &c), &d).iter();
    // D is the root's ordinary captain, the pair's 10 candidates win over its 20.
    assert_eq!(
        segments(&iter, &[&b.dense]),
        [segment(
            (0, 0..10),
            vec![
                mixed(0b01),
                mixed(0b01),
                mixed(0b11),
                window(0),
                window(0),
                Sailor,
                Sailor
            ]
        )]
    );

    let expected: Vec<_> = pairs
        .iter()
        .map(|&id| (id, (a.get(id).unwrap().0, d.get(id).unwrap().0)))
        .collect();
    assert_every_way(
        || (((&b, &a), &c), &d).iter(),
        |(((b, a), c), d)| {
            assert_eq!((a.0, b.0), (c.0, d.0));
            (a.0, d.0)
        },
        &sorted(expected),
    );
}

/// Asserts that the grouped pair drives from the middle of `(C, (A, B), D)`.
#[test]
fn middle_position() {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 0, 10);
    for (i, &id) in (0..).zip(&pairs) {
        world.add_component(id, (C(i), D(i)));
    }
    add_lone_c(&mut world, 10);
    world.group::<(A, B)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let iter = (&c, (&a, &b), &d).iter();
    assert_eq!(
        segments(&iter, &[&a.dense]),
        [segment(
            (0, 0..10),
            vec![
                mixed(0b010),
                Sailor,
                mixed(0b11),
                window(0),
                window(0),
                Sailor
            ]
        )]
    );

    assert_every_way(
        || (&c, (&a, &b), &d).iter(),
        |(c, (a, b), d)| {
            assert_eq!((a.0, b.0), (c.0, d.0));
            a.0
        },
        &sorted(pairs.iter().map(|&id| (id, a.get(id).unwrap().0)).collect()),
    );
}

/// 20 entities own A, B, C and D, 30 only A and B. `late` gains C and D after AB and CD are
/// grouped, it's in their unevaluated regions.
///
/// Returns the 21 entities owning every component.
fn disjoint_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut all: Vec<EntityId> = (0..20)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i))))
        .collect();
    add_ab(&mut world, 100, 30);
    let late = world.add_entity((A(50), B(50)));
    world.group::<(A, B)>();
    world.group::<(C, D)>();
    world.add_component(late, (C(50), D(50)));
    all.push(late);

    (world, all)
}

/// Asserts that with disjoint AB and CD groups, `((A, B), (C, D))` drives through the CD pair's
/// complete walks, coverage included, and looks the AB pair up as a whole.
///
/// The CD walks cost 90 with the AB lookups, against 105 for CD's ordinary walk and 204 for
/// the AB walks.
#[test]
fn disjoint_cd() {
    let (world, all) = disjoint_world();
    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let iter = ((&a, &b), (&c, &d)).iter();
    let ab_sailors = || vec![Sailor, Sailor, Sailor];
    assert_eq!(
        segments(&iter, &[&c.dense, &d.dense]),
        [
            segment(
                (0, 0..20),
                [
                    vec![mixed(0b10)],
                    ab_sailors(),
                    vec![mixed(0b11), window(0), window(0)]
                ]
                .concat()
            ),
            segment(
                (0, 20..21),
                [
                    vec![mixed(0b10)],
                    ab_sailors(),
                    vec![mixed(0b01), window(20), Sailor]
                ]
                .concat()
            ),
            segment(
                (1, 20..21),
                [
                    vec![mixed(0b10)],
                    ab_sailors(),
                    vec![covered(0b10, &[record(0, 20, 21)]), Sailor, window(20)]
                ]
                .concat()
            ),
        ]
    );

    assert_every_way(
        || ((&a, &b), (&c, &d)).iter(),
        |((a, b), (c, d))| {
            assert_eq!((a.0, b.0), (c.0, d.0));
            a.0
        },
        &sorted(all.iter().map(|&id| (id, a.get(id).unwrap().0)).collect()),
    );
}

/// Asserts that with disjoint AB and CD groups, `((A, B), (C, D))` drives through the aligned
/// AB pair when it's cheaper and still finds the entity whose C and D are unevaluated
/// through the CD pair's lookups.
#[test]
fn disjoint_ab() {
    let mut world = World::new_for_test();
    let mut all: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i))))
        .collect();
    let late = world.add_entity((A(4), B(4)));
    world.group::<(A, B)>();
    world.group::<(C, D)>();
    world.add_component(late, (C(4), D(4)));
    all.push(late);

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let iter = ((&a, &b), (&c, &d)).iter();
    // 20 for the AB walk against 25 for AB's ordinary walk and 26 for the CD walks.
    assert_eq!(
        segments(&iter, &[&a.dense]),
        [segment(
            (0, 0..5),
            vec![
                mixed(0b01),
                mixed(0b11),
                window(0),
                window(0),
                Sailor,
                Sailor,
                Sailor
            ]
        )]
    );

    assert_every_way(
        || ((&a, &b), (&c, &d)).iter(),
        |((a, b), (c, d))| {
            assert_eq!((a.0, b.0), (c.0, d.0));
            a.0
        },
        &sorted(all.iter().map(|&id| (id, a.get(id).unwrap().0)).collect()),
    );
}

/// Asserts that an AC group doesn't give `((A, B), C)` any grouped alternative:
/// it would cross the nested `Mixed`'s boundary.
#[test]
fn cross_boundary() {
    let mut world = World::new_for_test();
    let all: Vec<EntityId> = (0..10)
        .map(|i| world.add_entity((A(i), B(i), C(i))))
        .collect();
    world.group::<(A, C)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    // Both `Mixed`s report their ordinary walk alone, without grouped alternative.
    let (_, _, inner_metadata) = (&a, &b).into_shiperator();
    assert!(matches!(inner_metadata.0, NodeMetadata::Mixed { .. }));

    let (_, _, metadata) = ((&a, &b), &c).into_shiperator();
    assert!(matches!(metadata.0, NodeMetadata::Mixed { .. }));

    let iter = ((&a, &b), &c).iter();
    assert!(iter.schedule.is_none());

    assert_every_way(
        || ((&a, &b), &c).iter(),
        |((a, b), c)| (a.0, b.0, c.0),
        &abc_values(&all, &a, &b, &c),
    );
}

/// Asserts that a nested pair with entities added after the regroup drives through its
/// unevaluated walks, the pair keeping the coverage that rejects duplicates while the root
/// has none. Every pair owns C, each one is yielded once.
///
/// The walks cost 46 against 48 for C captaining.
#[test]
fn coverage() {
    let (mut world, pairs) = late_pairs_world(10, 2);
    add_c(&mut world, &pairs);

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let iter = ((&a, &b), &c).iter();
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [
            segment(
                (0, 0..10),
                vec![mixed(0b01), mixed(0b11), window(0), window(0), Sailor]
            ),
            segment(
                (0, 10..12),
                vec![mixed(0b01), mixed(0b01), window(10), Sailor, Sailor]
            ),
            segment(
                (1, 10..12),
                vec![
                    mixed(0b01),
                    covered(0b10, &[record(0, 10, 12)]),
                    Sailor,
                    window(10),
                    Sailor
                ]
            ),
        ]
    );
    assert_eq!(iter.size_hint(), (0, Some(14)));

    assert_every_way(
        || ((&a, &b), &c).iter(),
        |((a, b), c)| (a.0, b.0, c.0),
        &abc_values(&pairs, &a, &b, &c),
    );
}

/// Asserts that once removing A's logical 0 leaves its range untrusted, the nested walk ties
/// with the ordinary one and loses, then wins again after a regroup.
#[test]
fn removal_then_regroup() {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 0, 10);
    add_c(&mut world, &pairs);
    add_lone_c(&mut world, 10);
    world.group::<(A, B)>();
    world.remove::<A>(pairs[0]);

    {
        let (a, b, c) = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();

        // A's 9 components are still in its range, but none is trusted:
        // 27 for the pair's damaged walk and 9 more for C, as much as the pair's ordinary walk.
        // The damaged walk would install the same state, only the selection tells them apart.
        let (_, _, metadata) = ((&a, &b), &c).into_shiperator();
        let NodeMetadata::GroupedMixed(selection) = &metadata.0 else {
            panic!("a Mixed with grouped alternatives reports its selection");
        };
        assert_eq!(
            selection.cheapest().unwrap().work,
            selection.ordinary.work.unwrap()
        );
        assert!(selection.winner().is_none());

        let iter = ((&a, &b), &c).iter();
        assert!(iter.schedule.is_none());
        assert_eq!(iter.shiperator.captain_mask, 0b01);

        assert_every_way(
            || ((&a, &b), &c).iter(),
            |((a, b), c)| (a.0, b.0, c.0),
            &abc_values(&pairs, &a, &b, &c),
        );
    }

    world.regroup();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = ((&a, &b), &c).iter();
    assert_eq!(
        segments(&iter, &[&a.dense]),
        [segment(
            (0, 0..9),
            vec![mixed(0b01), mixed(0b11), window(0), window(0), Sailor]
        )]
    );

    assert_every_way(
        || ((&a, &b), &c).iter(),
        |((a, b), c)| (a.0, b.0, c.0),
        &abc_values(&pairs, &a, &b, &c),
    );
}

/// Asserts that when C wins over a pair with unevaluated walks, the pair answers C's lookups
/// as a sailor: it finds the late pairs, without any coverage of its own.
#[test]
fn sailor_membership() {
    let (mut world, pairs) = late_pairs_world(10, 2);
    add_c(&mut world, &pairs[9..]);

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let iter = (&c, (&a, &b)).iter();
    assert!(iter.schedule.is_none());
    assert_eq!(iter.shiperator.captain_mask, 0b01);
    assert_eq!(iter.shiperator.shiperator.1.coverage.len(), 0);

    assert_every_way(
        || (&c, (&a, &b)).iter(),
        |(c, (a, b))| (a.0, b.0, c.0),
        &abc_values(&pairs, &a, &b, &c),
    );
}

/// Asserts that in `((A, B), (A, B))`, the first pair drives with its coverage while the
/// second one, over the same storages, answers lookups for every candidate, covered or not.
#[test]
fn repeated_pair() {
    let (world, pairs) = late_pairs_world(20, 1);
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let iter = ((&a, &b), (&a, &b)).iter();
    let sailors = || vec![Sailor, Sailor, Sailor];
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [
            segment(
                (0, 0..20),
                [
                    vec![mixed(0b01), mixed(0b11), window(0), window(0)],
                    sailors()
                ]
                .concat()
            ),
            segment(
                (0, 20..21),
                [
                    vec![mixed(0b01), mixed(0b01), window(20), Sailor],
                    sailors()
                ]
                .concat()
            ),
            segment(
                (1, 20..21),
                [
                    vec![
                        mixed(0b01),
                        covered(0b10, &[record(0, 20, 21)]),
                        Sailor,
                        window(20)
                    ],
                    sailors()
                ]
                .concat()
            ),
        ]
    );

    assert_every_way(
        || ((&a, &b), (&a, &b)).iter(),
        |((a0, b0), (a1, b1))| {
            assert!(core::ptr::eq(a0, a1) && core::ptr::eq(b0, b1));
            a0.0
        },
        &sorted(pairs.iter().map(|&id| (id, a.get(id).unwrap().0)).collect()),
    );
}

/// 10 pairs are grouped with `lone` owning A, then `lone` gains B and 2 entities only owning A
/// are added.
///
/// The pair walks its aligned prefix, then A's unevaluated region where no candidate owns B,
/// then B's where only `lone` is left. Only 3 aligned pairs and `lone` own C, along with
/// 40 entities owning only C.\
/// The walks cost 42 against 44 for the pair's ordinary walk.
///
/// Returns `lone`, followed by the aligned pairs.
fn sparse_c_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 0, 10);
    let lone = world.add_entity(A(10));
    world.group::<(A, B)>();
    world.add_component(lone, B(10));
    world.add_entity(A(11));
    world.add_entity(A(12));
    add_c(&mut world, &pairs[..3]);
    world.add_component(lone, C(10));
    add_lone_c(&mut world, 40);

    (world, [vec![lone], pairs].concat())
}

/// Asserts that nested segments whose candidates fail the root's lookups, or all of their own,
/// yield nothing while later segments still do, however a `next` prefix stops.
#[test]
fn empty_segments() {
    let (world, ids) = sparse_c_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let iter = ((&a, &b), &c).iter();
    assert_eq!(
        segments(&iter, &[&a.dense, &b.dense]),
        [
            segment(
                (0, 0..10),
                vec![mixed(0b01), mixed(0b11), window(0), window(0), Sailor]
            ),
            segment(
                (0, 11..13),
                vec![mixed(0b01), mixed(0b01), window(11), Sailor, Sailor]
            ),
            segment(
                (1, 10..11),
                vec![
                    mixed(0b01),
                    covered(0b10, &[record(0, 11, 13)]),
                    Sailor,
                    window(10),
                    Sailor
                ]
            ),
        ]
    );

    let expected = abc_values(&ids, &a, &b, &c);
    assert_eq!(expected.len(), 4);
    assert_every_way(
        || ((&a, &b), &c).iter(),
        |((a, b), c)| (a.0, b.0, c.0),
        &expected,
    );
}

/// Asserts that mutable and modification tracked outputs of a nested group can all be held
/// at once, each write landing once on its own entity and flagging only the matches.
#[test]
fn held_mut() {
    let mut world = World::new_for_test();
    let all: Vec<EntityId> = (0..6)
        .map(|i| world.add_entity((A(i), Modif(i), C(i))))
        .collect();
    let lone_a = world.add_entity(A(100));
    world.add_entity(A(101));
    add_lone_c(&mut world, 10);
    world.group::<(A, Modif)>();

    let (mut a, mut modifs, mut c) = world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, Modif>, ViewMut<'_, C>)>()
        .unwrap();

    let iter = ((&mut a, &mut modifs), &mut c).iter();
    assert_eq!(iter.shiperator.captain_mask, 0b01);
    assert_eq!(iter.shiperator.shiperator.0.captain_mask, 0b11);

    let mut items: Vec<_> = iter.collect();
    assert_eq!(items.len(), 6);
    for ((a, modif), c) in &mut items {
        a.0 += 100;
        modif.0 += 200;
        c.0 += 300;
    }
    drop(items);

    for (i, &id) in (0..).zip(&all) {
        assert_eq!((&a).get(id).unwrap().0, 100 + i);
        assert_eq!((&modifs).get(id).unwrap().0, 200 + i);
        assert_eq!((&c).get(id).unwrap().0, 300 + i);
        assert!(modifs.is_modified(id));
    }
    assert_eq!((&a).get(lone_a).unwrap().0, 100);
}

/// Asserts that `World::iter` installs the nested walks once and every iterator replays them
/// from the first segment.
#[test]
fn owned() {
    let (mut world, pairs) = late_pairs_world(10, 2);
    add_c(&mut world, &pairs);

    let expected = {
        let (a, b, c) = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();

        abc_values(&pairs, &a, &b, &c)
    };

    let mut iter_ref = world.iter::<((&A, &B), &C)>();
    assert_eq!(iter_ref.shiperator.captain_mask, 0b01);
    assert_eq!(iter_ref.shiperator.shiperator.0.captain_mask, 0b11);
    let schedule = iter_ref.entities.schedule.as_ref().unwrap();
    assert_eq!(schedule.follow_ups().len(), 2);

    for _ in 0..2 {
        let next: Vec<_> = iter_ref
            .iter()
            .with_id()
            .map(|(id, ((a, b), c))| (id, (a.0, b.0, c.0)))
            .collect();
        assert_eq!(sorted(next), expected);

        let fold = iter_ref
            .iter()
            .with_id()
            .fold(Vec::new(), |mut acc, (id, ((a, b), c))| {
                acc.push((id, (a.0, b.0, c.0)));
                acc
            });
        assert_eq!(sorted(fold), expected);
    }
}

/// Asserts that splitting a nested plan's aligned segment and consuming the right half first
/// yields every pair once: the right half's covered follow-up rejects the late pairs A's walk
/// already gave it. The plan lives until both halves are gone.
#[test]
fn split_right_first() {
    let (mut world, pairs) = late_pairs_world(10, 2);
    add_c(&mut world, &pairs);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let (left, right) = ((&a, &b), &c).iter().split_current_segment();
    let right = right.expect("the aligned segment has 10 candidates");
    let plan = left.schedule.as_ref().unwrap().downgrade();

    let right_ids: Vec<EntityId> = right.with_id().map(|(id, _)| id).collect();
    assert_eq!(right_ids.len(), 7);
    assert!(plan.upgrade().is_some());

    let left_ids: Vec<EntityId> = left.with_id().map(|(id, _)| id).collect();
    assert_eq!(left_ids.len(), 5);
    assert!(plan.upgrade().is_none());

    assert_eq!(sorted([left_ids, right_ids].concat()), sorted(pairs));
}

/// Asserts that after splitting a nested plan's aligned segment, dropping the left half first
/// leaves the right half's plan and coverage alive for its follow-ups.
#[test]
fn split_left_dropped() {
    let (mut world, pairs) = late_pairs_world(10, 2);
    add_c(&mut world, &pairs);
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let (left, right) = ((&a, &b), &c).iter().split_current_segment();
    let right = right.expect("the aligned segment has 10 candidates");
    drop(left);

    let right_ids = right.with_id().fold(Vec::new(), |mut ids, (id, _)| {
        ids.push(id);
        ids
    });

    // The right half walks A's storage from its 6th component on.
    assert_eq!(sorted(right_ids), sorted(a.dense[5..].to_vec()));
    assert_eq!(a.dense.len(), pairs.len());
}

/// Asserts that `ring_world`'s pair, walking AB and ABC rings cut at wrap points then its
/// unevaluated regions, drives `((A, B), C)` from inside the nested `Mixed`: the root drives
/// every segment through the pair without coverage and looks C up, the pair keeps its own
/// per-segment masks and coverage. Every way yields every match once, and mutable outputs
/// held all at once each write once.
#[test]
fn ring_nested() {
    use super::grouping_fixtures::{A, B, C, ring_world};

    let (world, pairs) = ring_world();
    {
        let (a, b, c) = world
            .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
            .unwrap();
        let iter = ((&a, &b), &c).iter();

        let segments = segments(&iter, &[&a.dense, &b.dense]);
        let pair_masks: Vec<u32> = segments
            .iter()
            .map(|segment| {
                assert_eq!(segment.row[0], mixed(0b01));
                assert_eq!(segment.row[4], Sailor);
                let State::Mixed(mask, _) = segment.row[1] else {
                    panic!("the pair drives");
                };
                mask
            })
            .collect();
        assert_eq!(pair_masks, [0b11, 0b11, 0b11, 0b11, 0b01, 0b10]);
        let State::Mixed(_, coverage) = &segments[5].row[1] else {
            panic!("the pair drives");
        };
        assert_eq!(coverage.len(), 1);

        let expected: Vec<_> = sorted(
            pairs
                .iter()
                .filter(|&&id| c.contains(id))
                .map(|&id| {
                    let values = (
                        a.get(id).unwrap().0,
                        b.get(id).unwrap().0,
                        c.get(id).unwrap().0,
                    );

                    (id, values)
                })
                .collect(),
        );
        assert_eq!(expected.len(), 7);
        assert_every_way(
            || ((&a, &b), &c).iter(),
            |((a, b), c)| (a.0, b.0, c.0),
            &expected,
        );
    }

    let (mut a, mut b, mut c) = world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
        .unwrap();
    let items: Vec<_> = ((&mut a, &mut b), &mut c).iter().collect();
    assert_eq!(items.len(), 7);
    for ((a, b), c) in items {
        a.0 += 1000;
        b.0 += 1000;
        c.0 += 1000;
    }
    for (i, &id) in (0..).zip(&pairs) {
        let delta = if c.contains(id) { 1000 } else { 0 };
        assert_eq!((&a).get(id).unwrap().0, i + delta);
        assert_eq!((&b).get(id).unwrap().0, 100 + i + delta);
    }
}
