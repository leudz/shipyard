use crate::all_storages::AllStorages;
use crate::world::World;
use alloc::borrow::Cow;

pub struct WorldMemoryUsage<'w>(pub(crate) &'w World);

pub struct AllStoragesMemoryUsage<'a>(pub(crate) &'a AllStorages);

#[derive(Debug)]
pub struct SparseSetMemoryUsage {
    pub spase: usize,
    pub dense: usize,
    pub data: usize,
    pub insertion_data: usize,
    pub modification_data: usize,
    pub deletion_data: usize,
    pub removal_data: usize,
    pub self_data: usize,
}

impl SparseSetMemoryUsage {
    pub fn sum(&self) -> usize {
        self.spase
            + self.dense
            + self.data
            + self.insertion_data
            + self.modification_data
            + self.deletion_data
            + self.removal_data
            + self.self_data
    }
}

/// A enum to query the amount of memory a storage uses.
#[allow(missing_docs, unused)]
#[derive(Debug)]
pub enum StorageMemoryUsage {
    Entities {
        used_memory_bytes: usize,
        allocated_memory_bytes: usize,
        entity_count: usize,
    },
    SparseSet {
        storage_name: Cow<'static, str>,
        used_memory_usage: SparseSetMemoryUsage,
        allocated_memory_usage: SparseSetMemoryUsage,
        used_memory_bytes: usize,
        allocated_memory_bytes: usize,
        component_count: usize,
    },
    Unique {
        storage_name: Cow<'static, str>,
        used_memory_bytes: usize,
    },
}
