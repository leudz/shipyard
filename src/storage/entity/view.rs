use super::add_component::AddComponent;
use super::{super::AllStoragesViewMut, Key};
use crate::error;
use crate::sparse_set::ViewAddEntity;

/// View into the entities.
pub struct EntitiesView<'a> {
    pub(super) data: &'a [Key],
}

impl EntitiesView<'_> {
    /// Returns true if the key matches a living entity.
    pub(crate) fn is_alive(&self, key: Key) -> bool {
        key.index() < self.data.len() && key == unsafe { *self.data.get_unchecked(key.index()) }
    }
    pub fn try_add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: Key,
    ) -> Result<(), error::AddComponent> {
        storages.try_add_component(component, entity, &self)
    }
    pub fn add_component<C, S: AddComponent<C>>(&self, storages: S, component: C, entity: Key) {
        storages
            .try_add_component(component, entity, &self)
            .unwrap()
    }
}

/// View into the entities, this allows to add and remove entities.
pub struct EntitiesViewMut<'a> {
    pub(super) data: &'a mut Vec<Key>,
    pub(crate) list: &'a mut Option<(usize, usize)>,
}

impl EntitiesViewMut<'_> {
    pub(super) fn generate(&mut self) -> Key {
        let index = self.list.map(|(_, old)| old);
        if let Some((new, ref mut old)) = self.list {
            if *new == *old {
                *self.list = None;
            } else {
                *old = unsafe { self.data.get_unchecked(*old).index() };
            }
        }
        if let Some(index) = index {
            unsafe { self.data.get_unchecked_mut(index).set_index(index as u64) };
            unsafe { *self.data.get_unchecked(index) }
        } else {
            let key = Key::new(self.data.len() as u64);
            self.data.push(key);
            key
        }
    }
    /// Returns true if the key matches a living entity.
    fn is_alive(&self, key: Key) -> bool {
        self.as_non_mut().is_alive(key)
    }
    /// Delete an entity, returns true if the entity was alive.
    pub(super) fn delete_key(&mut self, key: Key) -> bool {
        if self.is_alive(key) {
            if unsafe {
                self.data
                    .get_unchecked_mut(key.index())
                    .bump_version()
                    .is_ok()
            } {
                if let Some((ref mut new, _)) = self.list {
                    unsafe {
                        self.data
                            .get_unchecked_mut(*new)
                            .set_index(key.index() as u64)
                    };
                    *new = key.index();
                } else {
                    *self.list = Some((key.index(), key.index()));
                }
            }
            true
        } else {
            false
        }
    }
    /// Stores `component` in a new entity, the `Key` to this entity is returned.
    ///
    /// Multiple components can be added at the same time using a tuple.
    /// # Example:
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///
    ///     assert_eq!(usizes.get(entity), Some(&0));
    ///     assert_eq!(u32s.get(entity), Some(&1));
    /// });
    /// ```
    pub fn add_entity<T: ViewAddEntity>(&mut self, storages: T, component: T::Component) -> Key {
        let key = self.generate();
        storages.add_entity(component, key);
        key
    }
    /// Delete an entity and all its components.
    /// Returns true if the entity was alive.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// let mut entity1 = None;
    /// let mut entity2 = None;
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entity1 = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
    ///     entity2 = Some(entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)));
    /// });
    ///
    /// world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
    ///     entities.delete(&mut all_storages, entity1.unwrap());
    /// });
    ///
    /// world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
    ///     assert_eq!((&usizes).get(entity1.unwrap()), None);
    ///     assert_eq!((&u32s).get(entity1.unwrap()), None);
    ///     assert_eq!(usizes.get(entity2.unwrap()), Some(&2));
    ///     assert_eq!(u32s.get(entity2.unwrap()), Some(&3));
    /// });
    /// ```
    ///
    /// [World::delete]: struct.World.html#method.delete
    /// [Entities]: struct.Entities.html
    /// [AllStorages]: struct.AllStorages.html
    pub fn delete(&mut self, storages: &mut AllStoragesViewMut, entity: Key) -> bool {
        if self.delete_key(entity) {
            storages.delete(entity);
            true
        } else {
            false
        }
    }
    fn as_non_mut(&self) -> EntitiesView {
        EntitiesView { data: self.data }
    }
    pub fn try_add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: Key,
    ) -> Result<(), error::AddComponent> {
        storages.try_add_component(component, entity, &self.as_non_mut())
    }
    pub fn add_component<C, S: AddComponent<C>>(&self, storages: S, component: C, entity: Key) {
        storages
            .try_add_component(component, entity, &self.as_non_mut())
            .unwrap()
    }
}
