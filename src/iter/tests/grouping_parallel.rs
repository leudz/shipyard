//! The parallel producers through Rayon's traits, and whole traversals bridged by Rayon.
//!
//! How a split shares candidates, follow-ups and the plan is tested without Rayon in
//! `grouping_split_ownership`, whose fixtures we reuse. Here we check that the producers use
//! that split, that `fold_with` honors cancellation and that Rayon's own scheduling,
//! whatever order it picks, yields every entity once.//! Exact traversals without follow-ups are driven as indexed, Rayon picks their split indices.

use super::grouping_bulk::cursor;
use super::grouping_split_ownership::{
    coverage_plan, coverage_world, pair, split_all, window_entered, window_expected, window_plan,
    window_world, A, B,
};
use crate::entity_id::EntityId;
use crate::iter::parallel::ExactProducer;
use crate::iter::{
    EntityCursor, IntoIter, Mixed, ParShiperator, RawEntityIdAccess, Shiper, WithId,
};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use rayon::iter::plumbing::{Folder, Producer, UnindexedProducer};
use rayon::iter::ParallelIterator;
use rayon::ThreadPoolBuilder;

/// Folder collecting items until it holds `limit` of them.
struct Collect<T> {
    items: Vec<T>,
    limit: usize,
}

impl<T> Folder<T> for Collect<T> {
    type Result = Vec<T>;

    fn consume(mut self, item: T) -> Self {
        self.items.push(item);
        self
    }

    fn complete(self) -> Vec<T> {
        self.items
    }

    fn full(&self) -> bool {
        self.items.len() >= self.limit
    }
}

/// Folds `producer` with a folder holding at most `limit` items.
fn fold_with<P: UnindexedProducer>(producer: P, limit: usize) -> Vec<P::Item> {
    producer
        .fold_with(Collect {
            items: Vec::new(),
            limit,
        })
        .complete()
}

/// Folds the indexed `producer` with a folder holding every item.
fn fold_exact<P: Producer>(producer: P) -> Vec<P::Item> {
    producer
        .fold_with(Collect {
            items: Vec::new(),
            limit: usize::MAX,
        })
        .complete()
}

fn ids<T>(items: Vec<(EntityId, T)>) -> Vec<EntityId> {
    items.into_iter().map(|(id, _)| id).collect()
}

/// Asserts that the producer doesn't split a single candidate in front of a 7 candidates
/// follow-up and still folds all 8 items.
#[test]
fn producer_no_split() {
    let (world, _) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (iter, right) = window_entered(&a, &[1, 7]).split();

    assert!(right.is_none());
    let values: Vec<_> = fold_with(iter, usize::MAX).iter().map(|a| a.0).collect();
    assert_eq!(values, [0, 1, 2, 3, 4, 5, 6, 7]);
}

/// Asserts that the `WithId` producer splits 5 candidates 2 and 3, the right half keeping
/// the follow-up, with ids matching their components.
#[test]
fn producer_halves() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let (left, right) = WithId(window_entered(&a, &[5, 2])).split();
    let right = right.unwrap();

    assert_eq!(right.0.cursor.local_index(), 2);
    let left: Vec<_> = fold_with(left, usize::MAX);
    let right: Vec<_> = fold_with(right, usize::MAX);
    for (id, a) in left.iter().chain(&right) {
        assert_eq!(e[a.0 as usize], *id);
    }
    assert_eq!(ids(left), e[..2]);
    assert_eq!(ids(right), e[2..7]);
}

/// Asserts that splitting through the producer until it can't anymore gives the same pieces
/// as the split it uses: after a 2 items prefix, every remaining candidate exactly once.
#[test]
fn producer_recursive() {
    fn split_producer<P: UnindexedProducer>(producer: P, pieces: &mut Vec<P>) {
        match producer.split() {
            (left, Some(right)) => {
                split_producer(left, pieces);
                split_producer(right, pieces);
            }
            (producer, None) => pieces.push(producer),
        }
    }

    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let iter = || {
        let mut iter = window_entered(&a, &[7, 1]);
        assert_eq!(iter.by_ref().take(2).count(), 2);

        iter
    };

    let mut producers = Vec::new();
    split_producer(WithId(iter()), &mut producers);
    let mut shipers = Vec::new();
    split_all(iter(), &mut shipers);

    assert_eq!(producers.len(), shipers.len());
    let items: Vec<_> = producers
        .into_iter()
        .flat_map(|producer| fold_with(producer, usize::MAX))
        .map(|(id, a)| (id, a.0))
        .collect();
    assert_eq!(items, window_expected(&e, 2, 8));
}

