use super::{Entities, EntityId, Storage, TypeIdHasher};
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// View of all component storages.
/// Let you remove entities.
pub struct AllStoragesViewMut<'a>(
    pub(super) &'a mut HashMap<TypeId, Storage, BuildHasherDefault<TypeIdHasher>>,
);

impl AllStoragesViewMut<'_> {
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

            true
        } else {
            false
        }
    }
}
