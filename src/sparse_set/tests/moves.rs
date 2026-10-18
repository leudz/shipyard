use super::*;

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
    let groups = &sparse_set.groups;
    let len = sparse_set.len();

    let sorted = |entities: &[EntityId]| {
        let mut indices: Vec<u64> = entities.iter().map(|entity| entity.index()).collect();
        indices.sort_unstable();
        indices
    };

    [
        sorted(&sparse_set.dense[..groups.grouped_end()]),
        sorted(&sparse_set.dense[groups.ungrouped_range(len)]),
        sorted(&sparse_set.dense[groups.unevaluated_range(len)]),
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

/// Builds a storage without any range, holding entities `0..len` in order, each with `I32(index)`.
fn group_free(len: usize) -> SparseSet<I32> {
    let mut sparse_set = SparseSet::new();
    for index in 0..len {
        sparse_set
            .insert(
                EntityId::new(index as u64),
                I32(i32::try_from(index).unwrap()),
                TrackingTimestamp::new(0),
            )
            .assert_inserted();
    }

    sparse_set
}

/// Asserts that moving a component of a storage without ranges to the ungrouped region
/// doesn't move anything, every component is already implicitly ungrouped.
#[test]
fn move_group_free_ungrouped() {
    let mut sparse_set = group_free(3);
    let dense = sparse_set.dense.clone();

    sparse_set.private_move_to(EntityId::new(1), GroupRegion::Ungrouped);

    assert_eq!(sparse_set.dense, dense);
    assert!(!sparse_set.groups.has_ranges());
    assert_eq!(sparse_set.groups.unevaluated_start, 0);
    assert_eq!(regions(&sparse_set), [vec![], vec![0, 1, 2], vec![]]);
    assert_reachable(&sparse_set);
}

/// Asserts that moving a component of a storage without ranges to the unevaluated region panics,
/// such a storage has no unevaluated region.
#[test]
#[should_panic(expected = "a storage without group ranges has no unevaluated region")]
fn move_group_free_unevaluated() {
    let mut sparse_set = group_free(3);

    sparse_set.private_move_to(EntityId::new(1), GroupRegion::Unevaluated);
}

/// Asserts that the panic of a move to the unevaluated region of a storage without ranges
/// happens before anything moves: entities, components and metadata are unchanged.
#[cfg(feature = "std")]
#[test]
fn move_group_free_unevaluated_untouched() {
    let mut sparse_set = group_free(3);
    let dense = sparse_set.dense.clone();

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        sparse_set.private_move_to(EntityId::new(1), GroupRegion::Unevaluated);
    }));

    assert!(result.is_err());
    assert_eq!(sparse_set.dense, dense);
    assert!(!sparse_set.groups.has_ranges());
    assert_eq!(sparse_set.groups.unevaluated_start, 0);
    assert_reachable(&sparse_set);
}

/// Asserts that moving an entity without a component in a storage without ranges panics.
#[test]
#[should_panic(expected = "only components present in the storage can be moved")]
fn move_group_free_missing() {
    let mut sparse_set = group_free(3);

    sparse_set.private_move_to(EntityId::new(10), GroupRegion::Ungrouped);
}

/// Asserts that moving to a group in a storage without ranges panics, the group can't be attached.
#[test]
#[should_panic(expected = "the destination group has no range in this storage")]
fn move_group_free_unattached() {
    let mut sparse_set = group_free(3);
    let group: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(1)]);

    sparse_set.private_move_to(EntityId::new(1), GroupRegion::Range(&group));
}

/// Asserts that `Storage::move_to` on a `NonSend` `SparseSet` without ranges
/// doesn't move anything when moving to the ungrouped region.
#[cfg(feature = "thread_local")]
#[test]
fn move_group_free_non_send() {
    let mut sparse_set = crate::borrow::NonSend(group_free(3));
    let dense = sparse_set.dense.clone();

    Storage::move_to(&mut sparse_set, EntityId::new(1), GroupRegion::Ungrouped);

    assert_eq!(sparse_set.dense, dense);
    assert_eq!(regions(&sparse_set), [vec![], vec![0, 1, 2], vec![]]);
}

/// Asserts that `Storage::move_to` on a `NonSend` `SparseSet` without ranges
/// panics when moving to the unevaluated region.
#[cfg(feature = "thread_local")]
#[test]
#[should_panic(expected = "a storage without group ranges has no unevaluated region")]
fn move_group_free_non_send_unevaluated() {
    let mut sparse_set = crate::borrow::NonSend(group_free(3));

    Storage::move_to(&mut sparse_set, EntityId::new(1), GroupRegion::Unevaluated);
}
