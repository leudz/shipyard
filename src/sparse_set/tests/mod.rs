mod grouping;
mod miri;
mod moves;
mod range_moves;
mod removal;
mod replacement;

use super::groups::GroupRange;
use super::*;
use crate::Component;
use alloc::sync::Arc;
use alloc::vec;
use core::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
#[cfg(feature = "std")]
use std::println;

#[derive(PartialEq, Eq, Debug)]
struct STR(&'static str);

impl Component for STR {
    type Tracking = crate::track::Untracked;
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct I32(i32);

impl Component for I32 {
    type Tracking = crate::track::Untracked;
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct TrackedI32(i32);

impl Component for TrackedI32 {
    type Tracking = crate::track::All;
}

/// Component counting its drops, to catch a move losing or duplicating a component.
struct Counted(u64, Arc<AtomicUsize>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.1.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

impl Component for Counted {
    type Tracking = crate::track::Untracked;
}

struct Zst;

impl Component for Zst {
    type Tracking = crate::track::Untracked;
}

/// Builds a storage holding one entity per slot, in physical order, each with `make(index)`.
///
/// `ranges` are `(len, shift)` pairs placed back to back and fully trusted, followed by
/// `ungrouped_len` ungrouped and `unevaluated_len` unevaluated components.\n/// Tracked entities are inserted at timestamp `index` and modified at `100 + index`.
fn storage_with_tails<T: Component>(
    ranges: &[(usize, isize)],
    ungrouped_len: usize,
    unevaluated_len: usize,
    make: impl Fn(u64) -> T,
) -> SparseSet<T> {
    let grouped_len = ranges.iter().map(|&(len, _)| len).sum::<usize>();
    let entity_count = grouped_len + ungrouped_len + unevaluated_len;

    let mut sparse_set = SparseSet::new();
    for index in 0..entity_count as u64 {
        sparse_set
            .insert(
                EntityId::new(index),
                make(index),
                TrackingTimestamp::new(index),
            )
            .assert_inserted();
    }
    if sparse_set.is_tracking_modification {
        for (index, timestamp) in sparse_set.modification_data.iter_mut().enumerate() {
            *timestamp = TrackingTimestamp::new(100 + index as u64);
        }
    }

    let mut start = 0;
    for (position, &(len, shift)) in ranges.iter().enumerate() {
        sparse_set.groups.ranges.push(GroupRange {
            group: Arc::from([StorageId::Custom(0), StorageId::Custom(position as u64 + 1)]),
            start,
            len,
            shift,
            trust: len,
        });
        start += len;
    }
    sparse_set.groups.unevaluated_start = grouped_len + ungrouped_len;

    sparse_set
}

fn ints_with_tails(
    ranges: &[(usize, isize)],
    ungrouped_len: usize,
    unevaluated_len: usize,
) -> SparseSet<I32> {
    storage_with_tails(ranges, ungrouped_len, unevaluated_len, |index| {
        I32(i32::try_from(index).unwrap())
    })
}

/// Panics unless every row still holds its entity's component and timestamps,
/// with the sparse array pointing at it, and the layout is valid.
///
/// `value` reads the entity index a component was built with, `None` when it can't.
fn assert_rows<T: Component>(sparse_set: &SparseSet<T>, value: impl Fn(&T) -> Option<u64>) {
    for (index, entity) in sparse_set.dense.iter().enumerate() {
        assert_eq!(sparse_set.index_of(*entity), Some(index));
        if let Some(value) = value(&sparse_set.data[index]) {
            assert_eq!(value, entity.index());
        }

        if sparse_set.is_tracking_insertion {
            assert_eq!(sparse_set.insertion_data[index].get(), entity.index());
        }
        if sparse_set.is_tracking_modification {
            assert_eq!(
                sparse_set.modification_data[index].get(),
                100 + entity.index()
            );
        }
    }

    #[cfg(debug_assertions)]
    sparse_set.groups.assert_invariants(sparse_set.len());
}

fn int_value(component: &I32) -> Option<u64> {
    Some(u64::try_from(component.0).unwrap())
}

/// Returns the shifts ring sweeps try, negative and positive, past the length too.
///
/// Under Miri, fewer of them still give every remainder for lengths up to 5.
fn shifts() -> Vec<isize> {
    if cfg!(miri) {
        vec![-6, -2, -1, 0, 1, 2, 6]
    } else {
        (-6..=6).collect()
    }
}

#[test]
fn insert() {
    let mut array = SparseSet::new();

    array
        .insert(
            EntityId::new_from_parts(0, 0),
            STR("0"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    assert_eq!(array.dense, &[EntityId::new_from_parts(0, 0)]);
    assert_eq!(array.data, &[STR("0")]);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(0, 0)),
        Some(&STR("0"))
    );

    array
        .insert(
            EntityId::new_from_parts(1, 0),
            STR("1"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(0, 0),
            EntityId::new_from_parts(1, 0)
        ]
    );
    assert_eq!(array.data, &[STR("0"), STR("1")]);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(0, 0)),
        Some(&STR("0"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(1, 0)),
        Some(&STR("1"))
    );

    array
        .insert(
            EntityId::new_from_parts(5, 0),
            STR("5"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(0, 0),
            EntityId::new_from_parts(1, 0),
            EntityId::new_from_parts(5, 0)
        ]
    );
    assert_eq!(array.data, &[STR("0"), STR("1"), STR("5")]);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0)),
        Some(&STR("5"))
    );

    assert_eq!(array.private_get(EntityId::new_from_parts(4, 0)), None);
}

