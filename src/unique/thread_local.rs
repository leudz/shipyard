use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::memory_usage::StorageMemoryUsage;
use crate::storage::{SBoxBuilder, Storage};
use crate::tracking::TrackingTimestamp;
use crate::unique::{Unique, UniqueStorage};
use core::any::type_name;

impl<T: Unique + Sync> Storage for NonSend<UniqueStorage<T>> {
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
            SBoxBuilder::new(NonSend(UniqueStorage {
                value: clone(&self.value),
                insert: other_current,
                modification: TrackingTimestamp::origin(),
                last_insert: TrackingTimestamp::origin(),
                last_modification: TrackingTimestamp::origin(),
                clone: None,
            }))
        })
    }
}

impl<T: Unique + Send> Storage for NonSync<UniqueStorage<T>> {
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
            SBoxBuilder::new(NonSync(UniqueStorage {
                value: clone(&self.value),
                insert: other_current,
                modification: TrackingTimestamp::origin(),
                last_insert: TrackingTimestamp::origin(),
                last_modification: TrackingTimestamp::origin(),
                clone: None,
            }))
        })
    }
}

impl<T: Unique> Storage for NonSendSync<UniqueStorage<T>> {
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
            SBoxBuilder::new(NonSendSync(UniqueStorage {
                value: clone(&self.value),
                insert: other_current,
                modification: TrackingTimestamp::origin(),
                last_insert: TrackingTimestamp::origin(),
                last_modification: TrackingTimestamp::origin(),
                clone: None,
            }))
        })
    }
}
