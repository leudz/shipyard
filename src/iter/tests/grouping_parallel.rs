//! Rayon producer contracts and cleanup on publicly constructed iterators.
//!
//! Internal producer calls let us inspect exact split/cancellation behavior; query
//! construction always goes through iter/par_iter, including automatic selection.

use super::grouping_fixtures::{A, B, coverage_world, late_iter, ring_world};
use crate::iter::parallel::ExactProducer;
use crate::iter::{IntoIter, ParShiperator};
use crate::{EntityId, Get, View, ViewMut, World};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use rayon::ThreadPoolBuilder;
use rayon::iter::ParallelIterator;
use rayon::iter::plumbing::{Folder, Producer, UnindexedProducer};

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

#[test]
fn producer_splits() {
    let (world, expected) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    for len in [0, 1] {
        let mut iter = (&a, &b).iter();
        iter.by_ref().take(20 - len).for_each(drop);
        let (iter, right) = iter.split();
        assert!(right.is_none());
        assert_eq!(iter.cursor.len(), len);
        assert_eq!(
            ids(fold_with(iter.with_id(), usize::MAX)),
            expected[20 - len..]
        );
    }
    let (left, right) = late_iter(&a, &b).with_id().split();
    let right = right.unwrap();
    assert_eq!(right.0.cursor.local_index(), 3);
    assert_eq!(ids(fold_with(left, usize::MAX)), expected[20..23]);
    assert_eq!(ids(fold_with(right, usize::MAX)), expected[23..]);
}

#[test]
fn producer_recursive() {
    fn split<P: UnindexedProducer>(p: P, pieces: &mut Vec<P>) {
        match p.split() {
            (left, Some(right)) => {
                split(left, pieces);
                split(right, pieces);
            }
            (p, None) => pieces.push(p),
        }
    }
    let (world, expected) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut pieces = Vec::new();
    split(late_iter(&a, &b).with_id(), &mut pieces);
    assert_eq!(pieces.len(), 6);
    let found: Vec<_> = pieces
        .into_iter()
        .flat_map(|p| ids(fold_with(p, usize::MAX)))
        .collect();
    assert_eq!(found, expected[20..]);
}

#[test]
fn fold_with_full() {
    let (world, expected) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    // Stop inside the current segment, then inside the covered follow-up.
    for limit in [1, 5] {
        assert_eq!(fold_with(late_iter(&a, &b), limit).len(), limit);
        assert_eq!(
            ids(fold_with(late_iter(&a, &b).with_id(), limit)),
            expected[20..20 + limit]
        );
    }
}

#[test]
fn bridge_coverage() {
    let (world, expected) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let (mut items, mut values) = pool.install(|| {
        let items: Vec<_> = (&a, &b)
            .par_iter()
            .with_id()
            .map(|(id, (a, b))| (id, a.0, b.0))
            .collect();
        let values: Vec<_> = (&a, &b).par_iter().map(|(a, b)| (a.0, b.0)).collect();
        (items, values)
    });
    items.sort_unstable_by_key(|&(_, a, _)| a);
    values.sort_unstable();
    assert_eq!(
        items,
        expected
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i as u32, 10 + i as u32))
            .collect::<Vec<_>>()
    );
    assert_eq!(values, (0..26).map(|i| (i, 10 + i)).collect::<Vec<_>>());
}

#[test]
fn bridge_mutable() {
    let (world, ids) = coverage_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    pool.install(|| {
        (&mut a, &mut b).par_iter().for_each(|(a, b)| {
            a.0 += 100;
            b.0 += 100;
        })
    });
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!((&a).get(id).unwrap().0, 100 + i as u32);
        assert_eq!((&b).get(id).unwrap().0, 110 + i as u32);
    }
}

#[test]
fn opt_len_follow_ups() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).iter();
    // The current aligned segment is exact, but later checked segments can reject.
    assert_eq!(iter.cursor.len(), 20);
    assert_eq!(ParShiperator(iter).opt_len(), None);
    let mut last = late_iter(&a, &b);
    assert_eq!(last.by_ref().take(4).count(), 4);
    assert!(last.schedule.as_ref().unwrap().follow_ups().is_empty());
    assert_eq!(ParShiperator(last).with_id().opt_len(), None);
}

