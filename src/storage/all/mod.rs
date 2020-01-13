mod hasher;

use super::{Entities, EntityId, Storage};
use crate::atomic_refcell::AtomicRefCell;
use crate::unknown_storage::UnknownStorage;
pub(crate) use hasher::TypeIdHasher;
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// Contains all components present in the World.
// Wrapper to hide `TypeIdHasher` and the whole `HashMap` from public interface
pub struct AllStorages(pub(crate) HashMap<TypeId, Storage, BuildHasherDefault<TypeIdHasher>>);

impl Default for AllStorages {
    fn default() -> Self {
        let mut storages = HashMap::default();

        let entities = Entities::default();
        let unknown: [*const (); 2] = unsafe {
            let unknown: &dyn UnknownStorage = &entities;
            let unknown: *const _ = unknown;
            let unknown: *const *const _ = &unknown;
            *(unknown as *const [*const (); 2])
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
    /// Delete an entity and all its components.
    /// Returns `true` if `entity` was alive.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// let mut entity1 = None;
    /// let mut entity2 = None;
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entity1 = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
    ///     entity2 = Some(entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)));
    /// });
    ///
    /// world.run::<AllStorages, _, _>(|mut all_storages| {
    ///     all_storages.delete(entity1.unwrap());
    /// });
    ///
    /// world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
    ///     assert_eq!((&usizes).get(entity1.unwrap()), None);
    ///     assert_eq!((&u32s).get(entity1.unwrap()), None);
    ///     assert_eq!(usizes.get(entity2.unwrap()), Some(&2));
    ///     assert_eq!(u32s.get(entity2.unwrap()), Some(&3));
    /// });
    /// ```
    pub fn delete(&mut self, entity: EntityId) -> bool {
        let mut entities = self.0[&TypeId::of::<Entities>()].entities_mut().unwrap();

        if entities.delete(entity) {
            drop(entities);

            self.strip(entity);

            true
        } else {
            false
        }
    }
    /// Deletes all components from an entity without deleting it.
    pub fn strip(&mut self, entity: EntityId) {
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
    pub fn register<T: 'static + Send + Sync>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(Storage::new::<T>);
    }
    pub fn register_non_send_non_sync<T: 'static>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(Storage::new_non_send_non_sync::<T>);
    }
    pub fn register_non_send<T: 'static + Sync>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(Storage::new_non_send::<T>);
    }
    pub fn register_non_sync<T: 'static + Send>(&mut self) {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(Storage::new_non_sync::<T>);
    }
}
