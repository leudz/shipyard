mod hasher;
mod view;

use super::{Entities, EntityId, Storage};
use crate::atomic_refcell::AtomicRefCell;
use crate::unknown_storage::UnknownStorage;
pub(crate) use hasher::TypeIdHasher;
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
pub use view::AllStoragesViewMut;

/// Contains all components present in the World.
// Wrapper to hide `TypeIdHasher` and the whole `HashMap` from public interface
pub struct AllStorages(pub(crate) HashMap<TypeId, Storage, BuildHasherDefault<TypeIdHasher>>);

impl Default for AllStorages {
    fn default() -> Self {
        let mut storages = HashMap::default();

        let entities = Entities::default();
        let unknown: [*const (); 2] = unsafe {
            *(&(&entities as &dyn UnknownStorage as *const _) as *const *const _
                as *const [*const (); 2])
        };

        storages.insert(
            TypeId::of::<Entities>(),
            Storage {
                container: AtomicRefCell::new(Box::new(entities), None, true),
                unknown: unknown[1],
            },
        );

        AllStorages(storages)
    }
}

impl AllStorages {
    /// Register a new unique component and create a storage for it.
    /// Does nothing if a storage already exists.
    pub(crate) fn register_unique<T: 'static + Send + Sync>(&mut self, componnent: T) {
        let type_id = TypeId::of::<T>();
        let storage = Storage::new::<T>();
        storage.sparse_set_mut().unwrap().insert_unique(componnent);
        if let Some(storage) = self.0.insert(type_id, storage) {
            *self.0.get_mut(&type_id).unwrap() = storage;
        }
    }
    pub(crate) fn view_mut(&mut self) -> AllStoragesViewMut {
        AllStoragesViewMut(&mut self.0)
    }
}
