use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::tracking::TrackingTimestamp;
use crate::ShipHashSet;

/// Trait used as a bound for AllStorages::delete_any.
pub trait CustomDeleteAny {
    #[allow(missing_docs)]
    fn delete_any(&mut self, ids: &mut ShipHashSet<EntityId>, current: TrackingTimestamp);
}

impl CustomDeleteAny for () {
    #[inline]
    fn delete_any(&mut self, _: &mut ShipHashSet<EntityId>, _current: TrackingTimestamp) {}
}

impl<T: Component> CustomDeleteAny for SparseSet<T> {
    #[inline]
    fn delete_any(&mut self, ids: &mut ShipHashSet<EntityId>, current: TrackingTimestamp) {
        ids.extend(&self.dense);
        self.private_clear(current);
    }
}
