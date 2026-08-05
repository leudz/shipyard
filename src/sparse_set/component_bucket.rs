use crate::component::Component;
use crate::entity_id::EntityId;
use crate::tracking::TrackingTimestamp;
use alloc::vec::Vec;
use arrayvec::ArrayVec;
use core::any::TypeId;

pub(crate) struct ComponentBucket<T: Component> {
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) insertion_data: Vec<TrackingTimestamp>,
    pub(crate) modification_data: Vec<TrackingTimestamp>,
    pub(crate) group: ArrayVec<TypeId, 10>,
}

impl<T: Component> ComponentBucket<T> {
    #[inline]
    pub(super) fn new() -> Self {
        ComponentBucket {
            dense: Vec::new(),
            data: Vec::new(),
            insertion_data: Vec::new(),
            modification_data: Vec::new(),
            group: ArrayVec::new(),
        }
    }
}
