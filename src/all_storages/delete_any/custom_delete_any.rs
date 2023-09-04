use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::ShipHashSet;

/// Trait used as a bound for AllStorages::delete_any.
pub trait CustomDeleteAny {
    fn delete_any(&mut self, ids: &mut ShipHashSet<EntityId>, current: u32);
}

impl CustomDeleteAny for () {
    #[inline]
    fn delete_any(&mut self, _: &mut ShipHashSet<EntityId>, _current: u32) {}
}

impl<T: Component> CustomDeleteAny for SparseSet<T> {
    #[inline]
    fn delete_any(&mut self, ids: &mut ShipHashSet<EntityId>, current: u32) {
        ids.extend(&self.dense);
        self.private_clear(current);
    }
}
