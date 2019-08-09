use super::Key;
use crate::component_storage::AllStoragesViewMut;
use crate::sparse_array::ViewAddEntity;

/// View into the entities, this allows to add and remove entities.
pub struct EntitiesViewMut<'a> {
    pub(super) data: &'a mut Vec<Key>,
    pub(crate) list: &'a mut Option<(usize, usize)>,
}

impl<'a> EntitiesViewMut<'a> {
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
            unsafe { self.data.get_unchecked_mut(index).set_index(index) };
            unsafe { *self.data.get_unchecked(index) }
        } else {
            let key = Key::new(self.data.len());
            self.data.push(key);
            key
        }
    }
    /// Return true if the key matches a living entity
    fn is_alive(&self, key: Key) -> bool {
        key.index() < self.data.len() && key == unsafe { *self.data.get_unchecked(key.index()) }
    }
    /// Delete an entity, returns true if the entity was alive
    pub(super) fn delete_key(&mut self, key: Key) -> bool {
        if self.is_alive(key) {
            if unsafe {
                self.data
                    .get_unchecked_mut(key.index())
                    .bump_version()
                    .is_ok()
            } {
                if let Some((ref mut new, _)) = self.list {
                    unsafe { self.data.get_unchecked_mut(*new).set_index(key.index()) };
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
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// world.run::<(Entities, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity = entities.add((&mut usizes, &mut u32s), (0, 1));
    ///
    ///     assert_eq!(usizes.get(entity), Some(&0));
    ///     assert_eq!(u32s.get(entity), Some(&1));
    /// });
    /// ```
    pub fn add<T: ViewAddEntity>(&mut self, storages: T, component: T::Component) -> Key {
        let key = self.generate();
        storages.add_entity(component, key.index());
        key
    }
    /// Delete an entity and all its components.
    /// Returns true if the entity was alive.
    ///
    /// [World::delete] is easier to use but will borrow and release [Entities] and [AllStorages] for each entity.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let entity1 = world.new_entity((0usize, 1u32));
    /// let entity2 = world.new_entity((2usize, 3u32));
    /// let mut entities = world.entities_mut();
    /// let mut all_storages = world.all_storages_mut();
    ///
    /// entities.delete(&mut all_storages, entity1);
    ///
    /// let (usizes, u32s) = all_storages.get_storage::<(&usize, &u32)>();
    /// assert_eq!((&usizes).get(entity1), None);
    /// assert_eq!((&u32s).get(entity1), None);
    /// assert_eq!(usizes.get(entity2), Some(&2));
    /// assert_eq!(u32s.get(entity2), Some(&3));
    /// ```
    ///
    /// [World::delete]: struct.World.html#method.delete
    /// [Entities]: struct.Entities.html
    /// [AllStorages]: struct.AllStorages.html
    pub fn delete(&mut self, storages: &mut AllStoragesViewMut, entity: Key) -> bool {
        if self.delete_key(entity) {
            storages.delete(entity.index());
            true
        } else {
            false
        }
    }
}
