mod miri;

use super::groups::GroupRange;
use super::*;
use crate::Component;
use alloc::sync::Arc;
use alloc::vec;
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

/// Builds a storage with entities 0, 1 and 2, all placed in a single fully trusted group range.
fn grouped_sparse_set() -> SparseSet<I32> {
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(
                EntityId::new_from_parts(index, 0),
                I32(index as i32),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
    }

    sparse_set.groups.ranges.push(GroupRange {
        group: Arc::from([StorageId::Custom(0), StorageId::Custom(1)]),
        start: 0,
        len: 3,
        shift: 0,
        trust: 3,
    });
    sparse_set.groups.unevaluated_start = 3;

    sparse_set
}

fn assert_reset(groups: &Groups) {
    assert_eq!(groups.ranges.len(), 1);
    assert_eq!(groups.ranges[0].start, 0);
    assert_eq!(groups.ranges[0].len, 0);
    assert_eq!(groups.ranges[0].trust, 0);
    assert_eq!(groups.unevaluated_start, 0);
}

fn assert_untouched(groups: &Groups) {
    assert_eq!(groups.ranges.len(), 1);
    assert_eq!(groups.ranges[0].start, 0);
    assert_eq!(groups.ranges[0].len, 3);
    assert_eq!(groups.ranges[0].trust, 3);
    assert_eq!(groups.unevaluated_start, 3);
}

/// Asserts that removing a component from a grouped storage resets its groups.
#[test]
fn groups_remove() {
    let mut sparse_set = grouped_sparse_set();

    let removed = sparse_set.dyn_remove(EntityId::new_from_parts(1, 0), TrackingTimestamp::new(0));

    assert_eq!(removed, Some(I32(1)));
    assert_reset(&sparse_set.groups);
}

/// Asserts that deleting a component from a grouped storage resets its groups.
#[test]
fn groups_delete() {
    let mut sparse_set = grouped_sparse_set();

    let deleted = sparse_set.dyn_delete(EntityId::new_from_parts(1, 0), TrackingTimestamp::new(0));

    assert!(deleted);
    assert_reset(&sparse_set.groups);
}

/// Asserts that `retain` removing a single component from a grouped storage resets its groups.
#[test]
fn groups_retain() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set.private_retain(TrackingTimestamp::new(0), |_, component| component.0 != 1);

    assert_eq!(sparse_set.len(), 2);
    assert_reset(&sparse_set.groups);
}

/// Asserts that inserting over the slot of an entity with a smaller generation
/// resets the storage's groups, since a different entity now sits in the range.
#[test]
fn groups_other_override() {
    let mut sparse_set = grouped_sparse_set();

    let result = sparse_set.insert(
        EntityId::new_from_parts(1, 1),
        I32(10),
        TrackingTimestamp::new(0),
    );

    assert!(matches!(result, InsertionResult::OtherComponentOverride));
    assert_reset(&sparse_set.groups);
}

/// Asserts that sorting a grouped storage resets its groups.
#[test]
fn groups_sort() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set.sort_unstable();

    assert_reset(&sparse_set.groups);
}

/// Asserts that clearing a grouped storage resets its groups.
#[test]
fn groups_clear() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set.private_clear(TrackingTimestamp::new(0));

    assert!(sparse_set.is_empty());
    assert_reset(&sparse_set.groups);
}

/// Asserts that draining a grouped storage resets its groups.
#[test]
fn groups_drain() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set.private_drain(TrackingTimestamp::new(0));

    assert!(sparse_set.is_empty());
    assert_reset(&sparse_set.groups);
}

/// Asserts that inserting a new entity in a grouped storage leaves its groups untouched,
/// the component lands in the unevaluated region.
#[test]
fn groups_insert_new() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set
        .insert(
            EntityId::new_from_parts(3, 0),
            I32(3),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();

    assert_eq!(sparse_set.len(), 4);
    assert_untouched(&sparse_set.groups);
}

/// Asserts that overwriting an entity's component in a grouped storage leaves its groups untouched.
#[test]
fn groups_insert_same() {
    let mut sparse_set = grouped_sparse_set();

    let result = sparse_set.insert(
        EntityId::new_from_parts(1, 0),
        I32(10),
        TrackingTimestamp::new(0),
    );

    assert!(matches!(result, InsertionResult::ComponentOverride(I32(1))));
    assert_untouched(&sparse_set.groups);
}

/// Asserts that a `SparseSet` exposes its grouping metadata and entities through `Storage`,
/// with `dense` listing the inserted entities in order.
#[test]
fn storage_groups() {
    let mut sparse_set = SparseSet::new();
    sparse_set
        .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(5), I32(5), TrackingTimestamp::new(0))
        .assert_inserted();

    assert!(Storage::groups(&sparse_set).is_some());
    assert!(Storage::groups_mut(&mut sparse_set).is_some());
    assert_eq!(
        Storage::dense(&sparse_set),
        Some(&[EntityId::new(0), EntityId::new(5)][..])
    );
}

/// Asserts that a `NonSend` `SparseSet` exposes its grouping metadata and entities through `Storage`,
/// with `dense` listing the inserted entities in order.
#[cfg(feature = "thread_local")]
#[test]
fn storage_groups_non_send() {
    let mut sparse_set = crate::borrow::NonSend(SparseSet::new());
    sparse_set
        .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(5), I32(5), TrackingTimestamp::new(0))
        .assert_inserted();

    assert!(Storage::groups(&sparse_set).is_some());
    assert!(Storage::groups_mut(&mut sparse_set).is_some());
    assert_eq!(
        Storage::dense(&sparse_set),
        Some(&[EntityId::new(0), EntityId::new(5)][..])
    );
}

