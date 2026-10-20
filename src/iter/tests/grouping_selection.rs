//! Selection between ordinary and grouped walks, on worlds grouped by `group` and `regroup`.
//!
//! Queries are built through their real constructors. We read the selection a `Mixed`
//! reports in its construction metadata, constructors always install the ordinary walk.\
//! Installing the winner is tested in `grouping_automatic`.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::planner::{
    Alternative, ConstructionMetadata, GroupedCandidate, NodeMetadata, Route, Selection, Walk,
};
use crate::iter::{EntityCursor, IntoIter, IntoShiperator, IterComponent, Optional, Shiperator};
use crate::sparse_set::{Ring, SparseSet};
use crate::storage::StorageId;
use crate::track;
use crate::tracking::TrackingTimestamp;
use crate::views::{AllStoragesView, View, ViewMut};
use crate::world::World;
use alloc::sync::Arc;
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

#[derive(Debug, PartialEq)]
struct E(u32);
impl Component for E {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct F(u32);
impl Component for F {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct G(u32);
impl Component for G {
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

/// Returns the selection a `Mixed` reported, `None` when it had no grouped alternative.
fn selection(metadata: &ConstructionMetadata) -> Option<&Selection> {
    match &metadata.0 {
        NodeMetadata::GroupedMixed(selection) => Some(selection),
        _ => None,
    }
}

/// Asserts that a `Mixed` reported no grouped alternative, without filter.
fn assert_ungrouped(metadata: &ConstructionMetadata) {
    assert!(matches!(metadata.0, NodeMetadata::Mixed { .. }));
}

/// Asserts that the cheapest grouped alternative of `metadata` wins with the members
/// `member_mask` flags.
fn assert_grouped_wins(metadata: &ConstructionMetadata, member_mask: u32) {
    let selection = selection(metadata).expect("a grouped alternative");
    let winner = selection.winner().expect("the grouped alternative wins");

    assert_eq!(selection.group_owner(winner).1, member_mask);
}

/// Asserts that `metadata` reports a grouped alternative but its ordinary walk with
/// child `captain` wins.
fn assert_ordinary_wins(metadata: &ConstructionMetadata, captain: usize) {
    let selection = selection(metadata).expect("a grouped alternative");

    assert!(selection.winner().is_none());
    assert_eq!(selection.ordinary.captain, captain);
}

/// Returns the cheapest grouped alternative of `metadata`.
fn cheapest(metadata: &ConstructionMetadata) -> &Alternative {
    selection(metadata)
        .expect("a grouped alternative")
        .cheapest()
        .unwrap()
}

/// Returns the walk over the first `len` positions of a group's only range, every child
/// `captains` flags reading its component directly.
fn aligned_walk(len: usize, captains: u32) -> Walk {
    Walk::Aligned {
        range: 0,
        start: 0,
        len,
        captains,
    }
}

/// Returns the walks of the cheapest grouped alternative `query` reports.
fn cheapest_candidate<Q: IntoShiperator>(query: Q) -> GroupedCandidate
where
    Q::Shiperator: Shiperator,
{
    let (_root, _, metadata) = query.into_shiperator();
    let selection = selection(&metadata).expect("a grouped alternative");

    // SAFE: `_root` keeps the storages `query` borrows borrowed, with their group metadata
    // unmodified, until we return.
    unsafe { selection.candidate(selection.cheapest().unwrap()) }
}

/// Returns the walks of the cheapest grouped alternative `query` reports.
fn walks<Q: IntoShiperator>(query: Q) -> Vec<Walk>
where
    Q::Shiperator: Shiperator,
{
    cheapest_candidate(query).walks().collect()
}

/// Returns the candidates and work of each grouped alternative of `metadata`, in order.
fn estimates(metadata: &ConstructionMetadata) -> Vec<(usize, u64)> {
    selection(metadata)
        .expect("a grouped alternative")
        .alternatives
        .iter()
        .map(|alternative| (alternative.candidates, alternative.work))
        .collect()
}

/// Adds `count` entities owning `A(i)` and `B(i)`.
fn add_ab(world: &mut World, count: u32) -> Vec<EntityId> {
    (0..count).map(|i| world.add_entity((A(i), B(i)))).collect()
}

/// Asserts that a window over a grouped storage reports its group, its own storage id
/// and the regions of the storage, borrowed from the storage's own metadata.
#[test]
fn window_metadata() {
    let mut world = World::new_for_test();
    add_ab(&mut world, 3);
    world.add_entity(A(10));
    world.group::<(A, B)>();
    world.add_entity(A(11));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, metadata) = (&a).into_shiperator();

