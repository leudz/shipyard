//! Bulk strategy, callback lifetime and prefix checks on automatically selected groups.
//!
//! General output, mutable tracking, ZST and adapter checks live in the public
//! `tests/iteration` suite. These tests additionally inspect strategy and plan ownership.

use super::grouping_fixtures::{A, B, C, RING_PAIRS, coverage_world, ring_world, trust_world};
use super::grouping_regions::grouped;
use crate::iter::mixed::BulkStrategy;
use crate::iter::{IntoIter, IntoShiperator, Shiper, Shiperator};
use crate::{EntityId, Get, View, ViewMut};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::fmt::{Debug, Write};

/// Consumes the iterators `iter` builds in every way and asserts they all yield
/// the same items in the same order.
///
/// Returns the items with their ids, as `WithId::next` yields them.
fn every_way<S, T>(iter: impl Fn() -> Shiper<S>, value: impl Fn(S::Out) -> T) -> Vec<(EntityId, T)>
where
    S: Shiperator,
    T: PartialEq + Debug + Clone,
{
    // `for` loops only call `next`.
    let mut with_ids = Vec::new();
    for (id, item) in iter().with_id() {
        with_ids.push((id, value(item)));
    }
    let ids: Vec<EntityId> = with_ids.iter().map(|(id, _)| *id).collect();
    let values: Vec<T> = with_ids.iter().map(|(_, value)| value.clone()).collect();

    let mut next = Vec::new();
    for item in iter() {
        next.push(value(item));
    }
    assert_eq!(next, values, "next");

    let fold = iter().fold(Vec::new(), |mut acc, item| {
        acc.push(value(item));
        acc
    });
    assert_eq!(fold, values, "fold");

    let mut for_each = Vec::new();
    iter().for_each(|item| for_each.push(value(item)));
    assert_eq!(for_each, values, "for_each");

    let id_fold = iter().with_id().fold(Vec::new(), |mut acc, (id, item)| {
        acc.push((id, value(item)));
        acc
    });
    assert_eq!(id_fold, with_ids, "WithId fold");

    let mut id_for_each = Vec::new();
    iter()
        .with_id()
        .for_each(|(id, item)| id_for_each.push((id, value(item))));
    assert_eq!(id_for_each, with_ids, "WithId for_each");

    let mut only_ids = Vec::new();
    iter().ids().for_each(|id| only_ids.push(id));
    assert_eq!(only_ids, ids, "ids");

    with_ids
}

/// Takes `prefix` items with `next` then consumes the rest in bulk, in every way.
///
/// Asserts each way ends up with exactly `all`, the items of a full traversal in order,
/// as `every_way` returns them.
fn assert_prefix<S, T>(
    iter: impl Fn() -> Shiper<S>,
    value: impl Fn(S::Out) -> T,
    all: &[(EntityId, T)],
    prefix: usize,
) where
    S: Shiperator,
    T: PartialEq + Debug + Clone,
{
    let all_values: Vec<T> = all.iter().map(|(_, value)| value.clone()).collect();

    let head_with_ids = || {
        let mut iter = iter().with_id();
        let head: Vec<(EntityId, T)> = (&mut iter)
            .take(prefix)
            .map(|(id, item)| (id, value(item)))
            .collect();

        (head, iter)
    };
    let head_values = || {
        let mut iter = iter();
        let head: Vec<T> = (&mut iter).take(prefix).map(&value).collect();

        (head, iter)
    };

    let (head, rest) = head_values();
    let fold = rest.fold(head, |mut acc, item| {
        acc.push(value(item));
        acc
    });
    assert_eq!(fold, all_values, "fold after {prefix}");

    let (mut for_each, rest) = head_values();
    rest.for_each(|item| for_each.push(value(item)));
    assert_eq!(for_each, all_values, "for_each after {prefix}");

    let (head, rest) = head_with_ids();
    let id_fold = rest.fold(head, |mut acc, (id, item)| {
        acc.push((id, value(item)));
        acc
    });
    assert_eq!(id_fold, all, "WithId fold after {prefix}");

    let (mut id_for_each, rest) = head_with_ids();
    rest.for_each(|(id, item)| id_for_each.push((id, value(item))));
    assert_eq!(id_for_each, all, "WithId for_each after {prefix}");
}

