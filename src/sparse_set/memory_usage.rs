#[cfg(test)]
mod tests;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::memory_usage::{MemoryUsageDetail, StorageMemoryUsage};
use crate::sparse_set::groups::GroupRange;
use crate::sparse_set::SparseSet;
use crate::tracking::TrackingTimestamp;
use core::any::type_name;
use core::fmt;
use core::mem::size_of;

impl<T: Component> SparseSet<T> {
    pub(super) fn private_memory_usage(&self) -> StorageMemoryUsage {
        StorageMemoryUsage {
            storage_name: type_name::<Self>().into(),
            allocated_memory_bytes: self.allocated_memory_bytes(),
            used_memory_bytes: self.used_memory_bytes(),
            component_count: self.len(),
        }
    }

    fn allocated_memory_bytes(&self) -> usize {
        self.sparse.reserved_memory()
            + (self.dense.capacity() * size_of::<EntityId>())
            + (self.data.capacity() * size_of::<T>())
            + (self.insertion_data.capacity() * size_of::<TrackingTimestamp>())
            + (self.modification_data.capacity() * size_of::<TrackingTimestamp>())
            + (self.deletion_data.capacity() * size_of::<(EntityId, TrackingTimestamp, T)>())
            + (self.removal_data.capacity() * size_of::<(EntityId, TrackingTimestamp)>())
            + (self.groups.ranges.capacity() * size_of::<GroupRange>())
            + size_of::<Self>()
    }

    fn used_memory_bytes(&self) -> usize {
        self.sparse.used_memory()
            + (self.dense.len() * size_of::<EntityId>())
            + (self.data.len() * size_of::<T>())
            + (self.insertion_data.len() * size_of::<TrackingTimestamp>())
            + (self.modification_data.len() * size_of::<TrackingTimestamp>())
            + (self.deletion_data.len() * size_of::<(EntityId, TrackingTimestamp, T)>())
            + (self.removal_data.len() * size_of::<(EntityId, TrackingTimestamp)>())
            + (self.groups.ranges.len() * size_of::<GroupRange>())
            + size_of::<Self>()
    }
}

/// Detailed memory usage of a `SparseSet`.
pub struct SparseSetMemoryUsage {
    #[allow(missing_docs)]
    pub base: StorageMemoryUsage,
    #[allow(missing_docs)]
    pub allocated: SparseSetMemory,
    #[allow(missing_docs)]
    pub used: SparseSetMemory,
}

impl fmt::Debug for SparseSetMemoryUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{:?}\n - used: {:?}\n - reserved: {:?}",
            self.base, self.used, self.allocated,
        ))
    }
}

/// Detailed memory layout of a `SparseSet`.
pub struct SparseSetMemory {
    #[allow(missing_docs)]
    pub sparse: usize,
    #[allow(missing_docs)]
    pub dense: usize,
    #[allow(missing_docs)]
    pub data: usize,
    #[allow(missing_docs)]
    pub insertion_data: usize,
    #[allow(missing_docs)]
    pub modification_data: usize,
    #[allow(missing_docs)]
    pub deletion_data: usize,
    #[allow(missing_docs)]
    pub removal_data: usize,
    /// Group ranges metadata. Group member lists are shared between storages and not counted.
    pub groups: usize,
    #[allow(missing_docs)]
    pub itself: usize,
}

impl fmt::Debug for SparseSetMemory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "(sparse: {}, dense: {}, data: {}, insertion: {}, modification: {}, deletion: {}, removal: {}, groups: {}, self: {})",
            self.sparse, self.dense, self.data, self.insertion_data, self.modification_data, self.deletion_data, self.removal_data, self.groups, self.itself
        ))
    }
}

impl<T: Component> MemoryUsageDetail for SparseSet<T> {
    type Out = SparseSetMemoryUsage;

    fn detailed_memory_usage(&self) -> Self::Out {
        SparseSetMemoryUsage {
            base: self.private_memory_usage(),
            allocated: SparseSetMemory {
                sparse: self.sparse.reserved_memory(),
                dense: self.dense.capacity() * size_of::<EntityId>(),
                data: self.data.capacity() * size_of::<T>(),
                insertion_data: self.insertion_data.capacity() * size_of::<TrackingTimestamp>(),
                modification_data: self.modification_data.capacity()
                    * size_of::<TrackingTimestamp>(),
                deletion_data: self.deletion_data.capacity()
                    * size_of::<(EntityId, TrackingTimestamp, T)>(),
                removal_data: self.removal_data.capacity()
                    * size_of::<(EntityId, TrackingTimestamp)>(),
                groups: self.groups.ranges.capacity() * size_of::<GroupRange>(),
                itself: size_of::<SparseSet<T>>(),
            },
            used: SparseSetMemory {
                sparse: self.sparse.used_memory(),
                dense: self.dense.len() * size_of::<EntityId>(),
                data: self.data.len() * size_of::<T>(),
                insertion_data: self.insertion_data.len() * size_of::<TrackingTimestamp>(),
                modification_data: self.modification_data.len() * size_of::<TrackingTimestamp>(),
                deletion_data: self.deletion_data.len()
                    * size_of::<(EntityId, TrackingTimestamp, T)>(),
                removal_data: self.removal_data.len() * size_of::<(EntityId, TrackingTimestamp)>(),
                groups: self.groups.ranges.len() * size_of::<GroupRange>(),
                itself: size_of::<SparseSet<T>>(),
            },
        }
    }
}
