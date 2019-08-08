use super::Key;
use crate::component_storage::AllStoragesViewMut;
use crate::sparse_array::ViewAddEntity;

/// View into the entities, this allows to add and remove entities.
pub struct EntityViewMut<'a> {
    pub(super) data: &'a mut Vec<Key>,
    pub(crate) list: &'a mut Option<(usize, usize)>,
}

impl<'a> EntityViewMut<'a> {
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
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// Due to current restriction, `storages` and `component` have to be tuples,
    /// even for a single value. In this case use (T,).
    pub fn add<T: ViewAddEntity>(&mut self, storages: T, component: T::Component) -> Key {
        let key = self.generate();
        storages.add_entity(component, key.index());
        key
    }
    /// Delete an entity and all its components.
    /// Returns true if the entity was alive.
    pub fn delete(&mut self, storages: &mut AllStoragesViewMut, entity: Key) -> bool {
        if self.delete_key(entity) {
            storages.delete(entity.index());
            true
        } else {
            false
        }
    }
}
