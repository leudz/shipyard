use super::*;
use crate::storage::{Storage, StorageId};
use alloc::sync::Arc;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct I32(i32);

impl Component for I32 {
    type Tracking = crate::track::Untracked;
}

/// Makes sure the `memory_usage` method is up-to-date with the current `SparseSet` data.
#[test]
fn memory_usage() {
    let mut sparse_set = SparseSet::new();
    sparse_set.track_all();

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(0),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    sparse_set.delete(
        EntityId::new_from_index_and_gen(0, 0),
        TrackingTimestamp::new(0),
    );

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(1),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    sparse_set.dyn_remove(
        EntityId::new_from_index_and_gen(0, 0),
        TrackingTimestamp::new(0),
    );

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(2),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();

    let expected_sparse_memory = sparse_set.sparse.used_memory();
    let expected_dense_memory = 1 * size_of::<EntityId>();
    let expected_data_memory = 1 * size_of::<I32>();
    let expected_insertion_tracking_memory = 1 * size_of::<TrackingTimestamp>();
    let expected_modification_tracking_memory = 1 * size_of::<TrackingTimestamp>();
    let expected_deletion_tracking_memory = 1 * size_of::<(EntityId, TrackingTimestamp, I32)>();
    let expected_removal_tracking_memory = 1 * size_of::<(EntityId, TrackingTimestamp)>();
    let expected_groups_memory = 0;
    let expected_self_memory = size_of::<SparseSet<I32>>();
    let expected_total_memory = expected_sparse_memory
        + expected_dense_memory
        + expected_data_memory
        + expected_insertion_tracking_memory
        + expected_modification_tracking_memory
        + expected_deletion_tracking_memory
        + expected_removal_tracking_memory
        + expected_groups_memory
        + expected_self_memory;

    let memory_usage = sparse_set.private_memory_usage();

    assert_eq!(memory_usage.used_memory_bytes, expected_total_memory);
}