/// Asserts that swapping two rows of a tracked storage keeps every entity's component,
/// insertion and modification state, and points the sparse array at the new indices.
#[test]
fn swap_rows() {
    let mut sparse_set = SparseSet::new();
    let e0 = EntityId::new(0);
    let e1 = EntityId::new(1);
    let e2 = EntityId::new(2);
    let last = TrackingTimestamp::new(1);
    let current = TrackingTimestamp::new(2);

    sparse_set
        .insert(e0, TrackedI32(0), current)
        .assert_inserted();
    sparse_set.insert(e1, TrackedI32(1), last).assert_inserted();
    sparse_set.insert(e2, TrackedI32(2), last).assert_inserted();
    let _ = sparse_set.insert(e2, TrackedI32(20), current);

    sparse_set.swap_rows(0, 2);

    assert_eq!(sparse_set.index_of(e0), Some(2));
    assert_eq!(sparse_set.index_of(e1), Some(1));
    assert_eq!(sparse_set.index_of(e2), Some(0));
    assert_eq!(sparse_set.private_get(e0), Some(&TrackedI32(0)));
    assert_eq!(sparse_set.private_get(e2), Some(&TrackedI32(20)));
    assert!(<crate::track::All as Tracking>::is_inserted(
        &sparse_set,
        e0,
        last,
        current
    ));
    assert!(!<crate::track::All as Tracking>::is_inserted(
        &sparse_set,
        e2,
        last,
        current
    ));
    assert!(!<crate::track::All as Tracking>::is_modified(
        &sparse_set,
        e0,
        last,
        current
    ));
    assert!(<crate::track::All as Tracking>::is_modified(
        &sparse_set,
        e2,
        last,
        current
    ));
}

/// Asserts that swapping a row with itself changes nothing.
#[test]
fn swap_rows_same() {
    let mut sparse_set = SparseSet::new();
    sparse_set
        .insert(EntityId::new(0), I32(0), TrackingTimestamp::new(0))
        .assert_inserted();
    sparse_set
        .insert(EntityId::new(1), I32(1), TrackingTimestamp::new(0))
        .assert_inserted();

    sparse_set.swap_rows(1, 1);

    assert_eq!(sparse_set.dense, [EntityId::new(0), EntityId::new(1)]);
    assert_eq!(sparse_set.index_of(EntityId::new(1)), Some(1));
}

/// Builds a storage holding entities `0..n` in order, each with `I32(index)`.
/// The first `range_len` are in a fully trusted range, the next `ungrouped_len` are ungrouped
/// and the last `unevaluated_len` are unevaluated.
fn layout(
    range_len: usize,
    ungrouped_len: usize,
    unevaluated_len: usize,
) -> (SparseSet<I32>, Arc<[StorageId]>) {
    let mut sparse_set = SparseSet::new();
    for index in 0..range_len + ungrouped_len + unevaluated_len {
        sparse_set
            .insert(
                EntityId::new(index as u64),
                I32(i32::try_from(index).unwrap()),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
    }

    let group: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(1)]);
    sparse_set.groups.ranges.push(GroupRange {
        group: group.clone(),
        start: 0,
        len: range_len,
        shift: 0,
        trust: range_len,
    });
    sparse_set.groups.unevaluated_start = range_len + ungrouped_len;

    (sparse_set, group)
}

/// Returns the sorted entity indices of the range, the ungrouped region and the unevaluated region.
fn regions(sparse_set: &SparseSet<I32>) -> [Vec<u64>; 3] {
    let grouped_end = sparse_set.groups.grouped_end();
    let unevaluated_start = sparse_set.groups.unevaluated_start;

    let sorted = |entities: &[EntityId]| {
        let mut indices: Vec<u64> = entities.iter().map(|entity| entity.index()).collect();
        indices.sort_unstable();
        indices
    };

    [
        sorted(&sparse_set.dense[..grouped_end]),
        sorted(&sparse_set.dense[grouped_end..unevaluated_start]),
        sorted(&sparse_set.dense[unevaluated_start..]),
    ]
}

/// Panics if an entity of a storage built by `layout` lost its component.
fn assert_reachable(sparse_set: &SparseSet<I32>) {
    for index in 0..sparse_set.len() {
        assert_eq!(
            sparse_set.private_get(EntityId::new(index as u64)),
            Some(&I32(i32::try_from(index).unwrap()))
        );
    }
}

