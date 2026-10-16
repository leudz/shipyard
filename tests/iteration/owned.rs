//! Iterators borrowing their storages themselves, from `World::iter` and `AllStorages::iter`.

use super::ways::every_way;
use shipyard::advanced::Mut;
use shipyard::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

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
struct Modif(u32);
impl Component for Modif {
    type Tracking = track::Modification;
}

/// `e0` to `e3` own `A(i)` and `Modif(i)`, `e1` and `e3` also own `B(10 + i)`.
fn world() -> (World, Vec<EntityId>) {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..4).map(|i| world.add_entity((A(i), Modif(i)))).collect();
    for i in [1, 3] {
        world.add_component(e[i], B(10 + i as u32));
    }
    world
        .borrow::<ViewMut<Modif>>()
        .unwrap()
        .clear_all_modified();

    (world, e)
}

/// Asserts that every iterator of the same `World::iter` starts over from the first
/// entity, whichever way the previous ones were consumed.
#[test]
fn repeat() {
    let (world, e) = world();
    let mut iter = world.iter::<(&A, &B)>();

    let items = every_way!(iter.iter(), |(a, b): (&A, &B)| (a.0, b.0));

    assert_eq!(items, [(e[1], (1, 11)), (e[3], (3, 13))]);
}

/// Asserts that every iterator of the same mutable `World::iter` starts over and sees
/// the writes of the previous ones.
#[test]
fn repeat_mut() {
    let (world, e) = world();
    let mut iter = world.iter::<(&mut A, &B)>();

    let items = every_way!(iter.iter(), |(a, b): (&mut A, &B)| (a.0, b.0));
    assert_eq!(items, [(e[1], (1, 11)), (e[3], (3, 13))]);

    iter.iter().for_each(|(a, _)| a.0 += 100);
    let items = every_way!(iter.iter(), |(a, b): (&mut A, &B)| (a.0, b.0));
    assert_eq!(items, [(e[1], (101, 11)), (e[3], (103, 13))]);
}

/// Asserts that an iterator dropped after a single item doesn't shorten the next one.
#[test]
fn early_drop() {
    let (world, _) = world();
    let mut iter = world.iter::<(&mut A, &B)>();

    let mut first = iter.iter();
    assert!(first.next().is_some());
    drop(first);

    assert_eq!(iter.iter().count(), 2);
}

/// Asserts that a single window `World::iter` reports its exact length every time.
#[test]
fn exact_len() {
    let (world, _) = world();
    let mut iter = world.iter::<&A>();

    let mut first = iter.iter();
    first.next();
    assert_eq!(first.len(), 3);

    assert_eq!(iter.iter().len(), 4);
}

/// Asserts that a callback panicking during bulk consumption leaves `World::iter` usable,
/// and that dropping it afterwards releases the storages.
#[test]
fn callback_unwind() {
    let (world, _) = world();
    let mut iter = world.iter::<(&mut A, &B)>();

    let result = catch_unwind(AssertUnwindSafe(|| {
        iter.iter().for_each(|(a, _)| {
            if a.0 == 3 {
                panic!("callback panic");
            }
        })
    }));
    assert!(result.is_err());

    assert_eq!(iter.iter().count(), 2);

    drop(iter);
    assert!(world.borrow::<ViewMut<A>>().is_ok());
}

/// Asserts that unwinding out of a scope holding a `World::iter` releases its storages.
#[test]
fn unwind_releases_borrows() {
    let (world, e) = world();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut iter = world.iter::<(&mut A, &B)>();

        iter.iter().with_id().fold((), |(), (id, _)| {
            if id == e[3] {
                panic!("callback panic");
            }
        });
    }));
    assert!(result.is_err());

    assert!(world.borrow::<ViewMut<A>>().is_ok());
    assert!(world.borrow::<ViewMut<B>>().is_ok());
}

/// Asserts that `World::iter` keeps its storages borrowed until it's dropped,
/// not only while one of its iterators lives.
#[test]
fn borrows_held() {
    let (world, _) = world();
    let mut iter = world.iter::<(&mut A, &B)>();

    assert_eq!(iter.iter().count(), 2);
    assert!(world.borrow::<View<A>>().is_err());
    assert!(world.borrow::<ViewMut<B>>().is_err());

    drop(iter);
    assert!(world.borrow::<ViewMut<A>>().is_ok());
    assert!(world.borrow::<ViewMut<B>>().is_ok());
}

/// Asserts that iterating a storage both exclusively and shared is rejected.
#[test]
#[should_panic(expected = "while it's already mutably borrowed")]
fn mut_and_shared_duplicate() {
    let (world, _) = world();

    let _ = world.iter::<(&mut A, &A)>();
}

/// Asserts that iterating a storage exclusively twice is rejected.
#[test]
#[should_panic(expected = "Cannot mutably borrow")]
fn mut_duplicate() {
    let (world, _) = world();

    let _ = world.iter::<(&mut A, &mut A)>();
}

/// Asserts that `Mut` outputs of `World::iter` only flag what's written through them,
/// across repeated iterations.
#[test]
fn mut_flags() {
    let (world, e) = world();
    let mut iter = world.iter::<(Mut<Modif>, &B)>();

    let items = every_way!(iter.iter(), |(modif, b): (Mut<Modif>, &B)| (modif.0, b.0));
    assert_eq!(items, [(e[1], (1, 11)), (e[3], (3, 13))]);

    iter.iter().with_id().for_each(|(id, (mut modif, _))| {
        if id == e[3] {
            modif.0 += 100;
        }
    });
    drop(iter);

    let modif = world.borrow::<View<Modif>>().unwrap();
    assert!(!modif.is_modified(e[0]));
    assert!(!modif.is_modified(e[1]));
    assert!(modif.is_modified(e[3]));
}

/// Asserts that every iterator of the same `AllStorages::iter` starts over from the first entity.
#[test]
fn all_storages_repeat() {
    let (world, e) = world();
    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let mut iter = all_storages.iter::<(&mut A, &B)>();

    let items = every_way!(iter.iter(), |(a, b): (&mut A, &B)| (a.0, b.0));

    assert_eq!(items, [(e[1], (1, 11)), (e[3], (3, 13))]);
}