    let NodeMetadata::Window(handle) = metadata.0 else {
        panic!("a grouped window reports its group");
    };
    // SAFE: `a` keeps A borrowed, and its group metadata unmodified, until the end of the test.
    let window = unsafe { handle.get(EntityCursor::empty()) };
    assert!(core::ptr::eq(window.groups, &a.groups));
    assert_eq!(window.storage_id(), StorageId::of::<SparseSet<A>>());
    assert_eq!((window.groups.unevaluated_start, window.len), (4, 5));
    let [range] = &window.groups.ranges[..] else {
        panic!("A holds a single range");
    };
    assert!(Arc::ptr_eq(&range.group, &b.groups.ranges[0].group));
    assert_eq!(
        (range.ring(), range.trust),
        (
            Ring {
                start: 0,
                len: 3,
                shift: 0
            },
            3
        )
    );
}

/// Asserts that a window over a storage in two overlapping groups reports each of its ranges.
#[test]
fn window_metadata_overlapping() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0)));
    world.add_entity((A(1), C(1)));
    world.add_entity((A(2), B(2), C(2)));
    world.group::<(A, B)>().group::<(A, C)>();

    let a = world.borrow::<View<'_, A>>().unwrap();
    let (_, _, metadata) = (&a).into_shiperator();

    let NodeMetadata::Window(handle) = metadata.0 else {
        panic!("a grouped window reports its groups");
    };
    // SAFE: `a` keeps A borrowed, and its group metadata unmodified, until the end of the test.
    let window = unsafe { handle.get(EntityCursor::empty()) };
    assert!(core::ptr::eq(window.groups, &a.groups));
    let lens: Vec<usize> = window.groups.ranges.iter().map(|range| range.len).collect();
    assert_eq!(lens, [1, 1, 1]);
}

/// Asserts that a query over storages without any group has no grouped alternative
/// and keeps its smallest captain.
#[test]
fn no_metadata() {
    let mut world = World::new_for_test();
    add_ab(&mut world, 3);
    world.add_entity(A(10));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, entities, metadata) = (&a, &b).into_shiperator();

    assert_ungrouped(&metadata);
    assert_eq!((mixed.captain_mask, entities.cursor.len()), (0b10, 3));
}

/// Asserts that a group of storages outside the query doesn't give it a grouped alternative.
#[test]
fn unrelated_group() {
    let mut world = World::new_for_test();
    add_ab(&mut world, 3);
    for i in 0..5 {
        world.add_entity((C(i), D(i)));
    }
    world.group::<(C, D)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, _, metadata) = (&a, &b).into_shiperator();

    assert_ungrouped(&metadata);
    assert_eq!(mixed.captain_mask, 0b01);
}

/// Asserts that an empty group has a grouped alternative without any walk,
/// which ties with the empty ordinary walk and loses.
#[test]
fn empty_group() {
    let mut world = World::new_for_test();
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, _, metadata) = (&a, &b).into_shiperator();

    assert!(walks((&a, &b)).is_empty());
    assert_eq!(selection(&metadata).unwrap().ordinary.work, Some(0));
    assert_ordinary_wins(&metadata, 0);
    assert_eq!(mixed.captain_mask, 0b01);
}

