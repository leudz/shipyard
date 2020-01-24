use crate::sparse_set::SparseSet;
use crate::storage::Entities;
use crate::storage::EntityId;
use core::any::{TypeId, Any};

pub(super) trait UnknownStorage {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<TypeId>);
    fn unpack(&mut self, entitiy: EntityId);
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
    }

impl<T: 'static> UnknownStorage for SparseSet<T> {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<TypeId>) {
        self.actual_delete(entity);

        storage_to_unpack.reserve(self.pack_info.observer_types.len());

        let mut i = 0;
        for observer in self.pack_info.observer_types.iter().copied() {
            while i < storage_to_unpack.len() && observer < storage_to_unpack[i] {
                i += 1;
            }
            if storage_to_unpack.is_empty() || observer != storage_to_unpack[i] {
                storage_to_unpack.insert(i, observer);
            }
        }
    }
    fn unpack(&mut self, entity: EntityId) {
        Self::unpack(self, entity);
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl dyn UnknownStorage {
    pub(crate) fn sparse_set<T: 'static>(&self) -> Option<&SparseSet<T>> {
        self.any().downcast_ref()
    }
    pub(crate) fn sparse_set_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        self.any_mut().downcast_mut()
    }
    pub(crate) fn entities(&self) -> Option<&Entities> {
        self.any().downcast_ref()
    }
    pub(crate) fn entities_mut(&mut self) -> Option<&mut Entities> {
        self.any_mut().downcast_mut()
    }
}
