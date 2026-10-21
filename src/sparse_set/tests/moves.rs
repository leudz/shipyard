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
