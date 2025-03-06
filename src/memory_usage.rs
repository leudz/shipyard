pub use crate::sparse_set::{SparseSetMemory, SparseSetMemoryUsage};

use crate::all_storages::AllStorages;
use crate::world::World;
use alloc::borrow::Cow;

#[allow(missing_docs)]
pub struct WorldMemoryUsage<'w>(pub(crate) &'w World);

#[allow(missing_docs)]
pub struct AllStoragesMemoryUsage<'a>(pub(crate) &'a AllStorages);

/// A structure representing the memory usage of a storage.
pub struct StorageMemoryUsage {
    #[allow(missing_docs)]
    pub storage_name: Cow<'static, str>,
    /// Amount of memory used by the storage in bytes.
    pub used_memory_bytes: usize,
    /// Amount of memory allocated by the storage in bytes (including reserved memory).
    pub allocated_memory_bytes: usize,
    #[allow(missing_docs)]
    pub component_count: usize,
}

impl core::fmt::Debug for StorageMemoryUsage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "{}: {} bytes used for {} components ({} bytes reserved in total)",
            self.storage_name,
            self.used_memory_bytes,
            self.component_count,
            self.allocated_memory_bytes
        ))
    }
}

/// A trait to query the detailed memory usage of a storage
pub trait MemoryUsageDetail {
    /// The output type of the detailed memory usage.
    type Out: core::fmt::Debug;

    /// Returns the detailed memory usage of the storage.
    fn detailed_memory_usage(&self) -> Self::Out;
}
