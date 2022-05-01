use crate::all_storages::AllStorages;
use crate::world::World;
use alloc::borrow::Cow;

pub struct WorldMemoryUsage<'w>(pub(crate) &'w World);

pub struct AllStoragesMemoryUsage<'a>(pub(crate) &'a AllStorages);

/// A trait to query the amount of memory a storage uses.
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