#[test]
fn remove() {
    let mut array = SparseSet::new();
    array
        .insert(
            EntityId::new_from_parts(0, 0),
            STR("0"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    array
        .insert(
            EntityId::new_from_parts(5, 0),
            STR("5"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    array
        .insert(
            EntityId::new_from_parts(10, 0),
            STR("10"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();

    assert_eq!(
        array.dyn_remove(EntityId::new_from_parts(0, 0), TrackingTimestamp::new(0)),
        Some(STR("0")),
    );
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0),
            EntityId::new_from_parts(5, 0)
        ]
    );
    assert_eq!(array.data, &[STR("10"), STR("5")]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0)),
        Some(&STR("5"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0)),
        Some(&STR("10"))
    );

    array
        .insert(
            EntityId::new_from_parts(3, 0),
            STR("3"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    array
        .insert(
            EntityId::new_from_parts(100, 0),
            STR("100"),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0),
            EntityId::new_from_parts(5, 0),
            EntityId::new_from_parts(3, 0),
            EntityId::new_from_parts(100, 0)
        ]
    );
    assert_eq!(array.data, &[STR("10"), STR("5"), STR("3"), STR("100")]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(3, 0)),
        Some(&STR("3"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0)),
        Some(&STR("5"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0)),
        Some(&STR("10"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(100, 0)),
        Some(&STR("100"))
    );

    assert_eq!(
        array.dyn_remove(EntityId::new_from_parts(3, 0), TrackingTimestamp::new(0)),
        Some(STR("3")),
    );
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0),
            EntityId::new_from_parts(5, 0),
            EntityId::new_from_parts(100, 0)
        ]
    );
    assert_eq!(array.data, &[STR("10"), STR("5"), STR("100")]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
    assert_eq!(array.private_get(EntityId::new_from_parts(3, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0)),
        Some(&STR("5"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0)),
        Some(&STR("10"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(100, 0)),
        Some(&STR("100"))
    );

    assert_eq!(
        array.dyn_remove(EntityId::new_from_parts(100, 0), TrackingTimestamp::new(0)),
        Some(STR("100"))
    );
    assert_eq!(
        array.dense,
        &[
            EntityId::new_from_parts(10, 0),
            EntityId::new_from_parts(5, 0)
        ]
    );
    assert_eq!(array.data, &[STR("10"), STR("5")]);
    assert_eq!(array.private_get(EntityId::new_from_parts(0, 0)), None);
    assert_eq!(array.private_get(EntityId::new_from_parts(3, 0)), None);
    assert_eq!(
        array.private_get(EntityId::new_from_parts(5, 0)),
        Some(&STR("5"))
    );
    assert_eq!(
        array.private_get(EntityId::new_from_parts(10, 0)),
        Some(&STR("10"))
    );
    assert_eq!(array.private_get(EntityId::new_from_parts(100, 0)), None);
}

#[test]
fn clear() {
    let mut sparse_set = SparseSet::new();
    sparse_set.track_all();

    sparse_set
        .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
        .assert_inserted();

    sparse_set.private_clear(TrackingTimestamp::new(0));

    assert_eq!(sparse_set.len(), 0);
    assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
    assert_eq!(sparse_set.private_get(EntityId::new(1)), None);
    assert_eq!(sparse_set.insertion_data.len(), 0);
    assert_eq!(sparse_set.modification_data.len(), 0);
    assert_eq!(sparse_set.deletion_data.len(), 2);
    assert_eq!(sparse_set.removal_data.len(), 0);
}

#[test]
fn drain() {
    let mut sparse_set = SparseSet::new();
    sparse_set.track_all();

    sparse_set
        .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
        .assert_inserted();

    let mut drain = sparse_set.private_drain(TrackingTimestamp::new(0));

    assert_eq!(drain.next(), Some(I32(0)));
    assert_eq!(drain.next(), Some(I32(1)));
    assert_eq!(drain.next(), None);

    drop(drain);

    assert_eq!(sparse_set.len(), 0);
    assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
    assert_eq!(sparse_set.private_get(EntityId::new(1)), None);
    assert_eq!(sparse_set.insertion_data.len(), 0);
    assert_eq!(sparse_set.modification_data.len(), 0);
    assert_eq!(sparse_set.deletion_data.len(), 0);
    assert_eq!(sparse_set.removal_data.len(), 2);
}

#[test]
fn drain_with_id() {
    let mut sparse_set = SparseSet::new();

    sparse_set
        .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
        .assert_inserted();

    let mut drain = sparse_set
        .private_drain(TrackingTimestamp::new(0))
        .with_id();

    assert_eq!(drain.next(), Some((EntityId::new(0), I32(0))));
    assert_eq!(drain.next(), Some((EntityId::new(1), I32(1))));
    assert_eq!(drain.next(), None);

    drop(drain);

    assert_eq!(sparse_set.len(), 0);
    assert_eq!(sparse_set.private_get(EntityId::new(0)), None);
}

#[test]
fn drain_empty() {
    let mut sparse_set = SparseSet::<I32>::new();

    assert_eq!(
        sparse_set.private_drain(TrackingTimestamp::new(0)).next(),
        None
    );
    assert_eq!(
        sparse_set
            .private_drain(TrackingTimestamp::new(0))
            .with_id()
            .next(),
        None
    );

    assert_eq!(sparse_set.len(), 0);
}

#[test]
fn unstable_sort() {
    let mut array = SparseSet::new();

    for i in (0..100).rev() {
        let mut entity_id = EntityId::zero();
        entity_id.set_index(100 - i);
        array
            .insert(entity_id, I32(i as i32), TrackingTimestamp::new(0))
            .assert_inserted();
    }

    array.sort_unstable();

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..100 {
        let mut entity_id = EntityId::zero();
        entity_id.set_index(100 - i);
        assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
    }
}

#[test]
fn partially_sorted_unstable_sort() {
    let mut array = SparseSet::new();

    for i in 0..20 {
        let mut entity_id = EntityId::zero();
        entity_id.set_index(i);
        array
            .insert(entity_id, I32(i as i32), TrackingTimestamp::new(0))
            .assert_inserted();
    }
    for i in (20..100).rev() {
        let mut entity_id = EntityId::zero();
        entity_id.set_index(100 - i + 20);
        array
            .insert(entity_id, I32(i as i32), TrackingTimestamp::new(0))
            .assert_inserted();
    }

    array.sort_unstable();

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..20 {
        let mut entity_id = EntityId::zero();
        entity_id.set_index(i);
        assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
    }
    for i in 20..100 {
        let mut entity_id = EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert_eq!(array.private_get(entity_id), Some(&I32(i as i32)));
    }
}

#[test]
fn unstable_sort_preserves_inserted_tracking() {
    let mut array = SparseSet::new();
    let tracked = EntityId::new_from_parts(0, 0);
    let other = EntityId::new_from_parts(1, 0);
    let another = EntityId::new_from_parts(2, 0);
    let last = TrackingTimestamp::new(1);
    let current = TrackingTimestamp::new(2);

    array
        .insert(tracked, TrackedI32(3), current)
        .assert_inserted();
    array.insert(other, TrackedI32(1), last).assert_inserted();
    array.insert(another, TrackedI32(2), last).assert_inserted();

    assert!(<crate::track::All as Tracking>::is_inserted(
        &array, tracked, last, current
    ));
    assert!(!<crate::track::All as Tracking>::is_inserted(
        &array, other, last, current
    ));

    array.sort_unstable();

    assert!(<crate::track::All as Tracking>::is_inserted(
        &array, tracked, last, current
    ));
    assert!(!<crate::track::All as Tracking>::is_inserted(
        &array, other, last, current
    ));
    assert!(!<crate::track::All as Tracking>::is_inserted(
        &array, another, last, current
    ));
}

#[test]
fn unstable_sort_preserves_modified_tracking() {
    let mut array = SparseSet::new();
    let tracked = EntityId::new_from_parts(0, 0);
    let other = EntityId::new_from_parts(1, 0);
    let another = EntityId::new_from_parts(2, 0);
    let last = TrackingTimestamp::new(1);
    let current = TrackingTimestamp::new(2);

    array
        .insert(tracked, TrackedI32(3), TrackingTimestamp::origin())
        .assert_inserted();
    array
        .insert(other, TrackedI32(1), TrackingTimestamp::origin())
        .assert_inserted();
    array
        .insert(another, TrackedI32(2), TrackingTimestamp::origin())
        .assert_inserted();

    let _ = array.insert(tracked, TrackedI32(3), current);

    assert!(<crate::track::All as Tracking>::is_modified(
        &array, tracked, last, current
    ));
    assert!(!<crate::track::All as Tracking>::is_modified(
        &array, other, last, current
    ));

    array.sort_unstable();

    assert!(<crate::track::All as Tracking>::is_modified(
        &array, tracked, last, current
    ));
    assert!(!<crate::track::All as Tracking>::is_modified(
        &array, other, last, current
    ));
    assert!(!<crate::track::All as Tracking>::is_modified(
        &array, another, last, current
    ));
}

#[test]
fn debug() {
    let mut sparse_set = SparseSet::new();

    sparse_set
        .insert(EntityId::new(0), STR("0"), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(5), STR("5"), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(10), STR("10"), TrackingTimestamp::new(0))
        .assert_inserted();

    #[cfg(feature = "std")]
    println!("{:#?}", sparse_set);
}

#[test]
fn multiple_enable_tracking() {
    let mut sparse_set = SparseSet::new();

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(0),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();

    sparse_set.track_all();
    sparse_set.track_all();
    sparse_set.track_all();

    assert_eq!(sparse_set.insertion_data.len(), 1);
    assert_eq!(sparse_set.modification_data.len(), 1);
}
