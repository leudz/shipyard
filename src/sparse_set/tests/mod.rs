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
/// `ungrouped_len` ungrouped and `unevaluated_len` unevaluated components.\
/// Tracked entities are inserted at timestamp `index` and modified at `100 + index`.
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
    for index in 0..sparse_set.len() {
        assert_row(sparse_set, index, &value);
    }

    #[cfg(debug_assertions)]
    sparse_set.groups.assert_invariants(sparse_set.len());
}

/// Same as [`assert_rows`] for the row at physical `index` alone, without checking the layout.
fn assert_row<T: Component>(
    sparse_set: &SparseSet<T>,
    index: usize,
    value: &impl Fn(&T) -> Option<u64>,
) {
    let entity = sparse_set.dense[index];

    assert_eq!(sparse_set.index_of(entity), Some(index));
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

/// Layout of a storage by entity index: each range in logical order with its trust,
/// then the sorted ungrouped and unevaluated regions.
///
/// It's read with the ring formula directly, not with `Ring`, so extraction tests don't rely
/// on the arithmetic the extraction itself uses.
#[derive(Clone, PartialEq, Debug)]
struct Layout {
    ranges: Vec<(Vec<u64>, usize)>,
    ungrouped: Vec<u64>,
    unevaluated: Vec<u64>,
}

fn layout<T: Component>(sparse_set: &SparseSet<T>) -> Layout {
    let groups = &sparse_set.groups;
    let indices = |entities: &[EntityId]| {
        let mut indices: Vec<u64> = entities.iter().map(|entity| entity.index()).collect();
        indices.sort_unstable();
        indices
    };

    let ranges = groups
        .ranges
        .iter()
        .map(|range| {
            let len = isize::try_from(range.len).unwrap();
            let order = (0..len)
                .map(|logical| {
                    let offset = usize::try_from((range.shift + logical).rem_euclid(len)).unwrap();
                    let physical = range.start + offset;
                    sparse_set.dense[physical].index()
                })
                .collect();

            (order, range.trust)
        })
        .collect();

    Layout {
        ranges,
        ungrouped: indices(&sparse_set.dense[groups.ungrouped_range(sparse_set.len())]),
        unevaluated: indices(&sparse_set.dense[groups.unevaluated_range(sparse_set.len())]),
    }
}

/// What an extraction test remembers of a storage before the extraction.
struct Before {
    layout: Layout,
    shifts: Vec<isize>,
    groups: Vec<Arc<[StorageId]>>,
}

impl Before {
    fn of<T: Component>(sparse_set: &SparseSet<T>) -> Before {
        let ranges = &sparse_set.groups.ranges;

        Before {
            layout: layout(sparse_set),
            shifts: ranges.iter().map(|range| range.shift).collect(),
            groups: ranges.iter().map(|range| range.group.clone()).collect(),
        }
    }
}

/// Panics unless `after` is `before` once the component with `entity`'s index left with a
/// physical extraction, and `sparse_set` still has every group attached in the same order.
///
/// The range the component left only keeps the logical positions before both its own and the one
/// in the range's physical last slot. The order after them isn't specified, so we compare it as a set.
/// Every other range keeps its logical order and trust.
fn assert_extracted<T: Component>(
    before: &Before,
    mut after: Layout,
    sparse_set: &SparseSet<T>,
    entity: u64,
    context: &str,
) {
    let mut expected = before.layout.clone();
    expected.ungrouped.retain(|&index| index != entity);
    expected.unevaluated.retain(|&index| index != entity);

    let source = expected
        .ranges
        .iter()
        .position(|(order, _)| order.contains(&entity));
    if let Some(range_index) = source {
        let (order, trust) = &mut expected.ranges[range_index];
        let logical = order.iter().position(|&index| index == entity).unwrap();
        let len = order.len();
        let shift = before.shifts[range_index];
        let wrap_offset = usize::try_from(shift.rem_euclid(isize::try_from(len).unwrap())).unwrap();
        let physical_last = len - 1 - wrap_offset;

        *trust = (*trust).min(logical).min(physical_last);
        order.remove(logical);
        order[*trust..].sort_unstable();

        // We sort the same untrusted part of what we got.
        let order = &mut after.ranges[range_index].0;
        if *trust <= order.len() {
            order[*trust..].sort_unstable();
        }
    }
    assert_eq!(after, expected, "{context}");

    let ranges = &sparse_set.groups.ranges;
    assert_eq!(ranges.len(), before.groups.len(), "{context}");
    for (range, group) in ranges.iter().zip(&before.groups) {
        assert!(Arc::ptr_eq(&range.group, group), "{context}");
    }
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
