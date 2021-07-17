use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use hashbrown::hash_set::HashSet;

/// Trait used as a bound for AllStorages::delete_any.
pub trait CustomDeleteAny {
    fn delete_any(&mut self, ids: &mut HashSet<EntityId>);
}

impl CustomDeleteAny for () {
    #[inline]
    fn delete_any(&mut self, _: &mut HashSet<EntityId>) {}
}

impl<T: Component> CustomDeleteAny for SparseSet<T, T::Tracking> {
    #[inline]
    fn delete_any(&mut self, ids: &mut HashSet<EntityId>) {
        ids.extend(&self.dense);
        self.clear();
    }
}
