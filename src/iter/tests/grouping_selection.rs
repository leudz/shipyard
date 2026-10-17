//! Selection between ordinary and grouped walks, on worlds grouped by `group` and `regroup`.
//!
//! Queries are built through their real constructors. We read the selection a `Mixed`
//! reports in its construction metadata, constructors always install the ordinary walk.\
//! Installing the winner is tested in `grouping_automatic`.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::planner::{Alternative, NodeMetadata, Route, Selection, Walk};
use crate::iter::{IntoIter, IntoShiperator, IterComponent, Optional, RawEntityIdAccess};
use crate::sparse_set::{Ring, SparseSet};
use crate::storage::StorageId;
use crate::track;
use crate::tracking::TrackingTimestamp;
use crate::views::{AllStoragesView, View, ViewMut};
use crate::world::World;
use crate::ShipHashSet;
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
fn selection(entities: &RawEntityIdAccess) -> Option<&Selection> {
    match &entities.metadata {
        NodeMetadata::Mixed(selection) if !selection.alternatives.is_empty() => Some(selection),
        _ => None,
    }
}

/// Asserts that a `Mixed` reported no grouped alternative, without filter.
fn assert_ungrouped(entities: &RawEntityIdAccess) {
    let NodeMetadata::Mixed(selection) = &entities.metadata else {
        panic!("a Mixed without filter reports its selection");
    };

    assert!(selection.alternatives.is_empty());
}

/// Asserts that the cheapest grouped alternative of `entities` wins with the members
/// `member_mask` flags.
fn assert_grouped_wins(entities: &RawEntityIdAccess, member_mask: u32) {
    let selection = selection(entities).expect("a grouped alternative");
    let winner = selection.winner().expect("the grouped alternative wins");

    assert_eq!(winner.group().member_mask(), member_mask);
}

/// Asserts that `entities` reported a grouped alternative but its ordinary walk with
/// child `captain` wins.
fn assert_ordinary_wins(entities: &RawEntityIdAccess, captain: usize) {
    let selection = selection(entities).expect("a grouped alternative");

    assert!(selection.winner().is_none());
    assert_eq!(selection.ordinary.captain, captain);
}

/// Returns the cheapest grouped alternative of `entities`.
fn cheapest(entities: &RawEntityIdAccess) -> &Alternative {
    selection(entities)
        .expect("a grouped alternative")
        .cheapest()
        .unwrap()
}

/// Returns the walks of the cheapest grouped alternative of `entities`.
fn walks(entities: &RawEntityIdAccess) -> &[Walk] {
    &cheapest(entities).group().walks
}

/// Adds `count` entities owning `A(i)` and `B(i)`.
fn add_ab(world: &mut World, count: u32) -> Vec<EntityId> {
    (0..count).map(|i| world.add_entity((A(i), B(i)))).collect()
}

/// Asserts that a window over a grouped storage reports its group, its own storage id
/// and the regions of the storage, read from the base's metadata.
#[test]
fn window_metadata() {
    let mut world = World::new();
    add_ab(&mut world, 3);
    world.add_entity(A(10));
    world.group::<(A, B)>();
    world.add_entity(A(11));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, entities) = (&a).into_shiperator();

    let NodeMetadata::Window(window) = &entities.metadata else {
        panic!("a grouped window reports its group");
    };
    assert_eq!(window.storage_id, StorageId::of::<SparseSet<A>>());
    assert_eq!((window.unevaluated_start, window.len), (4, 5));
    let [range] = &window.ranges[..] else {
        panic!("A holds a single range");
    };
    assert!(Arc::ptr_eq(&range.group, &b.groups.ranges[0].group));
    assert_eq!(
        (range.ring, range.trust),
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

/// Asserts that a window over a storage in two overlapping groups reports each of its ranges,
/// while selection stays ordinary until grouped walks handle several ranges.
#[test]
fn window_metadata_overlapping() {
    let mut world = World::new();
    world.add_entity((A(0), B(0)));
    world.add_entity((A(1), C(1)));
    world.add_entity((A(2), B(2), C(2)));
    world.group::<(A, B)>().group::<(A, C)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, entities) = (&a).into_shiperator();

    let NodeMetadata::Window(window) = &entities.metadata else {
        panic!("a grouped window reports its groups");
    };
    let lens: Vec<usize> = window.ranges.iter().map(|range| range.ring.len).collect();
    assert_eq!(lens, [1, 1, 1]);
    for (range, storage_range) in window.ranges.iter().zip(&a.groups.ranges) {
        assert!(Arc::ptr_eq(&range.group, &storage_range.group));
    }

    let (_, _, entities) = (&a, &b).into_shiperator();
    assert_ungrouped(&entities);
}