/// Asserts that moving an unevaluated component to the range appends it,
/// keeps the range fully trusted and pushes an ungrouped component into the grown ungrouped region.
#[test]
fn move_unevaluated_to_range() {
    let (mut sparse_set, group) = layout(2, 2, 3);

    sparse_set.private_move_to(EntityId::new(5), GroupRegion::Range(&group));

    assert_eq!(sparse_set.groups.ranges[0].len, 3);
    assert_eq!(sparse_set.groups.ranges[0].trust, 3);
    assert_eq!(sparse_set.groups.unevaluated_start, 5);
    assert_eq!(sparse_set.dense[2], EntityId::new(5));
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 1, 5], vec![2, 3], vec![4, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that moving an unevaluated component to the range works without any ungrouped component.
#[test]
fn move_unevaluated_to_range_no_ungrouped() {
    let (mut sparse_set, group) = layout(2, 0, 3);

    sparse_set.private_move_to(EntityId::new(3), GroupRegion::Range(&group));

    assert_eq!(sparse_set.groups.ranges[0].len, 3);
    assert_eq!(sparse_set.groups.ranges[0].trust, 3);
    assert_eq!(sparse_set.groups.unevaluated_start, 3);
    assert_eq!(regions(&sparse_set), [vec![0, 1, 3], vec![], vec![2, 4]]);
    assert_reachable(&sparse_set);
}

/// Asserts that moving an ungrouped component to the range appends it and keeps the range fully trusted.
#[test]
fn move_ungrouped_to_range() {
    let (mut sparse_set, group) = layout(2, 2, 3);

    sparse_set.private_move_to(EntityId::new(3), GroupRegion::Range(&group));

    assert_eq!(sparse_set.groups.ranges[0].len, 3);
    assert_eq!(sparse_set.groups.ranges[0].trust, 3);
    assert_eq!(sparse_set.groups.unevaluated_start, 4);
    assert_eq!(sparse_set.dense[2], EntityId::new(3));
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 1, 3], vec![2], vec![4, 5, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that appending to a damaged range leaves its trust unchanged.
#[test]
fn move_to_damaged_range() {
    let (mut sparse_set, group) = layout(3, 2, 2);
    sparse_set.groups.ranges[0].trust = 1;

    sparse_set.private_move_to(EntityId::new(3), GroupRegion::Range(&group));

    assert_eq!(sparse_set.groups.ranges[0].len, 4);
    assert_eq!(sparse_set.groups.ranges[0].trust, 1);
}

/// Asserts that moving an unevaluated component to the ungrouped region grows it by one.
#[test]
fn move_unevaluated_to_ungrouped() {
    let (mut sparse_set, _) = layout(2, 2, 3);

    sparse_set.private_move_to(EntityId::new(5), GroupRegion::Ungrouped);

    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.unevaluated_start, 5);
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 1], vec![2, 3, 5], vec![4, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that moving a component from the middle of the range to the unevaluated region
/// shrinks the range and drops its trust to the component's old position.
#[test]
fn move_range_to_unevaluated() {
    let (mut sparse_set, _) = layout(3, 2, 2);

    sparse_set.private_move_to(EntityId::new(1), GroupRegion::Unevaluated);

    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.ranges[0].trust, 1);
    assert_eq!(sparse_set.groups.unevaluated_start, 4);
    assert_eq!(sparse_set.dense[..2], [EntityId::new(0), EntityId::new(2)]);
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 2], vec![3, 4], vec![1, 5, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that moving a component from the range to the unevaluated region works without any ungrouped component.
#[test]
fn move_range_to_unevaluated_no_ungrouped() {
    let (mut sparse_set, _) = layout(3, 0, 2);

    sparse_set.private_move_to(EntityId::new(0), GroupRegion::Unevaluated);

    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.ranges[0].trust, 0);
    assert_eq!(sparse_set.groups.unevaluated_start, 2);
    assert_eq!(regions(&sparse_set), [vec![1, 2], vec![], vec![0, 3, 4]]);
    assert_reachable(&sparse_set);
}

/// Asserts that evicting the range's last component keeps the rest of the range fully trusted.
#[test]
fn move_range_last_to_unevaluated() {
    let (mut sparse_set, _) = layout(3, 2, 2);

    sparse_set.private_move_to(EntityId::new(2), GroupRegion::Unevaluated);

    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.ranges[0].trust, 2);
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 1], vec![3, 4], vec![2, 5, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that moving a component from the range to the ungrouped region shrinks the range
/// and drops its trust to the component's old position.
#[test]
fn move_range_to_ungrouped() {
    let (mut sparse_set, _) = layout(3, 2, 2);

    sparse_set.private_move_to(EntityId::new(1), GroupRegion::Ungrouped);

    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.ranges[0].trust, 1);
    assert_eq!(sparse_set.groups.unevaluated_start, 5);
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 2], vec![1, 3, 4], vec![5, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that moving an ungrouped component to the unevaluated region shrinks the ungrouped region by one.
#[test]
fn move_ungrouped_to_unevaluated() {
    let (mut sparse_set, _) = layout(2, 2, 3);

    sparse_set.private_move_to(EntityId::new(2), GroupRegion::Unevaluated);

    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.unevaluated_start, 3);
    assert_eq!(
        regions(&sparse_set),
        [vec![0, 1], vec![3], vec![2, 4, 5, 6]]
    );
    assert_reachable(&sparse_set);
}

/// Asserts that moving components to the region they're already in doesn't move anything.
#[test]
fn move_in_place() {
    let (mut sparse_set, group) = layout(2, 2, 3);
    let dense = sparse_set.dense.clone();

    sparse_set.private_move_to(EntityId::new(0), GroupRegion::Range(&group));
    sparse_set.private_move_to(EntityId::new(2), GroupRegion::Ungrouped);
    sparse_set.private_move_to(EntityId::new(4), GroupRegion::Unevaluated);

    assert_eq!(sparse_set.dense, dense);
    assert_eq!(sparse_set.groups.ranges[0].len, 2);
    assert_eq!(sparse_set.groups.ranges[0].trust, 2);
    assert_eq!(sparse_set.groups.unevaluated_start, 4);
}