fn aligned_world() -> World {
    let mut world = World::new_for_test();
    for i in 0..200 {
        world.add_entity((A(i), B(i)));
    }
    world.group::<(A, B)>();
    world
}

#[test]
fn indexed_collect() {
    let world = aligned_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    assert_eq!((&a, &b).par_iter().opt_len(), Some(200));
    let (items, values) = pool.install(|| {
        let items: Vec<_> = (&a, &b)
            .par_iter()
            .with_id()
            .map(|(id, (a, b))| (id, a.0, b.0))
            .collect();
        let values: Vec<_> = (&a, &b).par_iter().map(|(a, b)| (a.0, b.0)).collect();
        (items, values)
    });
    let sequential: Vec<_> = (&a, &b)
        .iter()
        .with_id()
        .map(|(id, (a, b))| (id, a.0, b.0))
        .collect();
    assert_eq!(items, sequential);
    assert_eq!(values, (0..200).map(|i| (i, i)).collect::<Vec<_>>());
}

#[test]
fn exact_split_at() {
    let world = aligned_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    for index in [0, 5, 198] {
        let mut iter = (&a, &b).iter();
        iter.by_ref().take(2).for_each(drop);
        let (left, right) = ExactProducer(iter).split_at(index);
        assert_eq!(
            fold_exact(left)
                .iter()
                .map(|(a, _)| a.0)
                .collect::<Vec<_>>(),
            (2..2 + index as u32).collect::<Vec<_>>()
        );
        assert_eq!(
            fold_exact(right)
                .iter()
                .map(|(a, _)| a.0)
                .collect::<Vec<_>>(),
            (2 + index as u32..200).collect::<Vec<_>>()
        );

        let mut iter = (&a, &b).iter().with_id();
        iter.by_ref().take(2).for_each(drop);
        let (left, right) = ExactProducer(iter).split_at(index);
        let found = ids(fold_exact(left))
            .into_iter()
            .chain(ids(fold_exact(right)))
            .collect::<Vec<_>>();
        assert_eq!(
            found,
            (&a, &b)
                .iter()
                .with_id()
                .skip(2)
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn bridge_short_circuit() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let visited = AtomicUsize::new(0);
    let found = pool.install(|| {
        (&a, &b)
            .par_iter()
            .inspect(|_| {
                visited.fetch_add(1, Ordering::Relaxed);
            })
            .find_any(|(a, _)| a.0 == 1)
    });
    assert_eq!(found.unwrap().0.0, 1);
    assert_eq!(visited.load(Ordering::Relaxed), 2);
}

#[test]
fn indexed_short_circuit() {
    let world = aligned_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let visited = AtomicUsize::new(0);
    let found = pool.install(|| {
        (&a, &b)
            .par_iter()
            .inspect(|_| {
                visited.fetch_add(1, Ordering::Relaxed);
            })
            .find_any(|(a, _)| a.0 == 1)
    });
    assert_eq!(found.unwrap().0.0, 1);
    assert_eq!(visited.load(Ordering::Relaxed), 2);
}
/// Increments its counter when dropped, from whichever thread drops it.
struct Dropped<'a>(&'a AtomicUsize);

impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// Asserts that a `for_each` callback panicking on `e24`, in the follow-up B walks, propagates
/// out of Rayon, drops the callback exactly once and releases the plan every piece shared.
#[cfg(feature = "std")]
#[test]
fn bridge_panic() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).par_iter();
    let plan = iter.0.schedule.as_ref().unwrap().downgrade();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let guard_drops = AtomicUsize::new(0);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let guard = Dropped(&guard_drops);

        pool.install(move || {
            iter.for_each(move |(a, _)| {
                let _ = &guard;
                if a.0 == 24 {
                    panic!("callback panic on e24");
                }
            })
        });
    }));

    assert!(result.is_err());
    assert_eq!(guard_drops.load(Ordering::Relaxed), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that when a `fold` callback panics on `e24`, every accumulated value Rayon's pieces
/// created is dropped exactly once and the plan is released.
#[cfg(feature = "std")]
#[test]
fn bridge_fold_panic() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let iter = (&a, &b).par_iter();
    let plan = iter.0.schedule.as_ref().unwrap().downgrade();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let created = AtomicUsize::new(0);
    let dropped = AtomicUsize::new(0);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        pool.install(|| {
            iter.fold(Vec::new, |mut acc, (a, _)| {
                if a.0 == 24 {
                    panic!("callback panic on e24");
                }

                created.fetch_add(1, Ordering::Relaxed);
                acc.push(Dropped(&dropped));
                acc
            })
            .for_each(drop)
        });
    }));

    assert!(result.is_err());
    assert_eq!(
        dropped.load(Ordering::Relaxed),
        created.load(Ordering::Relaxed)
    );
    assert!(plan.upgrade().is_none());
}

