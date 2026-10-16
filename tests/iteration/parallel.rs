//! Parallel traversals compared with sequential ones, entity by entity.
//!
//! Rayon picks the order, so items are sorted before being compared.

use core::sync::atomic::{AtomicUsize, Ordering};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use shipyard::iter::OneOfTwo;
use shipyard::*;

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
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::Modification;
}

/// `e0` to `e499` own `A(i)`, every third one also owns `B(i)`.
/// Then 300 entities only own `B(1000 + i)`.
///
/// `B` is the smaller storage, it captains `(&A, &B)` and rejects its last 300 candidates.
fn world() -> World {
    let mut world = World::new();

    let e: Vec<EntityId> = (0..500).map(|i| world.add_entity(A(i))).collect();
    for (i, &id) in e.iter().enumerate().step_by(3) {
        world.add_component(id, B(i as u32));
    }
    for i in 0..300 {
        world.add_entity(B(1000 + i));
    }

    world
}

fn sorted<T: Ord>(mut items: Vec<T>) -> Vec<T> {
    items.sort_unstable();
    items
}

fn or_value(item: OneOfTwo<&A, &B>) -> u32 {
    match item {
        OneOfTwo::One(a) => a.0,
        OneOfTwo::Two(b) => b.0,
    }
}

/// Asserts that a single storage yields the same ids and values in parallel and sequentially.
#[test]
fn single() {
    let world = world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let parallel: Vec<_> = a.par_iter().with_id().map(|(id, a)| (id, a.0)).collect();
    let sequential: Vec<_> = a.iter().with_id().map(|(id, a)| (id, a.0)).collect();
    assert_eq!(sorted(parallel), sorted(sequential));

    let parallel: Vec<_> = a.par_iter().map(|a| a.0).collect();
    assert_eq!(sorted(parallel), (0..500).collect::<Vec<_>>());
}

/// Asserts that a pair whose captain rejects candidates yields the same ids and values
/// in parallel and sequentially.
#[test]
fn mixed() {
    let world = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let parallel: Vec<_> = (&a, &b)
        .par_iter()
        .with_id()
        .map(|(id, (a, b))| (id, a.0, b.0))
        .collect();
    let sequential: Vec<_> = (&a, &b)
        .iter()
        .with_id()
        .map(|(id, (a, b))| (id, a.0, b.0))
        .collect();
    assert_eq!(sequential.len(), 167);
    assert_eq!(sorted(parallel), sorted(sequential));

    let parallel: Vec<_> = (&a, &b).par_iter().map(|(a, b)| (a.0, b.0)).collect();
    let sequential: Vec<_> = (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect();
    assert_eq!(sorted(parallel), sorted(sequential));
}

/// Asserts that `Or` yields the same ids and values in parallel and sequentially,
/// across its transition to the second branch.
#[test]
fn or() {
    let world = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    let parallel: Vec<_> = (&a | &b)
        .par_iter()
        .with_id()
        .map(|(id, item)| (id, or_value(item)))
        .collect();
    let sequential: Vec<_> = (&a | &b)
        .iter()
        .with_id()
        .map(|(id, item)| (id, or_value(item)))
        .collect();
    assert_eq!(sequential.len(), 800);
    assert_eq!(sorted(parallel), sorted(sequential));

    let parallel: Vec<_> = (&a | &b).par_iter().map(or_value).collect();
    let sequential: Vec<_> = (&a | &b).iter().map(or_value).collect();
    assert_eq!(sorted(parallel), sorted(sequential));
}

/// Asserts that writing through parallel mutable outputs gives the same components as
/// writing through sequential ones: every accepted entity is written once.
#[test]
fn mutable() {
    let parallel_world = world();
    let sequential_world = world();

    parallel_world.run(|(mut a, b): (ViewMut<'_, A>, View<'_, B>)| {
        (&mut a, &b).par_iter().for_each(|(a, b)| a.0 += b.0 + 1);
    });
    sequential_world.run(|(mut a, b): (ViewMut<'_, A>, View<'_, B>)| {
        (&mut a, &b).iter().for_each(|(a, b)| a.0 += b.0 + 1);
    });

    let values = |world: &World| {
        let a = world.borrow::<View<'_, A>>().unwrap();
        let values: Vec<_> = a.iter().with_id().map(|(id, a)| (id, a.0)).collect();

        sorted(values)
    };
    assert_eq!(values(&parallel_world), values(&sequential_world));
}

/// Asserts that parallel tracked outputs flag the same entities as sequential ones:
/// only the components written through their `Mut`.
#[test]
fn tracked() {
    let build = || {
        let mut world = World::new();
        for i in 0..300 {
            world.add_entity(Tracked(i));
        }

        world
    };
    let parallel_world = build();
    let sequential_world = build();

    parallel_world.run(|mut tracked: ViewMut<'_, Tracked, track::Modification>| {
        (&mut tracked).par_iter().for_each(|mut tracked| {
            if tracked.0 % 4 == 0 {
                tracked.0 += 1;
            }
        });
    });
    sequential_world.run(|mut tracked: ViewMut<'_, Tracked, track::Modification>| {
        (&mut tracked).iter().for_each(|mut tracked| {
            if tracked.0 % 4 == 0 {
                tracked.0 += 1;
            }
        });
    });

    let modified = |world: &World| {
        let tracked = world
            .borrow::<View<'_, Tracked, track::Modification>>()
            .unwrap();
        let modified: Vec<_> = tracked
            .modified()
            .iter()
            .with_id()
            .map(|(id, tracked)| (id, tracked.0))
            .collect();

        sorted(modified)
    };
    let modified_parallel = modified(&parallel_world);
    assert_eq!(modified_parallel.len(), 75);
    assert_eq!(modified_parallel, modified(&sequential_world));
}

/// Asserts that a short-circuiting consumer stops the traversal. On a single thread the
/// iterator splits once and the left half is walked first: it stops at the match and
/// the right half is skipped.
#[test]
fn short_circuit() {
    let world = world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    // `B` captains, `e0` then `e3` are its first two items.
    let visited = AtomicUsize::new(0);
    let found = pool.install(|| {
        (&a, &b)
            .par_iter()
            .inspect(|_| {
                visited.fetch_add(1, Ordering::Relaxed);
            })
            .find_any(|(a, _)| a.0 == 3)
    });
    assert_eq!(found.map(|(a, b)| (a.0, b.0)), Some((3, 3)));
    assert_eq!(visited.load(Ordering::Relaxed), 2);

    // The first branch drives the current segment, `e0` then `e1`.
    let visited = AtomicUsize::new(0);
    let found = pool.install(|| {
        (&a | &b)
            .par_iter()
            .inspect(|_| {
                visited.fetch_add(1, Ordering::Relaxed);
            })
            .find_any(|item| or_value(*item) == 1)
    });
    assert_eq!(found.map(or_value), Some(1));
    assert_eq!(visited.load(Ordering::Relaxed), 2);
}
