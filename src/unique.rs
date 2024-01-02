use crate::component::Unique;
use crate::memory_usage::StorageMemoryUsage;
use crate::storage::Storage;
use crate::tracking::TrackingTimestamp;

/// Unique storage.
pub struct UniqueStorage<T: Unique> {
    pub(crate) value: T,
    pub(crate) insert: TrackingTimestamp,
    pub(crate) modification: TrackingTimestamp,
    pub(crate) last_insert: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
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
    pub(crate) fn new(value: T, current: TrackingTimestamp) -> Self {
        UniqueStorage {
            value,
            insert: current,
            modification: current.furthest_from(),
            last_insert: current.furthest_from(),
            last_modification: current.furthest_from(),
        }
    }
}
