use crate::{memory_usage::StorageMemoryUsage, storage::Storage, Component};

/// Unique storage.
pub struct Unique<T: Component> {
    pub(crate) value: T,
    pub(crate) is_modified: bool,
}

impl<T: Component> Storage for Unique<T> {
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(StorageMemoryUsage {
            storage_name: core::any::type_name::<Self>().into(),
            allocated_memory_bytes: core::mem::size_of::<Self>(),
            used_memory_bytes: core::mem::size_of::<Self>(),
            component_count: 1,
        })
    }
}

impl<T: Component> Unique<T> {
    pub(crate) fn new(value: T) -> Self {
        Unique {
            value,
            is_modified: false,
        }
    }
}
