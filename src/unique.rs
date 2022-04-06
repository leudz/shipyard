use crate::{component::Unique, memory_usage::StorageMemoryUsage, storage::Storage};

/// Unique storage.
pub struct UniqueStorage<T: Unique> {
    pub(crate) value: T,
    pub(crate) insert: u32,
    pub(crate) modification: u32,
    pub(crate) last_insert: u32,
    pub(crate) last_modification: u32,
}

impl<T: Unique> Storage for UniqueStorage<T> {
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(StorageMemoryUsage {
            storage_name: core::any::type_name::<Self>().into(),
            allocated_memory_bytes: core::mem::size_of::<Self>(),
            used_memory_bytes: core::mem::size_of::<Self>(),
            component_count: 1,
        })
    }
    fn is_empty(&self) -> bool {
        false
    }
}

impl<T: Unique> UniqueStorage<T> {
    pub(crate) fn new(value: T, current: u32) -> Self {
        UniqueStorage {
            value,
            insert: current,
            modification: 0,
            last_insert: 0,
            last_modification: 0,
        }
    }
}
