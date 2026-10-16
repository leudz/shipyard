//! Modification flags of `Mut` outputs, whichever way the iterator is consumed.

use super::ways::every_way;
use shipyard::advanced::Mut;
use shipyard::{track, Component, EntityId, Get, IntoIter, View, ViewMut, World};

#[derive(Debug)]
struct Modif(u32);
impl Component for Modif {
    type Tracking = track::Modification;
}

#[derive(Debug)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

/// `e0` to `e3` own `Modif(i)`, `e1` and `e3` also own `A(i)`.
/// Six more entities only own `A` so `Modif` drives queries with it, rejecting `e0` and `e2`.
///
/// No flag is set.
fn world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity(Modif(i))).collect();
    for i in [1, 3] {
        world.add_component(e[i], A(i as u32));
    }
    for i in 0..6 {
        world.add_entity(A(100 + i));
    }
    world
        .borrow::<ViewMut<Modif>>()
        .unwrap()
        .clear_all_modified();

    (world, e)
}

/// Returns the entities of `e` whose `Modif` is flagged as modified.
fn modified(modif: &ViewMut<Modif>, e: &[EntityId]) -> Vec<EntityId> {
    e.iter()
        .copied()
        .filter(|&entity| modif.is_modified(entity))
        .collect()
}

/// Asserts that reading `Mut` outputs in every way, prefixes included, flags nothing.
#[test]
fn read_no_flags() {
    let (world, e) = world();
    let (mut modif, a) = world.borrow::<(ViewMut<Modif>, View<A>)>().unwrap();
    assert_eq!(modified(&modif, &e), []);

    let items = every_way!((&mut modif).iter(), |modif: Mut<Modif>| modif.0);
    assert_eq!(items.len(), 4);
    let items = every_way!((&mut modif, &a).iter(), |(modif, a): (Mut<Modif>, &A)| {
        (modif.0, a.0)
    });
    assert_eq!(items, [(e[1], (1, 1)), (e[3], (3, 3))]);

    assert_eq!(modified(&modif, &e), []);
}

/// Asserts that writing through the `Mut` `next` yields flags only that component.
#[test]
fn write_next() {
    let (world, e) = world();
    let (mut modif, a) = world.borrow::<(ViewMut<Modif>, View<A>)>().unwrap();

    let mut iter = (&mut modif, &a).iter();
    let (mut first, _) = iter.next().unwrap();
    first.0 += 10;
    drop(iter);

    assert_eq!(modified(&modif, &e), [e[1]]);
}

/// Asserts that writing through every `Mut` `fold` yields flags them and only them:
/// the rejected `e0` and `e2` stay unflagged.
#[test]
fn write_fold() {
    let (world, e) = world();
    let (mut modif, a) = world.borrow::<(ViewMut<Modif>, View<A>)>().unwrap();

    let count = (&mut modif, &a).iter().fold(0, |count, (mut modif, _)| {
        modif.0 += 10;
        count + 1
    });

    assert_eq!(count, 2);
    assert_eq!(modified(&modif, &e), [e[1], e[3]]);
}

/// Asserts that writing through the `Mut`s `for_each` yields after a `next` prefix
/// flags them, while the prefix only read stays unflagged.
#[test]
fn write_for_each_after_prefix() {
    let (world, e) = world();
    let (mut modif, a) = world.borrow::<(ViewMut<Modif>, View<A>)>().unwrap();

    let mut iter = (&mut modif, &a).iter();
    let (first, _) = iter.next().unwrap();
    assert_eq!(first.0, 1);
    iter.for_each(|(mut modif, _)| modif.0 += 10);

    assert_eq!(modified(&modif, &e), [e[3]]);
}

/// Asserts that `WithId` yields each `Mut` with its own entity: writing through the one
/// of `e3` flags `e3` alone.
#[test]
fn write_with_id() {
    let (world, e) = world();
    let (mut modif, a) = world.borrow::<(ViewMut<Modif>, View<A>)>().unwrap();

    (&mut modif, &a)
        .iter()
        .with_id()
        .for_each(|(id, (mut modif, _))| {
            if id == e[3] {
                modif.0 += 10;
            }
        });

    assert_eq!(modified(&modif, &e), [e[3]]);
}

/// Asserts that components written through `Mut` during an iteration are the ones
/// a later `Modified` query yields, in every way.
#[test]
fn modified_after_write() {
    let (world, e) = world();
    let (mut modif, a) = world.borrow::<(ViewMut<Modif>, View<A>)>().unwrap();

    (&mut modif).iter().with_id().for_each(|(id, mut modif)| {
        if id == e[0] || id == e[3] {
            modif.0 += 10;
        }
    });

    let items = every_way!(modif.modified().iter(), |modif: &Modif| modif.0);
    assert_eq!(items, [(e[0], 10), (e[3], 13)]);
    let items = every_way!((modif.modified(), &a).iter(), |(modif, a): (&Modif, &A)| (
        modif.0, a.0
    ));
    assert_eq!(items, [(e[3], (13, 3))]);
}

/// Asserts that a `Modified` query over a mutable view yields `Mut`s whose writes flag again.
#[test]
fn modified_mut() {
    let (world, e) = world();
    let mut modif = world.borrow::<ViewMut<Modif>>().unwrap();

    (&mut modif).get(e[2]).unwrap().0 += 10;
    modif
        .modified_mut()
        .iter()
        .for_each(|mut modif| modif.0 += 10);

    assert_eq!(modified(&modif, &e), [e[2]]);
    assert_eq!((&modif).get(e[2]).unwrap().0, 22);
}
