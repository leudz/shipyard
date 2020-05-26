use crate::storage::EntityId;
use crate::unknown_storage::UnknownStorage;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

pub(super) struct Unique<T>(pub(crate) T);

impl<T: 'static> UnknownStorage for Unique<T> {
    fn delete(&mut self, _: EntityId, _: &mut Vec<TypeId>) {}
    fn clear(&mut self) {}
    fn unpack(&mut self, _: EntityId) {}
    fn any(&self) -> &dyn Any {
        &self.0
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        &mut self.0
    }
}