/// Asserts that a fully aligned group wins with a single aligned walk, skipping the
/// evaluated ungrouped components, while the iterator still installs the ordinary walk.
#[test]
fn aligned() {
    let mut world = World::new_for_test();
    add_ab(&mut world, 5);
    world.add_entity(A(10));
    world.add_entity(A(11));
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, entities, metadata) = (&a, &b).into_shiperator();

    assert_grouped_wins(&metadata, 0b11);
    assert_eq!(walks((&a, &b)), [aligned_walk(5, 0b11)]);
    // 5 candidates without sailor against B's 5 candidates with A looked up.
    let selection = selection(&metadata).unwrap();
    assert_eq!(selection.cheapest().unwrap().work, 10);
    assert_eq!(selection.ordinary.work, Some(15));

    assert_eq!((mixed.captain_mask, entities.cursor.len()), (0b10, 5));
}

/// Asserts that a small ordinary captain beats a large group,
/// groups are hints and don't force their walks.
#[test]
fn small_captain() {
    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 20);
    for &entity in &entities[..2] {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (mixed, _, metadata) = (&a, &b, &c).into_shiperator();

    assert_ordinary_wins(&metadata, 2);
    assert_eq!(mixed.captain_mask, 0b100);
}

/// Asserts that with two disjoint groups, the one with the cheaper complete walk wins.
#[test]
fn disjoint_groups() {
    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 20);
    for &entity in &entities[..10] {
        world.add_component(entity, (C(0), D(0)));
    }
    world.group::<(A, B)>();
    world.group::<(C, D)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();
    let (_, _, metadata) = (&a, &b, &c, &d).into_shiperator();

    assert_grouped_wins(&metadata, 0b1100);
    assert_eq!(walks((&a, &b, &c, &d)), [aligned_walk(10, 0b1100)]);
}

/// Asserts that two disjoint groups with equal estimates pick the one whose
/// first member comes first in the query.
#[test]
fn disjoint_tie() {
    let mut world = World::new_for_test();
    for i in 0..10 {
        world.add_entity((A(i), B(i), C(i), D(i)));
    }
    world.group::<(C, D)>();
    world.group::<(A, B)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let (_, _, metadata) = (&a, &b, &c, &d).into_shiperator();
    assert_grouped_wins(&metadata, 0b0011);

    let (_, _, metadata) = (&c, &d, &a, &b).into_shiperator();
    assert_grouped_wins(&metadata, 0b0011);
}

/// Asserts that entities added after a regroup make every member walk its unevaluated
/// region, which costs more than the ordinary walk, until the next regroup aligns them.
#[test]
fn multiwalk() {
    let mut world = World::new_for_test();
    add_ab(&mut world, 10);
    world.group::<(A, B)>();
    add_ab(&mut world, 10);

    {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let (_, _, metadata) = (&a, &b).into_shiperator();

        assert_eq!(
            walks((&a, &b)),
            [
                aligned_walk(10, 0b11),
                Walk::Unevaluated {
                    owner: 0,
                    start: 10,
                    len: 10
                },
                Walk::Unevaluated {
                    owner: 1,
                    start: 10,
                    len: 10
                },
            ]
        );
        // 10 aligned candidates, then 20 unevaluated ones looking the other member up,
        // against A's 20 candidates looking B up.
        assert_eq!(estimates(&metadata), [(30, 10 * 2 + 20 * 3)]);
        assert_eq!(selection(&metadata).unwrap().ordinary.work, Some(20 * 3));
        assert_ordinary_wins(&metadata, 0);
    }

    world.regroup();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, metadata) = (&a, &b).into_shiperator();

    assert_eq!(walks((&a, &b)), [aligned_walk(20, 0b11)]);
    assert_eq!(estimates(&metadata), [(20, 40)]);
    assert_grouped_wins(&metadata, 0b11);
}