/// Asserts that `fold_with` stops once the folder is full, here in the middle of the follow-up,
/// instead of folding every item left.
#[test]
fn fold_with_full() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let values: Vec<_> = fold_with(window_entered(&a, &[3, 5]), 5)
        .iter()
        .map(|a| a.0)
        .collect();
    assert_eq!(values, [0, 1, 2, 3, 4]);

    let items = fold_with(WithId(window_entered(&a, &[3, 5])), 5);
    assert_eq!(ids(items), e[..5]);
}

/// Asserts that Rayon bridging a plan with coverage and a segment without match yields every
/// entity once, with or without ids, whatever order the threads walk the pieces in.
#[test]
fn bridge_coverage() {
    let (world, e) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = || ParShiperator(coverage_plan(pair(&a, &b), &a.dense, &b.dense));
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

    let (mut items, mut values) = pool.install(|| {
        let items: Vec<_> = iter()
            .with_id()
            .map(|(id, (a, b))| (id, a.0, b.0))
            .collect();
        let values: Vec<_> = iter().map(|(a, b)| (a.0, b.0)).collect();

        (items, values)
    });
    items.sort_unstable_by_key(|&(_, a, _)| a);
    values.sort_unstable();

    let expected: Vec<_> = (0..6u32).map(|i| (e[i as usize], i, 10 + i)).collect();
    assert_eq!(items, expected);
    let expected: Vec<_> = (0..6).map(|i| (i, 10 + i)).collect();
    assert_eq!(values, expected);
}

/// Asserts that mutable outputs bridged by Rayon over a plan with coverage are written exactly
/// once each: a covered duplicate would be written twice.
#[test]
fn bridge_mutable() {
    let (world, e) = coverage_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let a_dense = a.dense.clone();
    let b_dense = b.dense.clone();
    let windows = Mixed::new(
        (FullRawWindowMut::new(&mut a), FullRawWindowMut::new(&mut b)),
        0b01,
    );
    let iter = ParShiperator(coverage_plan(windows, &a_dense, &b_dense));
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

    pool.install(|| {
        iter.for_each(|(a, b)| {
            a.0 += 100;
            b.0 += 100;
        })
    });

    for (i, &id) in e.iter().enumerate() {
        assert_eq!((&a).get(id).unwrap(), &A(100 + i as u32));
        assert_eq!((&b).get(id).unwrap(), &B(110 + i as u32));
    }
}

/// Asserts that a short-circuiting consumer stops the traversal: on a single thread the
/// iterator splits once, the left half stops after finding `e1` and the right half is skipped.
/// Ignoring cancellation would also visit `e2`, then the right half.
#[test]
fn bridge_short_circuit() {
    let (world, _) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let visited = AtomicUsize::new(0);
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let found = pool.install(|| {
        ParShiperator(window_entered(&a, &[6, 2]))
            .inspect(|_| {
                visited.fetch_add(1, Ordering::Relaxed);
            })
            .find_any(|a| a.0 == 1)
    });

    assert_eq!(found, Some(&A(1)));
    assert_eq!(visited.load(Ordering::Relaxed), 2);
}

