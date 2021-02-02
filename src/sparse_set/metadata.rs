use crate::entity_id::EntityId;
use alloc::vec::Vec;

pub struct Metadata<T> {
    pub(super) track_insertion: bool,
    pub(super) track_modification: bool,
    pub(super) track_removal: Option<Vec<EntityId>>,
    pub(super) track_deletion: Option<Vec<(EntityId, T)>>,
}

impl<T> Metadata<T> {
    pub(crate) fn new() -> Self {
        Metadata {
            track_insertion: false,
            track_modification: false,
            track_removal: None,
            track_deletion: None,
        }
    }
    pub(crate) fn used_memory(&self) -> usize {
        core::mem::size_of::<Self>()
            + self
                .track_removal
                .as_ref()
                .map(|removed| removed.len() * core::mem::size_of::<EntityId>())
                .unwrap_or(0)
            + self
                .track_deletion
                .as_ref()
                .map(|deletion| deletion.len() * core::mem::size_of::<(EntityId, T)>())
                .unwrap_or(0)
    }
    pub(crate) fn reserved_memory(&self) -> usize {
        core::mem::size_of::<Self>()
            + self
                .track_removal
                .as_ref()
                .map(|removed| removed.capacity() * core::mem::size_of::<EntityId>())
                .unwrap_or(0)
            + self
                .track_deletion
                .as_ref()
                .map(|deletion| deletion.capacity() * core::mem::size_of::<(EntityId, T)>())
                .unwrap_or(0)
    }
}
