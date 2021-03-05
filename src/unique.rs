use crate::{memory_usage::StorageMemoryUsage, storage::Storage};

/// Unique storage.
pub struct Unique<T> {
    pub(crate) value: T,
    pub(crate) is_modified: bool,
}

impl<T: 'static> Storage for Unique<T> {
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(StorageMemoryUsage {
            storage_name: core::any::type_name::<Self>().into(),
            allocated_memory_bytes: core::mem::size_of::<Self>(),
            used_memory_bytes: core::mem::size_of::<Self>(),
            component_count: 1,
        })
    }
}

impl<T> Unique<T> {
    pub(crate) fn new(value: T) -> Self {
        Unique {
            value,
            is_modified: false,
        }
    }
}