/// Verifies that the `detailed_memory_usage` method accurately reflects the current `SparseSet` data breakdown.
#[test]
fn detailed_memory_usage() {
    let mut sparse_set = SparseSet::new();
    sparse_set.track_all();

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(0),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    sparse_set.delete(
        EntityId::new_from_index_and_gen(0, 0),
        TrackingTimestamp::new(0),
    );

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(1),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();
    sparse_set.dyn_remove(
        EntityId::new_from_index_and_gen(0, 0),
        TrackingTimestamp::new(0),
    );

    sparse_set
        .insert(
            EntityId::new_from_parts(0, 0),
            I32(2),
            TrackingTimestamp::new(0),
        )
        .assert_inserted();

    let expected_sparse_allocated = sparse_set.sparse.reserved_memory();
    let expected_dense_allocated = sparse_set.dense.capacity() * size_of::<EntityId>();
    let expected_data_allocated = sparse_set.data.capacity() * size_of::<I32>();
    let expected_insertion_allocated =
        sparse_set.insertion_data.capacity() * size_of::<TrackingTimestamp>();
    let expected_modification_allocated =
        sparse_set.modification_data.capacity() * size_of::<TrackingTimestamp>();
    let expected_deletion_allocated =
        sparse_set.deletion_data.capacity() * size_of::<(EntityId, TrackingTimestamp, I32)>();
    let expected_removal_allocated =
        sparse_set.removal_data.capacity() * size_of::<(EntityId, TrackingTimestamp)>();

    let expected_sparse_used = sparse_set.sparse.used_memory();
    let expected_dense_used = sparse_set.dense.len() * size_of::<EntityId>();
    let expected_data_used = sparse_set.data.len() * size_of::<I32>();
    let expected_insertion_used = sparse_set.insertion_data.len() * size_of::<TrackingTimestamp>();
    let expected_modification_used =
        sparse_set.modification_data.len() * size_of::<TrackingTimestamp>();
    let expected_deletion_used =
        sparse_set.deletion_data.len() * size_of::<(EntityId, TrackingTimestamp, I32)>();
    let expected_removal_used =
        sparse_set.removal_data.len() * size_of::<(EntityId, TrackingTimestamp)>();

    let memory_usage = sparse_set.private_memory_usage();
    let detailed_usage = sparse_set.detailed_memory_usage();

    assert_eq!(detailed_usage.base.storage_name, memory_usage.storage_name);
    assert_eq!(
        detailed_usage.base.component_count,
        memory_usage.component_count
    );
    assert_eq!(
        detailed_usage.base.used_memory_bytes,
        memory_usage.used_memory_bytes
    );
    assert_eq!(
        detailed_usage.base.allocated_memory_bytes,
        memory_usage.allocated_memory_bytes
    );
    assert_eq!(
        memory_usage.used_memory_bytes,
        (detailed_usage.used.sparse
            + detailed_usage.used.dense
            + detailed_usage.used.data
            + detailed_usage.used.insertion_data
            + detailed_usage.used.modification_data
            + detailed_usage.used.deletion_data
            + detailed_usage.used.removal_data
            + detailed_usage.used.groups
            + detailed_usage.used.itself)
    );
    assert_eq!(
        memory_usage.allocated_memory_bytes,
        (detailed_usage.allocated.sparse
            + detailed_usage.allocated.dense
            + detailed_usage.allocated.data
            + detailed_usage.allocated.insertion_data
            + detailed_usage.allocated.modification_data
            + detailed_usage.allocated.deletion_data
            + detailed_usage.allocated.removal_data
            + detailed_usage.allocated.groups
            + detailed_usage.allocated.itself)
    );

    assert_eq!(detailed_usage.allocated.sparse, expected_sparse_allocated);
    assert_eq!(detailed_usage.allocated.dense, expected_dense_allocated);
    assert_eq!(detailed_usage.allocated.data, expected_data_allocated);
    assert_eq!(
        detailed_usage.allocated.insertion_data,
        expected_insertion_allocated
    );
    assert_eq!(
        detailed_usage.allocated.modification_data,
        expected_modification_allocated
    );
    assert_eq!(
        detailed_usage.allocated.deletion_data,
        expected_deletion_allocated
    );
    assert_eq!(
        detailed_usage.allocated.removal_data,
        expected_removal_allocated
    );

    assert_eq!(detailed_usage.used.sparse, expected_sparse_used);
    assert_eq!(detailed_usage.used.dense, expected_dense_used);
    assert_eq!(detailed_usage.used.data, expected_data_used);
    assert_eq!(detailed_usage.used.insertion_data, expected_insertion_used);
    assert_eq!(
        detailed_usage.used.modification_data,
        expected_modification_used
    );
    assert_eq!(detailed_usage.used.deletion_data, expected_deletion_used);
    assert_eq!(detailed_usage.used.removal_data, expected_removal_used);
    assert_eq!(detailed_usage.used.groups, 0);
}

/// Asserts that one attached group range is reported as used memory,
/// and that the detailed and total reports agree.
#[test]
fn groups_memory_usage() {
    let mut sparse_set = SparseSet::<I32>::new();
    sparse_set.groups.ranges.push(GroupRange {
        group: Arc::from([StorageId::Custom(0), StorageId::Custom(1)]),
        start: 0,
        len: 0,
        shift: 0,
        trust: 0,
    });

    let detailed_usage = sparse_set.detailed_memory_usage();

    assert_eq!(detailed_usage.used.groups, size_of::<GroupRange>());
    assert_eq!(
        detailed_usage.allocated.groups,
        sparse_set.groups.ranges.capacity() * size_of::<GroupRange>()
    );
    assert_eq!(
        detailed_usage.base.used_memory_bytes,
        sparse_set.sparse.used_memory() + size_of::<GroupRange>() + size_of::<SparseSet<I32>>()
    );
}