#[test]
fn strategies_and_prefixes() {
    let (world, ids) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = (&a, &b).iter();
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::Aligned);
    assert_eq!(iter.by_ref().take(20).count(), 20);
    assert_eq!(iter.next().unwrap().0.0, 20);
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::SingleCaptain);
    assert_eq!(iter.by_ref().take(2).count(), 2);
    assert_eq!(iter.next().unwrap().0.0, 23);
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::Checked);
    assert!(!iter.schedule.as_ref().unwrap().has_follow_ups());

    let expected: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, (i as u32, 10 + i as u32)))
        .collect();
    assert_eq!(every_way(|| (&a, &b).iter(), |(a, b)| (a.0, b.0)), expected);
    // Includes both boundaries, each segment's interior, the rejected tail and exhaustion.
    for prefix in [0, 1, 19, 20, 21, 23, 24, 26, 28] {
        assert_prefix(|| (&a, &b).iter(), |(a, b)| (a.0, b.0), &expected, prefix);
    }
}

#[test]
fn partial_strategy() {
    let mut world = crate::World::new_for_test();
    let ids: Vec<_> = (0..10)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    world.group::<(A, B)>();
    world.add_component(ids[1], C(1));
    // Keep the ordinary C walk more expensive than the grouped alternative.
    for i in 0..100 {
        world.add_entity(C(i));
    }
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = (&a, &b, &c).iter();
    // The aligned AB pair drives a single segment, installed without plan.
    assert_eq!(iter.shiperator.captain_mask, 0b011);
    assert!(iter.schedule.is_none());
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::Partial);
    assert_eq!(
        every_way(|| (&a, &b, &c).iter(), |(a, b, _)| (a.0, b.0)),
        [(ids[1], (1, 11))]
    );
}

#[test]
fn stateful_callbacks() {
    let (world, ids) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut calls = 0;
    let text = (&a, &b).iter().fold(String::new(), |mut acc, (a, b)| {
        assert_eq!(a.0, calls);
        calls += 1;
        write!(acc, "{}:{} ", a.0, b.0).unwrap();
        acc
    });
    let mut expected = String::new();
    for i in 0..26 {
        write!(expected, "{}:{} ", i, 10 + i).unwrap();
    }
    assert_eq!(calls, 26);
    assert_eq!(text, expected);

    let mut previous = None;
    let mut pairs = Vec::new();
    (&a, &b).iter().with_id().for_each(|(id, (a, _))| {
        pairs.push((previous, id));
        previous = Some(a.0);
    });
    let expected: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (i.checked_sub(1).map(|p| p as u32), id))
        .collect();
    assert_eq!(pairs, expected);
}

/// Increments its counter when dropped.
struct Dropped<'a>(&'a Cell<u32>);

impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

/// Asserts that a fold moving its accumulator and callback through every segment
/// drops neither until it's done, then each exactly once.
#[test]
fn fold_drops() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let item_drops = Cell::new(0);
    let guard_drops = Cell::new(0);
    let guard = Dropped(&guard_drops);
    let item_drops_ref = &item_drops;

    let items = (&a, &b).iter().fold(Vec::new(), move |mut acc, _| {
        let _ = &guard;
        acc.push(Dropped(item_drops_ref));
        acc
    });

    assert_eq!(item_drops.get(), 0);
    assert_eq!(guard_drops.get(), 1);
    assert_eq!(items.len(), 26);

    drop(items);
    assert_eq!(item_drops.get(), 26);
}

