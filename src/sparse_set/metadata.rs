use super::SparseSet;
use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseArray;
use alloc::vec::Vec;

pub struct Metadata<T> {
    pub(crate) update: Option<UpdatePack<T>>,
    pub(super) local_on_insert: Vec<fn(EntityId, &mut SparseSet<T>)>,
    pub(super) local_on_remove: Vec<fn(EntityId, &mut SparseSet<T>)>,
    pub(crate) global_on_insert: Vec<fn(EntityId, &mut SparseSet<T>, &AllStorages)>,
    pub(crate) global_on_remove: Vec<fn(EntityId, &mut SparseSet<T>, &AllStorages)>,
    pub(crate) on_insert_ids_sparse: SparseArray<[EntityId; super::BUCKET_SIZE]>,
    pub(crate) on_insert_ids_dense: Vec<EntityId>,
    pub(crate) on_remove_ids_sparse: SparseArray<[EntityId; super::BUCKET_SIZE]>,
    pub(crate) on_remove_ids_dense: Vec<EntityId>,
}

impl<T> Metadata<T> {
    pub(crate) fn used_memory(&self) -> usize {
        core::mem::size_of::<Self>()
            + if let Some(update) = &self.update {
                update.removed.len() * core::mem::size_of::<EntityId>()
            } else {
                0
            }
            + if let Some(update) = &self.update {
                update.deleted.len() * core::mem::size_of::<EntityId>()
            } else {
                0
            }
            + self.local_on_insert.len() * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.local_on_remove.len() * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.global_on_insert.len() * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.global_on_remove.len() * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.on_insert_ids_sparse.used_memory()
            + self.on_insert_ids_dense.len() * core::mem::size_of::<EntityId>()
            + self.on_remove_ids_sparse.used_memory()
            + self.on_remove_ids_dense.len() * core::mem::size_of::<EntityId>()
    }
    pub(crate) fn reserved_memory(&self) -> usize {
        core::mem::size_of::<Self>()
            + if let Some(update) = &self.update {
                update.removed.capacity() * core::mem::size_of::<EntityId>()
            } else {
                0
            }
            + if let Some(update) = &self.update {
                update.deleted.capacity() * core::mem::size_of::<EntityId>()
            } else {
                0
            }
            + self.local_on_insert.capacity()
                * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.local_on_remove.capacity()
                * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.global_on_insert.capacity()
                * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.global_on_remove.capacity()
                * core::mem::size_of::<fn(EntityId, &mut SparseSet<T>)>()
            + self.on_insert_ids_sparse.reserved_memory()
            + self.on_insert_ids_dense.capacity() * core::mem::size_of::<EntityId>()
            + self.on_remove_ids_sparse.reserved_memory()
            + self.on_remove_ids_dense.capacity() * core::mem::size_of::<EntityId>()
    }
}

impl<T> Default for Metadata<T> {
    fn default() -> Self {
        Metadata {
            update: None,
            local_on_insert: Vec::new(),
            local_on_remove: Vec::new(),
            global_on_insert: Vec::new(),
            on_insert_ids_sparse: SparseArray::new(),
            on_insert_ids_dense: Vec::new(),
            global_on_remove: Vec::new(),
            on_remove_ids_sparse: SparseArray::new(),
            on_remove_ids_dense: Vec::new(),
        }
    }
}

pub(crate) struct UpdatePack<T> {
    pub(crate) removed: Vec<EntityId>,
    pub(crate) deleted: Vec<(EntityId, T)>,
}

impl<T> Default for UpdatePack<T> {
    fn default() -> Self {
        UpdatePack {
            removed: Vec::new(),
            deleted: Vec::new(),
        }
    }
}
