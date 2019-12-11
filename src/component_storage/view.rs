use super::{hasher::TypeIdHasher, Storage};
use crate::entity::Key;
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// View of all component storages.
/// Let you remove entities.
pub struct AllStoragesViewMut<'a>(
    pub(super) &'a mut HashMap<TypeId, Storage, BuildHasherDefault<TypeIdHasher>>,
);

impl AllStoragesViewMut<'_> {
    pub(crate) fn delete(&mut self, entity: Key) {
        let mut storage_to_unpack = Vec::new();
        for storage in self.0.values_mut() {
            let observers = storage.delete(entity).unwrap();
            storage_to_unpack.reserve(observers.len());
            let mut i = 0;
            for observer in observers.iter().copied() {
                while i < storage_to_unpack.len() && observer < storage_to_unpack[i] {
                    i += 1;
                }
                if storage_to_unpack.is_empty() || observer != storage_to_unpack[i] {
                    storage_to_unpack.insert(i, observer);
                }
            }
        }

        for storage in storage_to_unpack {
            self.0.get_mut(&storage).unwrap().unpack(entity).unwrap();
        }
    }
}
