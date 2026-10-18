use super::*;

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

/// Panics unless entity 1 left the range with a physical swap-remove: the range keeps entities 0
/// and 2 and only trusts entity 0, nothing is ungrouped or unevaluated.
fn assert_shrunk(sparse_set: &SparseSet<I32>) {
    let groups = &sparse_set.groups;
    assert_eq!(groups.ranges.len(), 1);
    assert_eq!(groups.ranges[0].start, 0);
    assert_eq!(groups.ranges[0].len, 2);
    assert_eq!(groups.ranges[0].trust, 1);
    assert_eq!(groups.unevaluated_start, 2);
    assert_eq!(
        sparse_set.dense,
        [
            EntityId::new_from_parts(0, 0),
            EntityId::new_from_parts(2, 0)
        ]
    );
}

fn assert_untouched(groups: &Groups) {
    assert_eq!(groups.ranges.len(), 1);
    assert_eq!(groups.ranges[0].start, 0);
    assert_eq!(groups.ranges[0].len, 3);
    assert_eq!(groups.ranges[0].trust, 3);
    assert_eq!(groups.unevaluated_start, 3);
}

/// Asserts that removing a component from a grouped storage keeps its range,
/// without the component and only trusting the positions before it.
#[test]
fn groups_remove() {
    let mut sparse_set = grouped_sparse_set();

    let removed = sparse_set.dyn_remove(EntityId::new_from_parts(1, 0), TrackingTimestamp::new(0));

    assert_eq!(removed, Some(I32(1)));
    assert_shrunk(&sparse_set);
}

/// Asserts that deleting a component from a grouped storage keeps its range,
/// without the component and only trusting the positions before it.
#[test]
fn groups_delete() {
    let mut sparse_set = grouped_sparse_set();

    let deleted = sparse_set.dyn_delete(EntityId::new_from_parts(1, 0), TrackingTimestamp::new(0));

    assert!(deleted);
    assert_shrunk(&sparse_set);
}

/// Asserts that `retain` removing a single component from a grouped storage keeps its range,
/// without the component and only trusting the positions before it.
#[test]
fn groups_retain() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set.private_retain(TrackingTimestamp::new(0), |_, component| component.0 != 1);

    assert_shrunk(&sparse_set);
}

/// Asserts that `retain_mut` removing a single component from a grouped storage keeps its range,
/// without the component and only trusting the positions before it.
#[test]
fn groups_retain_mut() {
    let mut sparse_set = grouped_sparse_set();

    sparse_set.private_retain_mut(TrackingTimestamp::new(0), |_, component| component.0 != 1);

    assert_shrunk(&sparse_set);
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