/// Asserts that equal estimates keep the ordinary walk: 4 aligned AB candidates looking up C
/// cost as much as C's 3 candidates looking up A and B.
#[test]
fn equal_cost() {
    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 4);
    for &entity in &entities[..3] {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (mixed, _, metadata) = (&a, &b, &c).into_shiperator();

    let selection = selection(&metadata).unwrap();
    assert_eq!(selection.cheapest().unwrap().work, 12);
    assert_eq!(selection.ordinary.work, Some(12));
    assert_ordinary_wins(&metadata, 2);
    assert_eq!(mixed.captain_mask, 0b100);
}

/// Asserts that an AB query with only an ABC group has no grouped alternative,
/// and still yields the AB entities lacking C.
#[test]
fn projection() {
    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 5);
    for &entity in &entities[..3] {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B, C)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, metadata) = (&a, &b).into_shiperator();

    assert_ungrouped(&metadata);
    assert_eq!((&a, &b).iter().count(), 5);
}

/// AB and AC groups after regroup promoted grouped entities into ABC:
/// - AB holds 6 entities, two of its first 8 gained a C
/// - AC holds 7 entities, one of its first 8 gained a B
/// - ABC holds 7 entities, the 4 owning A, B and C from the start and the 3 promoted
/// - 3 entities own a B alone
fn overlap_world() -> World {
    let mut world = World::new_for_test();
    let ab: Vec<EntityId> = (0..8).map(|i| world.add_entity((A(i), B(i)))).collect();
    let ac: Vec<EntityId> = (0..8).map(|i| world.add_entity((A(i), C(i)))).collect();
    for i in 0..4 {
        world.add_entity((A(i), B(i), C(i)));
    }
    for i in 0..3 {
        world.add_entity(B(i));
    }
    world.group::<(A, B)>().group::<(A, C)>();

    world.add_component(ab[0], C(0));
    world.add_component(ab[1], C(1));
    world.add_component(ac[0], B(0));
    world.regroup();

    world
}

/// Returns the length of each range the cheapest alternative `query` reports walks, by group size.
fn cover_ranges<Q: IntoShiperator>(query: Q) -> Vec<usize>
where
    Q::Shiperator: Shiperator,
{
    cheapest_candidate(query)
        .ranges
        .iter()
        .map(|range| range.group.len())
        .collect()
}

/// Asserts that an `(A, B)` query over AB and AC wins with an aligned walk over AB then one
/// over the ABC union, skipping AC.
#[test]
fn overlapping() {
    let world = overlap_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, metadata) = (&a, &b).into_shiperator();

    assert_grouped_wins(&metadata, 0b11);
    assert_eq!(cover_ranges((&a, &b)), [2, 3]);
    assert_eq!(
        walks((&a, &b)),
        [
            aligned_walk(6, 0b11),
            Walk::Aligned {
                range: 1,
                start: 0,
                len: 7,
                captains: 0b11
            }
        ]
    );
    // 13 candidates without sailor, against B's 16 candidates looking A up.
    assert_eq!(estimates(&metadata), [(13, 26)]);
    assert_eq!(selection(&metadata).unwrap().ordinary.work, Some(16 * 3));
}

/// Asserts that an `(A, B, C)` query over AB and AC covers their ABC union alone:
/// matches can't be in AB or AC once evaluated.
#[test]
fn union_exclusion() {
    let world = overlap_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (_, _, metadata) = (&a, &b, &c).into_shiperator();

    assert_grouped_wins(&metadata, 0b111);
    assert_eq!(cover_ranges((&a, &b, &c)), [3]);
    assert_eq!(walks((&a, &b, &c)), [aligned_walk(7, 0b111)]);
    assert_eq!(estimates(&metadata), [(7, 14)]);
}

