#[cfg(feature = "thread_local")]
mod thread_local;

use crate::component::Unique;
use crate::memory_usage::StorageMemoryUsage;
use crate::storage::{SBoxBuilder, Storage};
use crate::tracking::TrackingTimestamp;
use core::any::type_name;
use core::mem::size_of;

/// Unique storage.
pub struct UniqueStorage<T: Unique> {
    pub(crate) value: T,
    pub(crate) insert: TrackingTimestamp,
    pub(crate) modification: TrackingTimestamp,
    pub(crate) last_insert: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) clone: Option<fn(&T) -> T>,
}

impl<T: Unique + Send + Sync> Storage for UniqueStorage<T> {
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(StorageMemoryUsage {
            storage_name: type_name::<Self>().into(),
            allocated_memory_bytes: size_of::<Self>(),
            used_memory_bytes: size_of::<Self>(),
            component_count: 1,
        })
    }

    fn is_empty(&self) -> bool {
        false
    }

    fn clear_all_inserted(&mut self, current: TrackingTimestamp) {
        self.insert = current;
    }

    fn clear_all_modified(&mut self, current: TrackingTimestamp) {
        self.modification = current;
    }

    fn try_clone(&self, other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        self.clone.map(|clone| {
            SBoxBuilder::new(UniqueStorage {
                value: clone(&self.value),
                insert: other_current,
                modification: TrackingTimestamp::origin(),
                last_insert: TrackingTimestamp::origin(),
                last_modification: TrackingTimestamp::origin(),
                clone: None,
            })
        })
    }
}

impl<T: Unique> UniqueStorage<T> {
    pub(crate) fn new(value: T, current: TrackingTimestamp) -> Self {
        UniqueStorage {
            value,
            insert: current,
            modification: TrackingTimestamp::origin(),
            last_insert: TrackingTimestamp::origin(),
            last_modification: TrackingTimestamp::origin(),
            clone: None,
        }
    }
}

impl<T: Unique + Clone> UniqueStorage<T> {
    pub(crate) fn register_clone(&mut self) {
        self.clone = Some(T::clone)
    }
}