/// Asserts that a callback panicking in the second segment drops the accumulator,
/// the callback and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn fold_panic() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let item_drops = Cell::new(0);
    let guard_drops = Cell::new(0);
    let iter = (&a, &b).iter();
    let plan = iter.schedule.as_ref().unwrap().downgrade();

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let guard = Dropped(&guard_drops);
        let item_drops = &item_drops;

        iter.fold(Vec::new(), move |mut acc, (a, _)| {
            let _ = &guard;
            if a.0 == 21 {
                panic!("callback panic on e21");
            }
            acc.push(Dropped(item_drops));
            acc
        })
    }));

    assert!(result.is_err());
    assert_eq!(item_drops.get(), 21);
    assert_eq!(guard_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that a `WithId::for_each` callback panicking in the second segment
/// drops the callback and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn for_each_panic() {
    let (world, _) = coverage_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let guard_drops = Cell::new(0);
    let iter = (&a, &b).iter();
    let plan = iter.schedule.as_ref().unwrap().downgrade();

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let guard = Dropped(&guard_drops);

        iter.with_id().for_each(move |(_, (a, _))| {
            let _ = &guard;
            if a.0 == 21 {
                panic!("callback panic on e21");
            }
        });
    }));

    assert!(result.is_err());
    assert_eq!(guard_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Returns the bulk strategy `strategy` reads for each segment of `iter`, entering them in order.
fn strategies<S: Shiperator>(
    mut iter: Shiper<S>,
    strategy: impl Fn(&S) -> BulkStrategy,
) -> Vec<BulkStrategy> {
    let mut strategies = Vec::new();
    loop {
        strategies.push(strategy(&iter.shiperator));
        if !iter.enter_next_segment() {
            break;
        }
    }

    strategies
}

/// Asserts that each segment of `ring_world`'s pair walks picks its bulk strategy from its own
/// mask and coverage: AB and the 3 pieces of ABC fold without lookups, A's unevaluated walk
/// with its single captain, B's checking coverage.
/// Every way yields every pair once, after any `next` prefix, across every boundary.
#[test]
fn ring_strategies_and_prefixes() {
    let (world, pairs) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();

    assert_eq!(
        strategies((&a, &b).iter(), |mixed| mixed.bulk_strategy()),
        [
            BulkStrategy::Aligned,
            BulkStrategy::Aligned,
            BulkStrategy::Aligned,
            BulkStrategy::Aligned,
            BulkStrategy::SingleCaptain,
            BulkStrategy::Checked,
        ]
    );

    let all = every_way(|| (&a, &b).iter(), |(a, b)| (a.0, b.0));
    let mut items = all.clone();
    items.sort_unstable();
    let mut expected: Vec<_> = (0..)
        .zip(&pairs)
        .map(|(i, &id)| (id, (i, 100 + i)))
        .collect();
    expected.sort_unstable();
    assert_eq!(items, expected);

    // Under Miri, every 7th prefix still ends in AB, ABC and the unevaluated walks.
    let step = if cfg!(miri) { 7 } else { 1 };
    for prefix in (0..=RING_PAIRS + 1).step_by(step) {
        assert_prefix(|| (&a, &b).iter(), |(a, b)| (a.0, b.0), &all, prefix);
    }
}

/// Asserts that the segments of `trust_world`'s walks change bulk strategy with their masks:
/// aligned with every child driving, partial where the damaged walk looks B up, single captain
/// for A's unevaluated walk, checked for B's. Every way yields every match once,
/// after any `next` prefix.
#[test]
fn trust_strategies_and_prefixes() {
    let (world, e) = trust_world();
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = || grouped((&a, &b, &c).into_shiperator());
    let value = |(a, b, c): (&A, &B, &C)| (a.0, b.0, c.0);

    assert_eq!(
        strategies(iter(), |mixed| mixed.bulk_strategy()),
        [
            BulkStrategy::Aligned,
            BulkStrategy::Partial,
            BulkStrategy::Partial,
            BulkStrategy::SingleCaptain,
            BulkStrategy::Checked,
        ]
    );

    let all = every_way(iter, value);
    let mut items = all.clone();
    items.sort_unstable();
    let expected: Vec<_> = (0..)
        .zip(&e)
        .map(|(i, &id)| (id, (i, 100 + i, 200 + i)))
        .collect();
    assert_eq!(items, expected);

    for prefix in 0..=e.len() + 1 {
        assert_prefix(iter, value, &all, prefix);
    }
}

/// Asserts that `ring_world`'s pair walks count their 17 candidates as an upper bound,
/// never claim an exact length, and keep the bound above the pairs left after every `next`.
#[test]
fn ring_size_hint() {
    let (world, _) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let mut iter = (&a, &b).iter();

    // AB, ABC, then A's and B's 2 unevaluated pairs.
    assert_eq!(iter.size_hint(), (0, Some(6 + 7 + 2 + 2)));
    for left in (0..RING_PAIRS).rev() {
        assert!(iter.next().is_some());

        let (lower, upper) = iter.size_hint();
        assert!(lower == 0 && upper.unwrap() >= left, "{left} left");
    }
    assert!(iter.next().is_none());
}

/// Asserts that iterators from the same `World::iter` replay `ring_world`'s pair walks
/// from their first segment every time.
#[test]
fn ring_replay() {
    let (world, pairs) = ring_world();
    let mut expected: Vec<_> = (0..)
        .zip(&pairs)
        .map(|(i, &id)| (id, (i, 100 + i)))
        .collect();
    expected.sort_unstable();

    let mut owned = world.iter::<(&A, &B)>();
    for pass in 0..3 {
        let mut items: Vec<_> = owned
            .iter()
            .with_id()
            .map(|(id, (a, b))| (id, (a.0, b.0)))
            .collect();
        items.sort_unstable();

        assert_eq!(items, expected, "pass {pass}");
    }
}

/// Asserts that a callback panicking inside ABC, past AB and in front of the unevaluated
/// walks, drops the accumulator, the callback and the plan exactly once.
#[cfg(feature = "std")]
#[test]
fn ring_fold_panic() {
    let (world, _) = ring_world();
    let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
    let item_drops = Cell::new(0);
    let guard_drops = Cell::new(0);
    let iter = (&a, &b).iter();
    let plan = iter.schedule.as_ref().unwrap().downgrade();

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let guard = Dropped(&guard_drops);
        let item_drops = &item_drops;

        // Pair 9 is in ABC.
        iter.fold(Vec::new(), move |mut acc, (a, _)| {
            let _ = &guard;
            if a.0 == 9 {
                panic!("callback panic on pair 9");
            }
            acc.push(Dropped(item_drops));
            acc
        })
    }));

    assert!(result.is_err());
    // Every item before the panic was pushed, they're dropped with the accumulator.
    assert!(item_drops.get() >= 6);
    assert_eq!(guard_drops.get(), 1);
    assert!(plan.upgrade().is_none());
}