/// Asserts that moving to a group without a range in the storage panics,
/// even when an equal group exists, since groups are compared by pointer.
#[test]
#[should_panic(expected = "the destination group has no range in this storage")]
fn move_to_other_group() {
    let (mut sparse_set, _) = layout(2, 2, 3);
    let other_group: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(1)]);

    sparse_set.private_move_to(EntityId::new(4), GroupRegion::Range(&other_group));
}

/// Asserts that moving an entity without a component in the storage panics.
#[test]
#[should_panic(expected = "only components present in the storage can be moved")]
fn move_missing() {
    let (mut sparse_set, _) = layout(2, 2, 3);

    sparse_set.private_move_to(EntityId::new(10), GroupRegion::Ungrouped);
}

/// Asserts that `Storage::move_to` panics on storages that don't support grouping.
#[test]
#[should_panic(expected = "grouping only supports SparseSet storages")]
fn move_unsupported() {
    let mut entities = crate::entities::Entities::new();

    Storage::move_to(&mut entities, EntityId::new(0), GroupRegion::Ungrouped);
}

/// Asserts that `Storage::move_to` on a `NonSend` `SparseSet` moves the component.
#[cfg(feature = "thread_local")]
#[test]
fn move_non_send() {
    let (sparse_set, _) = layout(2, 2, 3);
    let mut sparse_set = crate::borrow::NonSend(sparse_set);

    Storage::move_to(&mut sparse_set, EntityId::new(5), GroupRegion::Ungrouped);

    assert_eq!(
        regions(&sparse_set),
        [vec![0, 1], vec![2, 3, 5], vec![4, 6]]
    );
}

mod range_moves {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

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

    /// Range layout shared by the tests: `(len, shift)` pairs, rotated rings and an empty one.
    ///
    /// The first range isn't rotated, so its physical last component is also its logical last.
    const RANGES: [(usize, isize); 5] = [(3, 0), (2, 1), (0, 0), (4, -3), (1, 5)];

    /// Builds a storage holding entities `0..12` in physical order, each with `make(index)`.
    ///
    /// `RANGES` are placed back to back and fully trusted, the last 2 components are ungrouped.\
    /// Tracked entities are inserted at timestamp `index` and modified at `100 + index`.
    fn storage<T: Component>(make: impl Fn(u64) -> T) -> SparseSet<T> {
        storage_with(&RANGES, make)
    }

    /// Same as `storage`, with `ranges` as `(len, shift)` pairs instead of `RANGES`.
    fn storage_with<T: Component>(
        ranges: &[(usize, isize)],
        make: impl Fn(u64) -> T,
    ) -> SparseSet<T> {
        storage_with_tails(ranges, 2, 0, make)
    }

    /// Same as `storage_with`, followed by `ungrouped_len` ungrouped
    /// and `unevaluated_len` unevaluated components.
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

    fn ints() -> SparseSet<I32> {
        storage(|index| I32(i32::try_from(index).unwrap()))
    }

    /// Returns the entity indices of each range, in logical order.
    fn logical_order<T: Component>(sparse_set: &SparseSet<T>) -> Vec<Vec<u64>> {
        sparse_set
            .groups
            .ranges
            .iter()
            .map(|range| {
                let ring = range.ring();

                (0..ring.len)
                    .map(|logical| sparse_set.dense[ring.physical_of(logical)].index())
                    .collect()
            })
            .collect()
    }

    fn starts<T: Component>(sparse_set: &SparseSet<T>) -> Vec<usize> {
        sparse_set
            .groups
            .ranges
            .iter()
            .map(|range| range.start)
            .collect()
    }