/// Asserts that `opt_len` only reports a length once every item left is in the current segment:
/// a window with a follow-up has none, until it enters its last segment.
#[test]
fn opt_len_follow_ups() {
    let (world, _) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let iter = || {
        let schedule = window_plan(&a, &[3, 2]).schedule;

        Shiper::new(
            FullRawWindow::from_view(&a),
            RawEntityIdAccess {
                cursor: EntityCursor::empty(),
                schedule,
                metadata: Default::default(),
            },
        )
    };
    let last_segment = || {
        let mut iter = iter();
        assert_eq!(iter.by_ref().take(4).count(), 4);

        iter
    };

    assert!(iter().is_exact_sized);
    assert_eq!(ParShiperator(iter()).opt_len(), None);
    assert_eq!(ParShiperator(last_segment()).opt_len(), Some(1));
    assert_eq!(ParShiperator(last_segment()).with_id().opt_len(), Some(1));
}

/// Asserts that an exact `Mixed` with follow-ups reports no length, a later segment might reject.
#[test]
fn opt_len_mixed() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let aligned = Mixed::new(
        (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
        0b11,
    );
    let schedule = coverage_plan(pair(&a, &b), &a.dense, &b.dense).schedule;

    let iter = Shiper::new(
        aligned,
        RawEntityIdAccess {
            cursor: EntityCursor::empty(),
            schedule,
            metadata: Default::default(),
        },
    );

    assert_eq!(ParShiperator(iter).opt_len(), None);
}

/// Asserts that the exact producer splits at the index Rayon asks for, even an uneven one after
/// a 2 items prefix, with or without ids.
#[test]
fn exact_split_at() {
    let (world, e) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let iter = || {
        let mut iter = (&a).iter();
        assert_eq!(iter.by_ref().take(2).count(), 2);

        iter
    };

    let (left, right) = ExactProducer(iter()).split_at(2);
    let left: Vec<_> = fold_exact(left).iter().map(|a| a.0).collect();
    let right: Vec<_> = fold_exact(right).iter().map(|a| a.0).collect();
    assert_eq!(left, [1, 2]);
    assert_eq!(right, [3, 4, 5, 6, 7]);

    let (left, right) = ExactProducer(WithId(iter())).split_at(5);
    assert_eq!(ids(fold_exact(left)), e[1..6]);
    assert_eq!(ids(fold_exact(right)), e[6..]);
}

/// `e0` to `e199` own `A(i)` and `B(i)`, at index `i` of both storages.
fn aligned_world() -> World {
    let mut world = World::new();

    for i in 0..200 {
        world.add_entity((A(i), B(i)));
    }

    world
}

/// Asserts that an exact `Mixed`, every child driving its only segment, is collected through
/// the indexed path: items come out in traversal order, ids matching their components.
#[test]
fn indexed_collect() {
    let world = aligned_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = || {
        let aligned = Mixed::new(
            (FullRawWindow::from_view(&a), FullRawWindow::from_view(&b)),
            0b11,
        );

        Shiper::new(
            aligned,
            RawEntityIdAccess {
                cursor: cursor(&a.dense),
                schedule: None,
                metadata: Default::default(),
            },
        )
    };
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

    assert_eq!(ParShiperator(iter()).opt_len(), Some(200));
    let (items, values) = pool.install(|| {
        let items: Vec<_> = ParShiperator(iter())
            .with_id()
            .map(|(id, (a, b))| (id, a.0, b.0))
            .collect();
        let values: Vec<_> = ParShiperator(iter()).map(|(a, b)| (a.0, b.0)).collect();

        (items, values)
    });

    let sequential: Vec<_> = iter()
        .with_id()
        .map(|(id, (a, b))| (id, a.0, b.0))
        .collect();
    assert_eq!(items, sequential);
    assert_eq!(values, (0..200).map(|i| (i, i)).collect::<Vec<_>>());
}

/// Asserts that the indexed path honors cancellation too: on a single thread the 9 items split
/// 4 and 5, the left half stops after finding `e0` behind `A(100)` and the right half is skipped.
#[test]
fn indexed_short_circuit() {
    let (world, _) = window_world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let visited = AtomicUsize::new(0);
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let found = pool.install(|| {
        ParShiperator((&a).iter())
            .inspect(|_| {
                visited.fetch_add(1, Ordering::Relaxed);
            })
            .find_any(|a| a.0 == 0)
    });

    assert_eq!(found, Some(&A(0)));
    assert_eq!(visited.load(Ordering::Relaxed), 2);
}