/// Asserts that a small ordinary captain beats walks over several ranges.
#[test]
fn overlap_small_captain() {
    let mut world = overlap_world();
    let ab = world.add_entity((A(100), B(100), D(100)));
    world.regroup();
    world.add_entity(D(101));

    let (a, b, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, D>)>()
        .unwrap();
    let (mixed, _, metadata) = (&a, &b, &d).into_shiperator();

    assert_ordinary_wins(&metadata, 2);
    assert_eq!(mixed.captain_mask, 0b100);
    assert_eq!((&a, &b, &d).iter().ids().collect::<Vec<_>>(), [ab]);
}

/// Asserts that AB and AC, with an ABC entity added after the last regroup, give an
/// `(A, B, C)` query an alternative made of unevaluated walks alone: no ABC union exists yet.
#[test]
fn absent_union() {
    let mut world = World::new_for_test();
    for i in 0..3 {
        world.add_entity((A(i), B(i)));
        world.add_entity((A(i), C(i)));
    }
    world.group::<(A, B)>().group::<(A, C)>();
    world.add_entity((A(10), B(10), C(10)));

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (_, _, metadata) = (&a, &b, &c).into_shiperator();

    // 3 candidates each looking 2 components up cost 12, C captaining alone 16.
    assert_grouped_wins(&metadata, 0b111);
    assert!(cover_ranges((&a, &b, &c)).is_empty());
    assert_eq!(
        walks((&a, &b, &c)),
        [
            Walk::Unevaluated {
                owner: 0,
                start: 6,
                len: 1
            },
            Walk::Unevaluated {
                owner: 1,
                start: 3,
                len: 1
            },
            Walk::Unevaluated {
                owner: 2,
                start: 3,
                len: 1
            },
        ]
    );
    assert_eq!(estimates(&metadata), [(3, 12)]);
    assert_eq!(selection(&metadata).unwrap().ordinary.work, Some(16));
}

/// AB and ACD groups holding 6 AB entities, 6 ACD entities and 5 entities owning all four
/// components in their ABCD union.
fn extra_world() -> World {
    let mut world = World::new_for_test();
    for i in 0..6 {
        world.add_entity((A(i), B(i)));
        world.add_entity((A(i), C(i), D(i)));
    }
    for i in 0..5 {
        world.add_entity((A(i), B(i), C(i), D(i)));
    }
    world.group::<(A, B)>().group::<(A, C, D)>();

    world
}

/// Asserts that an `(A, B, C)` query over AB and ACD covers AB, walking AB and ABCD without
/// borrowing D. C reads directly in ABCD, it's looked up in AB.
#[test]
fn extra_captain() {
    let world = extra_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (_, _, metadata) = (&a, &b, &c).into_shiperator();

    assert_grouped_wins(&metadata, 0b011);
    assert_eq!(cover_ranges((&a, &b, &c)), [2, 4]);
    assert_eq!(
        walks((&a, &b, &c)),
        [
            aligned_walk(6, 0b011),
            Walk::Aligned {
                range: 1,
                start: 0,
                len: 5,
                captains: 0b111
            }
        ]
    );
    // 6 candidates looking C up and 5 looking nothing up, against 11 looking 2 components up.
    assert_eq!(cheapest(&metadata).work, 28);
}

/// Asserts that a set of overlapping groups competes with a disjoint group: with AB and AC
/// against a smaller DE, an `(A, B, D, E)` query has both alternatives and DE wins.
#[test]
fn overlap_disjoint() {
    let mut world = overlap_world();
    for i in 0..3 {
        world.add_entity((A(i), B(i), D(i), E(i)));
    }
    world.group::<(D, E)>();

    let (a, b, d, e) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, D>, View<'_, E>)>()
        .unwrap();
    let (_, _, metadata) = (&a, &b, &d, &e).into_shiperator();

    let selection = selection(&metadata).unwrap();
    let required: Vec<u32> = selection
        .alternatives
        .iter()
        .map(|alternative| selection.group_owner(alternative).1)
        .collect();
    assert_eq!(required, [0b0011, 0b1100]);
    // AB walks 16 candidates looking D and E up, DE walks 3 looking A and B up.
    assert_eq!(estimates(&metadata), [(16, 16 * 4), (3, 3 * 4)]);
    assert_grouped_wins(&metadata, 0b1100);
}