    fn trusts<T: Component>(sparse_set: &SparseSet<T>) -> Vec<usize> {
        sparse_set
            .groups
            .ranges
            .iter()
            .map(|range| range.trust)
            .collect()
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

    /// Takes entity 2, the first range's last component, out of it and moves the later ranges left.
    fn shrink_first<T: Component>(sparse_set: &mut SparseSet<T>) {
        sparse_set.groups.ranges[0].len = 2;
        sparse_set.groups.ranges[0].trust = 2;

        sparse_set.move_ranges_left(1);
    }

    /// Moves the later ranges right over entity 10, the first ungrouped component,
    /// and appends it to the first range.
    fn grow_first<T: Component>(sparse_set: &mut SparseSet<T>) {
        sparse_set.move_ranges_right(1);

        sparse_set.groups.ranges[0].len = 4;
        sparse_set.groups.ranges[0].trust = 4;
    }

    fn int_value(component: &I32) -> Option<u64> {
        Some(u64::try_from(component.0).unwrap())
    }

    /// Asserts that moving ranges left keeps each later range's logical order,
    /// rotated or empty, and moves each of them one slot down.
    #[test]
    fn left() {
        let mut sparse_set = ints();
        let before = logical_order(&sparse_set);

        shrink_first(&mut sparse_set);

        assert_eq!(logical_order(&sparse_set)[1..], before[1..]);
        assert_eq!(starts(&sparse_set), [0, 2, 4, 4, 8]);
        assert_rows(&sparse_set, int_value);
    }

    /// Asserts that moving ranges left carries the component before them past all of them,
    /// leaving it as the first ungrouped component.
    #[test]
    fn left_carried() {
        let mut sparse_set = ints();

        shrink_first(&mut sparse_set);

        let grouped_end = sparse_set.groups.grouped_end();
        assert_eq!(grouped_end, 9);
        assert_eq!(sparse_set.dense[grouped_end], EntityId::new(2));
    }

    /// Asserts that moving ranges left leaves their trust alone, partial or full.
    #[test]
    fn left_trust() {
        let mut sparse_set = ints();
        sparse_set.groups.ranges[1].trust = 1;
        sparse_set.groups.ranges[3].trust = 0;

        shrink_first(&mut sparse_set);

        assert_eq!(trusts(&sparse_set), [2, 1, 0, 0, 1]);
    }

    /// Asserts that moving ranges right keeps each later range's logical order,
    /// rotated or empty, and moves each of them one slot up.
    #[test]
    fn right() {
        let mut sparse_set = ints();
        let before = logical_order(&sparse_set);

        grow_first(&mut sparse_set);

        assert_eq!(logical_order(&sparse_set)[1..], before[1..]);
        assert_eq!(starts(&sparse_set), [0, 4, 6, 6, 10]);
        assert_rows(&sparse_set, int_value);
    }

    /// Asserts that moving ranges right carries the component after them past all of them,
    /// leaving it right before the first moved range.
    #[test]
    fn right_carried() {
        let mut sparse_set = ints();

        grow_first(&mut sparse_set);

        assert_eq!(sparse_set.dense[3], EntityId::new(10));
    }

    /// Asserts that moving ranges right leaves their trust alone, partial or full.
    #[test]
    fn right_trust() {
        let mut sparse_set = ints();
        sparse_set.groups.ranges[1].trust = 1;
        sparse_set.groups.ranges[3].trust = 0;

        grow_first(&mut sparse_set);

        assert_eq!(trusts(&sparse_set), [4, 1, 0, 0, 1]);
    }

    /// Asserts that moving only empty ranges moves their starts and nothing else.
    #[test]
    fn empty_ranges() {
        let mut sparse_set = ints();
        sparse_set.groups.ranges.truncate(3);
        sparse_set.groups.ranges[1].len = 0;
        sparse_set.groups.ranges[1].trust = 0;
        sparse_set.groups.ranges[2].start = 3;
        let dense = sparse_set.dense.clone();

        sparse_set.move_ranges_right(1);
        assert_eq!(starts(&sparse_set), [0, 4, 4]);
        sparse_set.move_ranges_left(1);
        assert_eq!(starts(&sparse_set), [0, 3, 3]);

        assert_eq!(sparse_set.dense, dense);
    }

    /// Asserts that insertion and modification timestamps follow their components
    /// in both directions.
    #[test]
    fn tracked() {
        let mut sparse_set = storage(|index| TrackedI32(i32::try_from(index).unwrap()));
        let value = |component: &TrackedI32| Some(u64::try_from(component.0).unwrap());

        shrink_first(&mut sparse_set);
        assert_rows(&sparse_set, value);

        // Entity 2 left the range, we put it back.
        sparse_set.move_ranges_right(1);
        sparse_set.groups.ranges[0].len = 3;
        sparse_set.groups.ranges[0].trust = 3;
        assert_rows(&sparse_set, value);
    }

    /// Asserts that moving ranges in both directions never drops a component,
    /// and that dropping the storage then drops each of them once.
    #[test]
    fn drops() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut sparse_set = storage(|index| Counted(index, drops.clone()));

        shrink_first(&mut sparse_set);
        sparse_set.move_ranges_right(1);
        sparse_set.groups.ranges[0].len = 3;
        assert_rows(&sparse_set, |component| Some(component.0));

        assert_eq!(drops.load(AtomicOrdering::Relaxed), 0);
        drop(sparse_set);
        assert_eq!(drops.load(AtomicOrdering::Relaxed), 12);
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

    fn ints_with(ranges: &[(usize, isize)]) -> SparseSet<I32> {
        storage_with(ranges, |index| I32(i32::try_from(index).unwrap()))
    }

    /// Moves the component at `logical` of range `range_index` to `destination`, then panics
    /// unless that range lost it with a logical swap-remove and only trusts the positions
    /// before it, while every other range kept its order and trust.
    fn assert_take<T: Component>(
        sparse_set: &mut SparseSet<T>,
        range_index: usize,
        logical: usize,
        destination: GroupRegion<'_>,
        value: impl Fn(&T) -> Option<u64>,
    ) {
        let ring = sparse_set.groups.ranges[range_index].ring();
        let mut order = logical_order(sparse_set);
        let mut trust = trusts(sparse_set);
        let entity = EntityId::new(order[range_index][logical]);

        sparse_set.private_move_to(entity, destination);

        // `Vec::swap_remove` is exactly a logical swap-remove.
        order[range_index].swap_remove(logical);
        trust[range_index] = trust[range_index].min(logical);
        assert_eq!(
            logical_order(sparse_set),
            order,
            "{ring:?}, logical {logical}"
        );
        assert_eq!(trusts(sparse_set), trust, "{ring:?}, logical {logical}");

        let index = sparse_set.index_of(entity).unwrap();
        assert!(
            matches!(
                (sparse_set.groups.region_of(index), destination),
                (GroupRegion::Ungrouped, GroupRegion::Ungrouped)
                    | (GroupRegion::Unevaluated, GroupRegion::Unevaluated)
            ),
            "{ring:?}, logical {logical}"
        );
        assert_rows(sparse_set, value);
    }

    /// Asserts that every position of a small first ring, at any rotation, leaves with a
    /// logical swap-remove while the later rings, rotated or empty, keep their order.
    #[test]
    fn take_first() {
        for len in 1..=5 {
            for shift in shifts() {
                for logical in 0..len {
                    let mut sparse_set = ints_with(&[(len, shift), (2, 1), (0, 0), (3, -2)]);

                    assert_take(
                        &mut sparse_set,
                        0,
                        logical,
                        GroupRegion::Ungrouped,
                        int_value,
                    );
                }
            }
        }
    }

    /// Asserts that every position of a ring between other rings leaves with a logical
    /// swap-remove, the earlier ring staying untouched.
    #[test]
    fn take_middle() {
        for len in 1..=5 {
            for shift in shifts() {
                for logical in 0..len {
                    let mut sparse_set = ints_with(&[(2, 1), (len, shift), (0, 0), (3, -2)]);

                    assert_take(
                        &mut sparse_set,
                        1,
                        logical,
                        GroupRegion::Unevaluated,
                        int_value,
                    );
                }
            }
        }
    }

    /// Asserts that every position of the last ring leaves with a logical swap-remove.
    #[test]
    fn take_last() {
        for len in 1..=5 {
            for shift in shifts() {
                for logical in 0..len {
                    let mut sparse_set = ints_with(&[(2, 1), (0, 0), (len, shift)]);

                    assert_take(
                        &mut sparse_set,
                        2,
                        logical,
                        GroupRegion::Ungrouped,
                        int_value,
                    );
                }
            }
        }
    }

    /// Asserts that emptying a rotated singleton ring leaves it empty and attached.
    #[test]
    fn take_singleton() {
        let mut sparse_set = ints_with(&[(1, -4), (2, 1)]);

        assert_take(&mut sparse_set, 0, 0, GroupRegion::Ungrouped, int_value);

        assert_eq!(sparse_set.groups.ranges.len(), 2);
        assert_eq!(sparse_set.groups.ranges[0].len, 0);
    }

    /// Asserts that taking a component after a damaged prefix keeps that prefix's trust,
    /// and taking one inside it lowers the trust to its position.
    #[test]
    fn take_partial_trust() {
        let mut sparse_set = ints_with(&[(5, 2), (2, 1)]);
        sparse_set.groups.ranges[0].trust = 2;

        assert_take(&mut sparse_set, 0, 4, GroupRegion::Ungrouped, int_value);
        assert_eq!(sparse_set.groups.ranges[0].trust, 2);

        assert_take(&mut sparse_set, 0, 1, GroupRegion::Ungrouped, int_value);
        assert_eq!(sparse_set.groups.ranges[0].trust, 1);
    }

    /// Asserts that insertion and modification timestamps follow their components
    /// while rotated rings lose components.
    #[test]
    fn take_tracked() {
        let mut sparse_set = storage_with(&[(4, 3), (3, -1)], |index| {
            TrackedI32(i32::try_from(index).unwrap())
        });
        let value = |component: &TrackedI32| Some(u64::try_from(component.0).unwrap());

        for (range_index, logical) in [(0, 1), (1, 0), (0, 2), (1, 1)] {
            assert_take(
                &mut sparse_set,
                range_index,
                logical,
                GroupRegion::Ungrouped,
                value,
            );
        }
    }

    /// Asserts that emptying rotated rings never drops a component,
    /// and that dropping the storage then drops each of them once.
    #[test]
    fn take_drops() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut sparse_set =
            storage_with(&[(4, 3), (3, -1)], |index| Counted(index, drops.clone()));

        for (range_index, logical) in [(0, 1), (1, 2), (0, 2), (1, 0), (0, 0), (1, 0), (0, 0)] {
            assert_take(
                &mut sparse_set,
                range_index,
                logical,
                GroupRegion::Unevaluated,
                |component| Some(component.0),
            );
        }

        assert_eq!(drops.load(AtomicOrdering::Relaxed), 0);
        drop(sparse_set);
        assert_eq!(drops.load(AtomicOrdering::Relaxed), 9);
    }