/// Asserts that a query over storages without any group has no grouped alternative
/// and keeps its smallest captain.
#[test]
fn no_metadata() {
    let mut world = World::new();
    add_ab(&mut world, 3);
    world.add_entity(A(10));

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, len, entities) = (&a, &b).into_shiperator();

    assert_ungrouped(&entities);
    assert_eq!((mixed.captain_mask, len), (0b10, 3));
}

/// Asserts that a group of storages outside the query doesn't give it a grouped alternative.
#[test]
fn unrelated_group() {
    let mut world = World::new();
    add_ab(&mut world, 3);
    for i in 0..5 {
        world.add_entity((C(i), D(i)));
    }
    world.group::<(C, D)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, _, entities) = (&a, &b).into_shiperator();

    assert_ungrouped(&entities);
    assert_eq!(mixed.captain_mask, 0b01);
}

/// Asserts that an empty group has a grouped alternative without any walk,
/// which ties with the empty ordinary walk and loses.
#[test]
fn empty_group() {
    let mut world = World::new();
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, _, entities) = (&a, &b).into_shiperator();

    assert!(walks(&entities).is_empty());
    assert_eq!(selection(&entities).unwrap().ordinary.work, Some(0));
    assert_ordinary_wins(&entities, 0);
    assert_eq!(mixed.captain_mask, 0b01);
}

/// Asserts that a fully aligned group wins with a single aligned walk, skipping the
/// evaluated ungrouped components, while the iterator still installs the ordinary walk.
#[test]
fn aligned() {
    let mut world = World::new();
    add_ab(&mut world, 5);
    world.add_entity(A(10));
    world.add_entity(A(11));
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, len, entities) = (&a, &b).into_shiperator();

    assert_grouped_wins(&entities, 0b11);
    assert_eq!(walks(&entities), [Walk::Aligned { len: 5 }]);
    // 5 candidates without sailor against B's 5 candidates with A looked up.
    let selection = selection(&entities).unwrap();
    assert_eq!(selection.cheapest().unwrap().work, 10);
    assert_eq!(selection.ordinary.work, Some(15));

    assert_eq!((mixed.captain_mask, len), (0b10, 5));
}