/// Asserts that a mutable prefix stays usable while bulk traversal crosses C's trust
/// endpoint, wraps and duplicate coverage. Every component is yielded and written once.
#[test]
fn trust_held_prefix() {
    let (world, ids) = trust_world();
    let (mut a, mut b, mut c) = world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
        .unwrap();
    c.groups.ranges[0].trust = 2;

    let mut iter = grouped((&mut a, &mut b, &mut c).into_shiperator()).with_id();
    let first = iter.next().unwrap();
    let rest = iter.fold(Vec::new(), |mut items, item| {
        items.push(item);
        items
    });

    let mut seen = Vec::new();
    for (id, (a, b, c)) in core::iter::once(first).chain(rest) {
        assert!(!seen.contains(&id));
        seen.push(id);
        let i = ids.iter().position(|&expected| expected == id).unwrap() as u32;
        assert_eq!((a.0, b.0, c.0), (i, 100 + i, 200 + i));
        a.0 += 1000;
        b.0 += 1000;
        c.0 += 1000;
    }
    assert_eq!(seen.len(), ids.len());
    for (i, &id) in (0..).zip(&ids) {
        assert_eq!((&a).get(id).unwrap().0, 1000 + i);
        assert_eq!((&b).get(id).unwrap().0, 1100 + i);
        assert_eq!((&c).get(id).unwrap().0, 1200 + i);
    }
}