    struct Zst;

    impl Component for Zst {
        type Tracking = crate::track::Untracked;
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

    /// Layout of a storage by entity index: each range in logical order,
    /// then the sorted ungrouped and unevaluated regions.
    #[derive(Clone, PartialEq, Debug)]
    struct Snapshot {
        ranges: Vec<Vec<u64>>,
        ungrouped: Vec<u64>,
        unevaluated: Vec<u64>,
    }

    fn snapshot<T: Component>(sparse_set: &SparseSet<T>) -> Snapshot {
        let groups = &sparse_set.groups;
        let sorted = |entities: &[EntityId]| {
            let mut indices: Vec<u64> = entities.iter().map(|entity| entity.index()).collect();
            indices.sort_unstable();
            indices
        };

        Snapshot {
            ranges: logical_order(sparse_set),
            ungrouped: sorted(&sparse_set.dense[groups.grouped_end()..groups.unevaluated_start]),
            unevaluated: sorted(&sparse_set.dense[groups.unevaluated_start..]),
        }
    }

    /// Where a test moves a component.
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum To {
        Range(usize),
        Ungrouped,
        Unevaluated,
    }

    /// Returns where `entity` is in `snapshot`, with its logical position when it's in a range.
    fn find(snapshot: &Snapshot, entity: u64) -> (To, usize) {
        for (range_index, range) in snapshot.ranges.iter().enumerate() {
            if let Some(logical) = range.iter().position(|&index| index == entity) {
                return (To::Range(range_index), logical);
            }
        }

        if snapshot.ungrouped.contains(&entity) {
            (To::Ungrouped, 0)
        } else {
            (To::Unevaluated, 0)
        }
    }