/// Asserts that a small ordinary captain beats a large group,
/// groups are hints and don't force their walks.
#[test]
fn small_captain() {
    let mut world = World::new();
    let entities = add_ab(&mut world, 20);
    for &entity in &entities[..2] {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (mixed, _, entities) = (&a, &b, &c).into_shiperator();

    assert_ordinary_wins(&entities, 2);
    assert_eq!(mixed.captain_mask, 0b100);
}

/// Asserts that with two disjoint groups, the one with the cheaper complete walk wins.
#[test]
fn disjoint_groups() {
    let mut world = World::new();
    let entities = add_ab(&mut world, 20);
    for &entity in &entities[..10] {
        world.add_component(entity, (C(0), D(0)));
    }
    world.group::<(A, B)>();
    world.group::<(C, D)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();
    let (_, _, entities) = (&a, &b, &c, &d).into_shiperator();

    assert_grouped_wins(&entities, 0b1100);
    assert_eq!(walks(&entities), [Walk::Aligned { len: 10 }]);
}

/// Asserts that two disjoint groups with equal estimates pick the one whose
/// first member comes first in the query.
#[test]
fn disjoint_tie() {
    let mut world = World::new();
    for i in 0..10 {
        world.add_entity((A(i), B(i), C(i), D(i)));
    }
    world.group::<(C, D)>();
    world.group::<(A, B)>();

    let (a, b, c, d) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>, View<'_, D>)>()
        .unwrap();

    let (_, _, entities) = (&a, &b, &c, &d).into_shiperator();
    assert_grouped_wins(&entities, 0b0011);

    let (_, _, entities) = (&c, &d, &a, &b).into_shiperator();
    assert_grouped_wins(&entities, 0b0011);
}

/// Asserts that once removal resets A's group metadata, the grouped alternative walks
/// A's whole storage as unevaluated and loses the tie against the ordinary walk.
#[test]
fn reset() {
    let mut world = World::new();
    let entities = add_ab(&mut world, 10);
    world.group::<(A, B)>();
    world.remove::<A>(entities[0]);

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (mixed, _, entities) = (&a, &b).into_shiperator();

    // B's range is still there but untrusted, A drives the damaged part since its own is empty.
    assert_eq!(
        walks(&entities),
        [Walk::Unevaluated {
            owner: 0,
            start: 0,
            len: 9
        }]
    );
    let selection = selection(&entities).unwrap();
    assert_eq!(selection.cheapest().unwrap().work, 27);
    assert_eq!(selection.ordinary.work, Some(27));
    assert_ordinary_wins(&entities, 0);
    assert_eq!(mixed.captain_mask, 0b01);
}

/// Asserts that entities added after a regroup make every member walk its unevaluated
/// region, which costs more than the ordinary walk, until the next regroup aligns them.
#[test]
fn multiwalk() {
    let mut world = World::new();
    add_ab(&mut world, 10);
    world.group::<(A, B)>();
    add_ab(&mut world, 10);

    {
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        let (_, _, entities) = (&a, &b).into_shiperator();

        assert_eq!(
            walks(&entities),
            [
                Walk::Aligned { len: 10 },
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
        assert_ordinary_wins(&entities, 0);
    }

    world.regroup();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let (_, _, entities) = (&a, &b).into_shiperator();

    assert_eq!(walks(&entities), [Walk::Aligned { len: 20 }]);
    assert_grouped_wins(&entities, 0b11);
}

/// Asserts that equal estimates keep the ordinary walk: 4 aligned AB candidates looking up C
/// cost as much as C's 3 candidates looking up A and B.
#[test]
fn equal_cost() {
    let mut world = World::new();
    let entities = add_ab(&mut world, 4);
    for &entity in &entities[..3] {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let (mixed, _, entities) = (&a, &b, &c).into_shiperator();

    let selection = selection(&entities).unwrap();
    assert_eq!(selection.cheapest().unwrap().work, 12);
    assert_eq!(selection.ordinary.work, Some(12));
    assert_ordinary_wins(&entities, 2);
    assert_eq!(mixed.captain_mask, 0b100);
}

/// Asserts that an AB query with only an ABC group has no grouped alternative,
/// and still yields the AB entities lacking C.
#[test]
fn projection() {
    let mut world = World::new();
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

/// Asserts that queries containing `Not`, `Optional`, `Or` or a tracking filter keep
/// ordinary selection even though their storages are grouped, and report it to parents.
#[test]
fn filters() {
    let mut world = World::new();
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

    let (mixed, _, entities) = (&a, &b, !&d).into_shiperator();
    assert!(matches!(entities.metadata, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b001);

    let (mixed, _, entities) = (&a, &b, Optional(&d)).into_shiperator();
    assert!(matches!(entities.metadata, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b001);

    let (mixed, _, entities) = (&a, &b, &c | &d).into_shiperator();
    assert!(matches!(entities.metadata, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b001);

    let (mixed, _, entities) = (&c, ins.inserted()).into_shiperator();
    assert!(matches!(entities.metadata, NodeMetadata::Filtered));
    assert_eq!(mixed.captain_mask, 0b01);

    // A filter in a nested query disables grouping for its parent too.
    let (_, _, entities) = ((&c, !&d), &a, &b).into_shiperator();
    assert!(matches!(entities.metadata, NodeMetadata::Filtered));
}

/// Asserts that a modification tracked window isn't a filter and stays eligible.
#[test]
fn modification_tracked() {
    let mut world = World::new();
    for i in 0..5 {
        world.add_entity((Modif(i), B(i)));
    }
    world.group::<(Modif, B)>();

    let (mut modifs, b) = world.borrow::<(ViewMut<'_, Modif>, View<'_, B>)>().unwrap();
    let (_, _, entities) = (&mut modifs, &b).into_shiperator();

    assert_grouped_wins(&entities, 0b11);
}

/// Asserts that a storage repeated in a query doesn't prove its group: we don't pick which
/// position would drive, so the query keeps ordinary selection.
#[test]
fn repeated_storage() {
    let mut world = World::new();
    add_ab(&mut world, 4);
    world.group::<(A, B)>();

    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let (mixed, _, entities) = (&a, &a, &b).into_shiperator();
    assert_ungrouped(&entities);
    assert_eq!(mixed.captain_mask, 0b001);

    let (_, _, entities) = (&a, &b, &b).into_shiperator();
    assert_ungrouped(&entities);
}

/// Asserts that a group with a thread-local member only matches that thread-local storage,
/// not the `Send + Sync` storage holding the same component type.
#[cfg(feature = "thread_local")]
#[test]
fn thread_local_identity() {
    use crate::borrow::NonSend;
    use crate::AddComponent;

    let mut world = World::new();
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

    let (_, _, entities) = (&a, &b).into_shiperator();
    assert_ungrouped(&entities);

    let (_, _, entities) = (&*local_a, &b).into_shiperator();
    assert_grouped_wins(&entities, 0b11);
}

/// Asserts that a nested `Mixed` isn't a group member for its parent, its parent drives
/// through the alternative it reported instead.
///
/// 10 AB pairs all owning C: driving the aligned pair costs 20 and looking C up 10 more,
/// against 40 for C looking the pair up.
#[test]
fn nested() {
    let mut world = World::new();
    let entities = add_ab(&mut world, 10);
    for &entity in &entities {
        world.add_component(entity, C(0));
    }
    world.group::<(A, B)>();

    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let (_, _, entities) = (&a, &b).into_shiperator();
    assert_grouped_wins(&entities, 0b11);

    let (mixed, _, entities) = ((&a, &b), &c).into_shiperator();
    let selection = selection(&entities).unwrap();
    let winner = selection.winner().unwrap();

    assert_eq!(selection.ordinary.captain, 1);
    assert_eq!(selection.ordinary.work, Some(40));
    assert!(matches!(winner.route, Route::Nested { child: 0, .. }));
    assert_eq!((winner.candidates, winner.work), (10, 30));
    assert_eq!(winner.group().member_mask(), 0b11);
    // Constructors still install the ordinary walk, only the root swaps it.
    assert_eq!(mixed.captain_mask, 0b10);
}

/// A world where AB is grouped over 20 entities and only 2 of them own C.
fn owned_world() -> World {
    let mut world = World::new();
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

    let (mixed, _, _, _, entities) = <(&A, &B) as IterComponent>::into_shiperator(
        &all_storages,
        None,
        current,
        &mut ShipHashSet::new(),
    )
    .unwrap();
    assert_grouped_wins(&entities, 0b11);
    assert_eq!(mixed.captain_mask, 0b10);

    let (mixed, _, _, _, entities) = <(&A, &B, &C) as IterComponent>::into_shiperator(
        &all_storages,
        None,
        current,
        &mut ShipHashSet::new(),
    )
    .unwrap();
    assert_ordinary_wins(&entities, 2);
    assert_eq!(mixed.captain_mask, 0b100);
}

/// Asserts that `IntoIterRef` doesn't keep the construction metadata of a query
/// that reported a grouped alternative.
#[test]
fn owned_released() {
    let world = owned_world();

    let iter_ref = world.iter::<(&A, &B)>();

    assert!(matches!(
        iter_ref.entities.metadata,
        NodeMetadata::Ungrouped
    ));
}
