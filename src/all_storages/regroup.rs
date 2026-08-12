use super::storage_mask::StorageMask;
use crate::entity_id::EntityId;
use crate::ShipHashMap;
use alloc::vec::Vec;
use core::any::TypeId;

pub(super) struct RegroupCandidate {
    pub(super) storages: StorageMask,
    pub(super) expanded_storages: StorageMask,
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct RegroupBatchKey {
    pub(super) moved_storages: StorageMask,
    pub(super) target_group: StorageMask,
}

pub(super) struct RemovedGroupBatch {
    pub(super) type_ids: Vec<TypeId>,
    pub(super) entities: Vec<EntityId>,
}

pub(super) type InvalidatedGroups = ShipHashMap<EntityId, StorageMask>;

pub(super) struct RegroupBatch {
    /// Dense indices into `participating_storages`.
    pub(super) storage_indices: Vec<usize>,
    /// Sorted storage `TypeId`s passed to each storage as its group signature.
    pub(super) type_ids: Vec<TypeId>,
    /// Entities that resolved to this exact signature.
    pub(super) entities: Vec<EntityId>,
}

impl RegroupCandidate {
    pub(super) fn new(storages: StorageMask) -> Self {
        Self {
            expanded_storages: StorageMask::zero_like(&storages),
            storages,
        }
    }

    pub(super) fn merge(&mut self, other: RegroupCandidate) {
        self.storages.union_with(&other.storages);
        self.expanded_storages.union_with(&other.expanded_storages);
    }
}
