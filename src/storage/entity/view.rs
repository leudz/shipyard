use super::add_component::AddComponent;
use super::EntityId;
use crate::error;
use crate::sparse_set::ViewAddEntity;

/// View into the entities.
pub struct EntitiesView<'a> {
    pub(super) data: &'a [EntityId],
}

impl EntitiesView<'_> {
    /// Returns true if the EntityId matches a living entity.
    pub(crate) fn is_alive(&self, entity_id: EntityId) -> bool {
        entity_id.index() < self.data.len()
            && entity_id == unsafe { *self.data.get_unchecked(entity_id.index()) }
    }
    pub fn try_add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: EntityId,
    ) -> Result<(), error::AddComponent> {
        storages.try_add_component(component, entity, &self)
    }
    pub fn add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: EntityId,
    ) {
        storages
            .try_add_component(component, entity, &self)
            .unwrap()
    }
}

/// View into the entities, this allows to add and remove entities.
pub struct EntitiesViewMut<'a> {
    pub(super) data: &'a mut Vec<EntityId>,
    pub(crate) list: &'a mut Option<(usize, usize)>,
}

impl EntitiesViewMut<'_> {
    pub(super) fn generate(&mut self) -> EntityId {
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
            let entity_id = EntityId::new(self.data.len() as u64);
            self.data.push(entity_id);
            entity_id
        }
    }
    /// Returns true if the EntityId matches a living entity.
    fn is_alive(&self, entity_id: EntityId) -> bool {
        self.as_non_mut().is_alive(entity_id)
    }
    /// Delete an entity, returns true if the entity was alive.
    ///
    /// If the entity has components, they will not be deleted and still be accessible using this id.
    pub fn delete_unchecked(&mut self, entity_id: EntityId) -> bool {
        if self.is_alive(entity_id) {
            if unsafe {
                self.data
                    .get_unchecked_mut(entity_id.index())
                    .bump_version()
                    .is_ok()
            } {
                if let Some((ref mut new, _)) = self.list {
                    unsafe {
                        self.data
                            .get_unchecked_mut(*new)
                            .set_index(entity_id.index() as u64)
                    };
                    *new = entity_id.index();
                } else {
                    *self.list = Some((entity_id.index(), entity_id.index()));
                }
            }
            true
        } else {
            false
        }
    }
    /// Stores `component` in a new entity, the `EntityId` to this entity is returned.
    ///
    /// Multiple components can be added at the same time using a tuple.
    /// # Example:
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///
    ///     assert_eq!(usizes.get(entity), Some(&0));
    ///     assert_eq!(u32s.get(entity), Some(&1));
    /// });
    /// ```
    pub fn add_entity<T: ViewAddEntity>(
        &mut self,
        storages: T,
        component: T::Component,
    ) -> EntityId {
        let entity_id = self.generate();
        storages.add_entity(component, entity_id);
        entity_id
    }
    fn as_non_mut(&self) -> EntitiesView {
        EntitiesView { data: self.data }
    }
    pub fn try_add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: EntityId,
    ) -> Result<(), error::AddComponent> {
        storages.try_add_component(component, entity, &self.as_non_mut())
    }
    pub fn add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: EntityId,
    ) {
        storages
            .try_add_component(component, entity, &self.as_non_mut())
            .unwrap()
    }
}