/// Asserts that queries containing `Not`, `Optional`, `Or` or a tracking filter keep
/// ordinary selection even though their storages are grouped, and report it to parents.
#[test]
fn filters() {
    let mut world = World::new_for_test();
    for i in 0..5 {
        world.add_entity((A(i), B(i), C(i), Ins(i)));
    }
    world.add_entity(D(0));
    world.group::<(A, B)>();
    world.group::<(C, Ins)>();

    let (a, b, c, d, ins) = world
        .borrow::<(
            View<'_, A>,
            View<'_, B>,
            View<'_, C>,
            View<'_, D>,
            View<'_, Ins>,
        )>()
        .unwrap();

    let (mixed, _, metadata) = (&a, &b, !&d).into_shiperator();
    assert!(matches!(metadata.0, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b001);

    let (mixed, _, metadata) = (&a, &b, Optional(&d)).into_shiperator();
    assert!(matches!(metadata.0, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b001);

    let (mixed, _, metadata) = (&a, &b, &c | &d).into_shiperator();
    assert!(matches!(metadata.0, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b001);

    let (mixed, _, metadata) = (&c, ins.inserted()).into_shiperator();
    assert!(matches!(metadata.0, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b01);

    // A filter in a nested query disables grouping for its parent too.
    let (_, _, metadata) = ((&c, !&d), &a, &b).into_shiperator();
    assert!(matches!(metadata.0, NodeMetadata::Filtered));
}

/// Asserts that a modification tracked window isn't a filter and stays eligible.
#[test]
fn modification_tracked() {
    let mut world = World::new_for_test();
    for i in 0..5 {
        world.add_entity((Modif(i), B(i)));
    }
    world.group::<(Modif, B)>();

    let (mut modifs, b) = world.borrow::<(ViewMut<'_, Modif>, View<'_, B>)>().unwrap();
    let (_, _, metadata) = (&mut modifs, &b).into_shiperator();

    assert_grouped_wins(&metadata, 0b11);
}

/// Asserts that a storage repeated in a query doesn't prove its group: we don't pick which
/// position would drive, so the query keeps ordinary selection.
#[test]
fn repeated_storage() {
    let mut world = World::new_for_test();
    add_ab(&mut world, 4);
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (mixed, _, metadata) = (&a, &a, &b).into_shiperator();
    assert_ungrouped(&metadata);
    assert_eq!(mixed.captain_mask, 0b001);

    let (_, _, metadata) = (&a, &b, &b).into_shiperator();
    assert_ungrouped(&metadata);
}

/// Asserts that a group with a thread-local member only matches that thread-local storage,
/// not the `Send + Sync` storage holding the same component type.
#[cfg(feature = "thread_local")]
#[test]
fn thread_local_identity() {
    use crate::add_component::AddComponent;
    use crate::borrow::NonSend;

    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 4);
    {
        let mut local_a = world.borrow::<NonSend<ViewMut<'_, A>>>().unwrap();
        for &entity in &entities {
            local_a.add_component_unchecked(entity, A(10));
        }
    }
    world.group::<(NonSend<A>, B)>();

    let (a, local_a, b) = world
        .borrow::<(View<'_, A>, NonSend<View<'_, A>>, View<'_, B>)>()
        .unwrap();

    let (_, _, metadata) = (&a, &b).into_shiperator();
    assert_ungrouped(&metadata);

    let (_, _, metadata) = (&*local_a, &b).into_shiperator();
    assert_grouped_wins(&metadata, 0b11);
}

/// Asserts that a nested `Mixed` isn't a group member for its parent, its parent drives
/// through the alternative it reported instead.
///
/// 10 AB pairs all owning C: driving the aligned pair costs 20 and looking C up 10 more,
/// against 40 for C looking the pair up.
#[test]
fn nested() {
    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 10);
    for &entity in &entities {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let (_, _, metadata) = (&a, &b).into_shiperator();
    assert_grouped_wins(&metadata, 0b11);

    let (mixed, _, metadata) = ((&a, &b), &c).into_shiperator();
    let selection = selection(&metadata).unwrap();
    let winner = selection.winner().unwrap();

    assert_eq!(selection.ordinary.captain, 1);
    assert_eq!(selection.ordinary.work, Some(40));
    assert!(matches!(winner.route, Route::Nested { child: 0, .. }));
    assert_eq!((winner.candidates, winner.work), (10, 30));
    assert_eq!(selection.group_owner(winner).1, 0b11);
    // Constructors still install the ordinary walk, only the root swaps it.
    assert_eq!(mixed.captain_mask, 0b10);
}

/// Asserts that a parent can prefer the alternative its nested `Mixed` ranks second.
///
/// ABC and DE are disjoint groups, 4 entities own every component.\
/// Alone, `(A, B, C, D, E)` prefers walking the 6 ABC entities and looking D and E up (24)
/// over walking the 5 DE entities and looking A, B and C up (25).\
/// In `((A, B, C, D, E), F, G)` each candidate also looks F and G up: ABC costs 24 + 12 and
/// DE 25 + 10, DE wins. Lone components make every ordinary walk far more expensive.
#[test]
fn locally_losing() {
    let mut world = World::new_for_test();
    let matches: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C(i), D(i), E(i), F(i), G(i))))
        .collect();
    world.add_entity((A(10), B(10), C(10)));
    world.add_entity((A(11), B(11), C(11)));
    world.add_entity((D(12), E(12)));
    for i in 20..40 {
        world.add_entity(A(i));
        world.add_entity(B(i));
        world.add_entity(C(i));
        world.add_entity(D(i));
        world.add_entity(E(i));
        world.add_entity(F(i));
        world.add_entity(G(i));
    }
    world.group::<(A, B, C)>().group::<(D, E)>();

    let (a, b, c, d, e, f, g) = world
        .borrow::<(
            View<'_, A>,
            View<'_, B>,
            View<'_, C>,
            View<'_, D>,
            View<'_, E>,
            View<'_, F>,
            View<'_, G>,
        )>()
        .unwrap();

    let (_, _, metadata) = (&a, &b, &c, &d, &e).into_shiperator();
    assert_eq!(estimates(&metadata), [(6, 24), (5, 25)]);
    assert_grouped_wins(&metadata, 0b00111);

    let (_, _, metadata) = ((&a, &b, &c, &d, &e), &f, &g).into_shiperator();
    assert_eq!(estimates(&metadata), [(6, 36), (5, 35)]);
    let selection = selection(&metadata).unwrap();
    let winner = selection.winner().unwrap();
    assert!(matches!(winner.route, Route::Nested { child: 0, .. }));
    assert_eq!(selection.group_owner(winner).1, 0b11000);

    // The DE walk is installed: 5 candidates, the root rejecting none.
    let iter = ((&a, &b, &c, &d, &e), &f, &g).iter();
    assert_eq!(iter.size_hint(), (0, Some(5)));
    let mut ids: Vec<EntityId> = iter.ids().collect();
    ids.sort_unstable();
    assert_eq!(ids, matches);
}

/// Asserts that a nested alternative and a local one with equal estimates keep the one
/// involving the earlier child, whichever route it takes.
///
/// AB and CD are disjoint groups over the same 5 entities. Driving the AB pair costs 10 plus
/// 10 for looking C and D up. Driving CD costs 10 plus 10 for looking the AB pair up.
#[test]
fn route_tie() {
    let mut world = World::new_for_test();
    for i in 0..5 {
        world.add_entity((A(i), B(i), C(i), D(i)));
    }
    for i in 10..20 {
        world.add_entity(A(i));
        world.add_entity(B(i));
        world.add_entity(C(i));
        world.add_entity(D(i));
    }
    world.group::<(A, B)>().group::<(C, D)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let (_, _, metadata) = ((&a, &b), &c, &d).into_shiperator();
    assert_eq!(estimates(&metadata), [(5, 20), (5, 20)]);
    let winner = selection(&metadata).unwrap().winner().unwrap();
    assert!(matches!(winner.route, Route::Nested { child: 0, .. }));

    let (_, _, metadata) = (&c, &d, (&a, &b)).into_shiperator();
    assert_eq!(estimates(&metadata), [(5, 20), (5, 20)]);
    let winner = selection(&metadata).unwrap().winner().unwrap();
    assert_eq!(winner.route, Route::Local { required: 0b011 });
}

/// Asserts that removing a B from the middle of an aligned range splits the walk: the prefix
/// A and B still both trust, then the rest of B's range, B being shorter, with A looked up.
///
/// B's removal moves its last pair into the hole, so it trusts its first 6 positions.\
/// 6 aligned candidates and 3 damaged ones looking A up cost 21, against 27 for B captaining.
#[test]
fn damaged_owner() {
    let mut world = World::new_for_test();
    let pairs = add_ab(&mut world, 10);
    world.group::<(A, B)>();
    world.delete_component::<B>(pairs[6]);

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, metadata) = (&a, &b).into_shiperator();

    assert_eq!(
        walks((&a, &b)),
        [
            aligned_walk(6, 0b11),
            Walk::Damaged {
                range: 0,
                owner: 1,
                start: 6,
                len: 3,
                captains: 0b10
            }
        ]
    );
    assert_eq!(estimates(&metadata), [(9, 21)]);
    assert_eq!(selection(&metadata).unwrap().ordinary.work, Some(27));
    assert_grouped_wins(&metadata, 0b11);

    let mut ids: Vec<EntityId> = (&a, &b).iter().ids().collect();
    ids.sort_unstable();
    let mut expected = pairs;
    expected.remove(6);
    assert_eq!(ids, expected);
}