/// Asserts that `ring_world`'s pair walks, spread over several ranges and cut at wrap points,
/// yield every pair once in parallel, with no exact length to report.
#[test]
fn ring_collect() {
    let (world, pairs) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    assert!((&a, &b).iter().schedule.is_some());
    assert_eq!((&a, &b).par_iter().opt_len(), None);

    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let mut items: Vec<_> = pool.install(|| {
        (&a, &b)
            .par_iter()
            .with_id()
            .map(|(id, (a, b))| (id, a.0, b.0))
            .collect()
    });
    items.sort_unstable_by_key(|&(_, a, _)| a);

    let expected: Vec<_> = (0..).zip(&pairs).map(|(i, &id)| (id, i, 100 + i)).collect();
    assert_eq!(items, expected);
}

/// Asserts that parallel mutable outputs of `ring_world`'s pair walks write every pair once.
#[test]
fn ring_mutable() {
    let (world, pairs) = ring_world();
    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();

    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    pool.install(|| {
        (&mut a, &mut b).par_iter().for_each(|(a, b)| {
            a.0 += 1000;
            b.0 += 1000;
        })
    });

    for (i, &id) in (0..).zip(&pairs) {
        assert_eq!((&a).get(id).unwrap().0, 1000 + i);
        assert_eq!((&b).get(id).unwrap().0, 1100 + i);
    }
}

/// Asserts that a parallel search over `ring_world`'s pair walks can stop early and still
/// finds a pair from each kind of segment.
#[test]
fn ring_short_circuit() {
    let (world, pairs) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

    // Pair 3 is in AB, 9 in ABC and 14 unevaluated.
    for i in [3, 9, 14] {
        let found = pool.install(|| {
            (&a, &b)
                .par_iter()
                .with_id()
                .find_any(|(_, (a, _))| a.0 == i)
                .map(|(id, _)| id)
        });

        assert_eq!(found, Some(pairs[i as usize]));
    }
}

/// Asserts that parallel writes across a damaged ring, C's trust endpoint and covered
/// unevaluated candidates update every component exactly once.
#[test]
fn trust_mutable() {
    use super::grouping_fixtures::{C, trust_world};
    use super::grouping_regions::grouped;
    use crate::iter::IntoShiperator;

    let (world, ids) = trust_world();
    let (mut a, mut b, mut c) = world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
        .unwrap();
    c.groups.ranges[0].trust = 2;
    let iter = ParShiperator(grouped((&mut a, &mut b, &mut c).into_shiperator()));
    assert_eq!(iter.opt_len(), None);

    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    pool.install(|| {
        iter.for_each(|(a, b, c)| {
            a.0 += 1000;
            b.0 += 1000;
            c.0 += 1000;
        })
    });
    for (i, &id) in (0..).zip(&ids) {
        assert_eq!((&a).get(id).unwrap().0, 1000 + i);
        assert_eq!((&b).get(id).unwrap().0, 1100 + i);
        assert_eq!((&c).get(id).unwrap().0, 1200 + i);
    }
}
