use super::Key;
use crate::sparse_array::ViewAddEntity;

/// View into the entities, this allows to add and remove entities.
pub struct EntityViewMut<'a> {
    pub(super) data: &'a mut Vec<Key>,
    pub(crate) list: &'a mut Option<(usize, usize)>,
}

impl<'a> EntityViewMut<'a> {
    fn generate(&mut self) -> Key {
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
    /// Stores `component` in a new entity, the `Key` to this entity is returned.
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// Due to current restriction, `storages` and `component` have to be tuples,
    /// even for a single value. In this case use (T,).
    pub fn add<T: ViewAddEntity>(&mut self, storages: T, component: T::Component) -> Key {
        let key = self.generate();
        storages.add_entity(component, key);
        key
    }
}
