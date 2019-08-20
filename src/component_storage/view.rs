use super::{hasher::TypeIdHasher, ComponentStorage};
use crate::entity::Key;
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// View of all component storages.
/// Let you remove entities.
pub struct AllStoragesViewMut<'a>(
    pub(super) &'a mut HashMap<TypeId, ComponentStorage, BuildHasherDefault<TypeIdHasher>>,
);

impl AllStoragesViewMut<'_> {
    pub(crate) fn delete(&mut self, entity: Key) {
        for storage in self.0.values_mut() {
            storage.delete(entity).unwrap();
        }
        unimplemented!()
    }
}