    /// Returns `before` once `entity` moved to `to`: out of a range with a logical swap-remove,
    /// into a range at its logical end.
    fn expected(before: &Snapshot, entity: u64, to: To) -> Snapshot {
        let (from, logical) = find(before, entity);
        let mut after = before.clone();
        if from == to {
            return after;
        }

        // `Vec::swap_remove` is exactly a logical swap-remove.
        match from {
            To::Range(range_index) => {
                after.ranges[range_index].swap_remove(logical);
            }
            To::Ungrouped => after.ungrouped.retain(|&index| index != entity),
            To::Unevaluated => after.unevaluated.retain(|&index| index != entity),
        }

        match to {
            To::Range(range_index) => after.ranges[range_index].push(entity),
            To::Ungrouped => {
                after.ungrouped.push(entity);
                after.ungrouped.sort_unstable();
            }
            To::Unevaluated => {
                after.unevaluated.push(entity);
                after.unevaluated.sort_unstable();
            }
        }

        after
    }

    /// Returns each range's trust once `entity` moved to `to`.
    ///
    /// The source range only trusts the positions before `entity`'s, a fully trusted
    /// destination stays fully trusted and every other range keeps its trust.
    fn expected_trust<T: Component>(sparse_set: &SparseSet<T>, entity: u64, to: To) -> Vec<usize> {
        let before = snapshot(sparse_set);
        let (from, logical) = find(&before, entity);
        let mut trust = trusts(sparse_set);
        if from == to {
            return trust;
        }

        if let To::Range(range_index) = from {
            trust[range_index] = trust[range_index].min(logical);
        }
        if let To::Range(range_index) = to {
            if trust[range_index] == before.ranges[range_index].len() {
                trust[range_index] += 1;
            }
        }

        trust
    }

    /// Moves `entity` to `to`, then panics unless the layout, trust and rows are the expected ones.
    fn assert_move<T: Component>(
        sparse_set: &mut SparseSet<T>,
        entity: u64,
        to: To,
        value: impl Fn(&T) -> Option<u64>,
    ) {
        let before = snapshot(sparse_set);
        let trust = expected_trust(sparse_set, entity, to);
        let groups: Vec<Arc<[StorageId]>> = sparse_set
            .groups
            .ranges
            .iter()
            .map(|range| range.group.clone())
            .collect();

        let destination = match to {
            To::Range(range_index) => GroupRegion::Range(&groups[range_index]),
            To::Ungrouped => GroupRegion::Ungrouped,
            To::Unevaluated => GroupRegion::Unevaluated,
        };
        sparse_set.private_move_to(EntityId::new(entity), destination);

        let context = alloc::format!("{before:?}, entity {entity} to {to:?}");
        assert_eq!(
            snapshot(sparse_set),
            expected(&before, entity, to),
            "{context}"
        );
        assert_eq!(trusts(sparse_set), trust, "{context}");
        assert_rows(sparse_set, value);
    }

    /// Asserts that appending to a ring of any small length and rotation puts the component
    /// at its logical end, from the ungrouped and from the unevaluated region,
    /// while the later rings keep their order.
    #[test]
    fn append_first() {
        for len in 0..=5 {
            for shift in shifts() {
                let mut sparse_set =
                    ints_with_tails(&[(len, shift), (2, 1), (0, 0), (3, -2)], 2, 2);
                let ungrouped = len as u64 + 5;
                let unevaluated = len as u64 + 7;

                assert_move(&mut sparse_set, ungrouped, To::Range(0), int_value);
                assert_move(&mut sparse_set, unevaluated, To::Range(0), int_value);
            }
        }
    }

    /// Asserts that appending to the last ring of any small length and rotation puts the
    /// component at its logical end, the earlier rings staying untouched.
    #[test]
    fn append_last() {
        for len in 0..=5 {
            for shift in shifts() {
                let mut sparse_set = ints_with_tails(&[(2, 1), (0, 0), (len, shift)], 1, 1);
                let ungrouped = len as u64 + 2;
                let unevaluated = len as u64 + 3;

                assert_move(&mut sparse_set, ungrouped, To::Range(2), int_value);
                assert_move(&mut sparse_set, unevaluated, To::Range(2), int_value);
            }
        }
    }

    /// Asserts that appending to an empty ring between non-empty ones gives it its first component.
    #[test]
    fn append_empty() {
        let mut sparse_set = ints_with_tails(&[(3, 1), (0, 0), (4, -2)], 1, 1);

        assert_move(&mut sparse_set, 7, To::Range(1), int_value);
        assert_eq!(snapshot(&sparse_set).ranges[1], [7]);
    }