/// A world where AB is grouped over 20 entities and only 2 of them own C.
fn owned_world() -> World {
    let mut world = World::new_for_test();
    let entities = add_ab(&mut world, 20);
    for &entity in &entities[..2] {
        world.add_component(entity, C(0));
    }
    world.add_entity(A(30));
    world.group::<(A, B)>();

    world
}

/// Asserts that owned iteration reaches the same selection as borrowed views:
/// the group wins for AB, the small C wins for ABC.
#[test]
fn owned_route() {
    let world = owned_world();
    let all_storages = world.borrow::<AllStoragesView<'_>>().unwrap();
    let current = TrackingTimestamp::origin();

    let (mixed, _, _, _, metadata) =
        <(&A, &B) as IterComponent>::into_shiperator(&all_storages, None, current).unwrap();
    assert_grouped_wins(&metadata, 0b11);
    assert_eq!(mixed.captain_mask, 0b10);

    let (mixed, _, _, _, metadata) =
        <(&A, &B, &C) as IterComponent>::into_shiperator(&all_storages, None, current).unwrap();
    assert_ordinary_wins(&metadata, 2);
    assert_eq!(mixed.captain_mask, 0b100);
}

/// Asserts that every iterator of an `IntoIterRef` starts from the grouped walk installed when
/// it was built, a consumed iterator doesn't move the next one's start.
#[test]
fn owned_reuse() {
    let world = owned_world();

    let mut iter_ref = world.iter::<(&A, &B)>();

    for _ in 0..2 {
        let iter = iter_ref.iter();
        assert_eq!(iter.shiperator.captain_mask, 0b11);
        assert_eq!(iter.cursor.len(), 20);
        assert_eq!(iter.count(), 20);
    }
}
