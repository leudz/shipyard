//! Bulk strategy, callback lifetime and prefix checks on automatically selected groups.
//!
//! General output, mutable tracking, ZST and adapter checks live in the public
//! `tests/iteration` suite. These tests additionally inspect strategy and plan ownership.

use super::grouping_fixtures::{coverage_world, A, B};
use crate::iter::mixed::BulkStrategy;
use crate::iter::{IntoIter, Shiper, Shiperator};
use crate::{track, Component, EntityId, View};
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
/// Asserts each way ends up with exactly the items of a full traversal, in order.
fn assert_prefix<S, T>(iter: impl Fn() -> Shiper<S>, value: impl Fn(S::Out) -> T, prefix: usize)
where
    S: Shiperator,
    T: PartialEq + Debug + Clone,
{
    let all = every_way(&iter, &value);
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
    assert_eq!(iter.next().unwrap().0 .0, 20);
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::SingleCaptain);
    assert_eq!(iter.by_ref().take(2).count(), 2);
    assert_eq!(iter.next().unwrap().0 .0, 23);
    assert_eq!(iter.shiperator.bulk_strategy(), BulkStrategy::Checked);
    assert!(iter.schedule.as_ref().unwrap().follow_ups().is_empty());

    let expected: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, (i as u32, 10 + i as u32)))
        .collect();
    assert_eq!(every_way(|| (&a, &b).iter(), |(a, b)| (a.0, b.0)), expected);
    // Includes both boundaries, each segment's interior, the rejected tail and exhaustion.
    for prefix in [0, 1, 19, 20, 21, 23, 24, 26, 28] {
        assert_prefix(|| (&a, &b).iter(), |(a, b)| (a.0, b.0), prefix);
    }
}

struct C;
impl Component for C {
    type Tracking = track::Untracked;
}

#[test]
fn partial_strategy() {
    let mut world = crate::World::new();
    let ids: Vec<_> = (0..10)
        .map(|i| world.add_entity((A(i), B(10 + i))))
        .collect();
    world.group::<(A, B)>();
    world.add_component(ids[1], C);
    // Keep the ordinary C walk more expensive than the grouped alternative.
    for _ in 0..100 {
        world.add_entity(C);
    }
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let iter = (&a, &b, &c).iter();
    assert!(iter.schedule.is_some());
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
