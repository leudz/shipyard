use crate::entity_id::EntityId;
use alloc::vec::Vec;

pub struct Metadata<T> {
    pub(crate) update: Option<UpdatePack<T>>,
}

impl<T> Metadata<T> {
    pub(crate) fn used_memory(&self) -> usize {
        core::mem::size_of::<Self>()
            + self
                .update
                .as_ref()
                .map(|update| update.used_memory())
                .unwrap_or(0)
    }
    pub(crate) fn reserved_memory(&self) -> usize {
        core::mem::size_of::<Self>()
            + self
                .update
                .as_ref()
                .map(|update| update.reserved_memory())
                .unwrap_or(0)
    }
}

impl<T> Default for Metadata<T> {
    fn default() -> Self {
        Metadata { update: None }
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

impl<T> UpdatePack<T> {
    fn used_memory(&self) -> usize {
        self.removed.len() * core::mem::size_of::<EntityId>()
            + self.deleted.len() * core::mem::size_of::<EntityId>()
    }
    fn reserved_memory(&self) -> usize {
        self.removed.capacity() * core::mem::size_of::<EntityId>()
            + self.deleted.capacity() * core::mem::size_of::<EntityId>()
    }
}
