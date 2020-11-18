use super::SparseSet;
use crate::sparse_set::SparseArray;
use crate::storage::{AllStorages, EntityId};
use alloc::vec::Vec;

pub struct Metadata<T> {
    pub(crate) update: Option<UpdatePack<T>>,
    pub(super) local_on_insert: Vec<fn(EntityId, &mut SparseSet<T>)>,
    pub(super) local_on_remove: Vec<fn(EntityId, &mut SparseSet<T>)>,
    pub(crate) global_on_insert: Vec<fn(EntityId, &mut SparseSet<T>, &AllStorages)>,
    pub(crate) on_insert_ids_sparse: SparseArray<[EntityId; super::BUCKET_SIZE]>,
    pub(crate) on_insert_ids_dense: Vec<EntityId>,
    pub(crate) global_on_remove: Vec<fn(EntityId, &mut SparseSet<T>, &AllStorages)>,
    pub(crate) on_remove_ids_sparse: SparseArray<[EntityId; super::BUCKET_SIZE]>,
    pub(crate) on_remove_ids_dense: Vec<EntityId>,
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