    /// Asserts that appending to a damaged ring keeps its trust, it doesn't become fully trusted.
    #[test]
    fn append_damaged() {
        let mut sparse_set = ints_with_tails(&[(4, 2), (2, 1)], 1, 0);
        sparse_set.groups.ranges[0].trust = 1;

        assert_move(&mut sparse_set, 6, To::Range(0), int_value);
        assert_eq!(sparse_set.groups.ranges[0].trust, 1);
    }

    /// Asserts that every component can move to every region of a storage holding several
    /// rotated rings and an empty one: between ranges in both directions, into an empty range,
    /// in and out of both tail regions, and in place.
    #[test]
    fn move_matrix() {
        let destinations = [
            To::Range(0),
            To::Range(1),
            To::Range(2),
            To::Range(3),
            To::Ungrouped,
            To::Unevaluated,
        ];

        // Under Miri, every other component still leaves from every region.
        let step = if cfg!(miri) { 2 } else { 1 };
        for entity in (0..13).step_by(step) {
            for to in destinations {
                let mut sparse_set = ints_with_tails(&[(3, 1), (0, 0), (4, -2), (2, 5)], 2, 2);

                assert_move(&mut sparse_set, entity, to, int_value);
            }
        }
    }

    /// Asserts that moving a component to the region it's already in changes nothing.
    #[test]
    fn move_in_place() {
        let mut sparse_set = ints_with_tails(&[(3, 1), (4, -2)], 2, 2);
        let dense = sparse_set.dense.clone();

        assert_move(&mut sparse_set, 0, To::Range(0), int_value);
        assert_move(&mut sparse_set, 5, To::Range(1), int_value);
        assert_move(&mut sparse_set, 7, To::Ungrouped, int_value);
        assert_move(&mut sparse_set, 9, To::Unevaluated, int_value);

        assert_eq!(sparse_set.dense, dense);
    }

    /// Asserts that moves work without any ungrouped or unevaluated component.
    #[test]
    fn move_empty_tails() {
        let mut sparse_set = ints_with_tails(&[(3, 1), (2, -1)], 0, 0);

        assert_move(&mut sparse_set, 0, To::Range(1), int_value);
        assert_move(&mut sparse_set, 4, To::Range(0), int_value);
        assert_move(&mut sparse_set, 2, To::Unevaluated, int_value);
    }

    /// Asserts that a reset storage refills its ranges, whatever shifts they kept.
    #[test]
    fn move_after_reset() {
        let mut sparse_set = ints_with_tails(&[(3, 2), (2, -1)], 1, 1);
        sparse_set.groups.reset();

        for (entity, range_index) in [(0, 1), (4, 0), (6, 1), (2, 0), (1, 0)] {
            assert_move(&mut sparse_set, entity, To::Range(range_index), int_value);
        }
    }

    /// Asserts that a series of moves through rotated rings keeps applying logical
    /// swap-removes and appends, every range keeping its order otherwise.
    #[test]
    fn move_series() {
        let mut sparse_set = ints_with_tails(&[(4, 3), (0, 0), (3, -1), (5, 2)], 2, 3);
        let moves = [
            (5, To::Range(1)),
            (13, To::Range(0)),
            (0, To::Range(3)),
            (9, To::Range(1)),
            (2, To::Ungrouped),
            (12, To::Range(0)),
            (6, To::Unevaluated),
            (14, To::Range(2)),
            (11, To::Range(0)),
            (7, To::Range(3)),
        ];

        for (entity, to) in moves {
            assert_move(&mut sparse_set, entity, to, int_value);
        }
    }

    /// Asserts that insertion and modification timestamps follow their components
    /// between rings and tail regions.
    #[test]
    fn move_tracked() {
        let mut sparse_set = storage_with_tails(&[(3, 1), (0, 0), (4, -2)], 2, 2, |index| {
            TrackedI32(i32::try_from(index).unwrap())
        });
        let value = |component: &TrackedI32| Some(u64::try_from(component.0).unwrap());
        let moves = [
            (1, To::Range(2)),
            (5, To::Range(0)),
            (8, To::Range(1)),
            (3, To::Ungrouped),
            (10, To::Range(2)),
        ];

        for (entity, to) in moves {
            assert_move(&mut sparse_set, entity, to, value);
        }
    }

    /// Asserts that zero-sized components move between rings like any other.
    #[test]
    fn move_zst() {
        let mut sparse_set = storage_with_tails(&[(3, 1), (0, 0), (4, -2)], 2, 2, |_| Zst);
        let moves = [
            (1, To::Range(2)),
            (5, To::Range(0)),
            (8, To::Range(1)),
            (3, To::Unevaluated),
        ];

        for (entity, to) in moves {
            assert_move(&mut sparse_set, entity, to, |_| None);
        }
    }

    /// Asserts that moving every component into a ring never drops one,
    /// and that dropping the storage then drops each of them once.
    #[test]
    fn move_drops() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut sparse_set =
            storage_with_tails(&[(3, 1), (0, 0), (4, -2), (2, 3)], 2, 2, |index| {
                Counted(index, drops.clone())
            });

        for entity in 0..13 {
            let to = To::Range(entity as usize % 4);

            assert_move(&mut sparse_set, entity, to, |component| Some(component.0));
        }

        assert_eq!(drops.load(AtomicOrdering::Relaxed), 0);
        drop(sparse_set);
        assert_eq!(drops.load(AtomicOrdering::Relaxed), 13);
    }
}
