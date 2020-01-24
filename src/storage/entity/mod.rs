mod add_component;
mod entity_id;

use crate::error;
use crate::sparse_set::ViewAddEntity;
use crate::unknown_storage::UnknownStorage;
use add_component::AddComponent;
pub use entity_id::EntityId;
use std::any::TypeId;

/// Type used to borrow `Entities` mutably.
pub struct EntitiesMut;

/// Entities holds the EntityIds to all entities: living, removed and dead.
///
/// A living entity is an entity currently present, with or without component.
///
/// Removed and dead entities don't have any component.
///
/// The big difference is that removed ones can become alive again.
///
/// The life cycle of an entity looks like this:
///
/// Generation -> Deletion -> Dead\
///           ⬑----------↵
// An entity starts with a generation at 0, each removal will increase it by 1
// until version::MAX() where the entity is considered dead.
// Removed entities form a linked list inside the vector, using their index part to point to the next.
// Removed entities are added to one end and removed from the other.
// Dead entities are simply never added to the linked list.
pub struct Entities {
    data: Vec<EntityId>,
    list: Option<(usize, usize)>,
}

impl Default for Entities {
    fn default() -> Self {
        Entities {
            data: Vec::new(),
            list: None,
        }
    }
}

impl Entities {
    pub(super) fn delete(&mut self, entity: EntityId) -> bool {
        self.delete_unchecked(entity)
    }
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
    pub(super) fn generate(&mut self) -> EntityId {
        let index = self.list.map(|(_, old)| old);
        if let Some((new, ref mut old)) = self.list {
            if new == *old {
                self.list = None;
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
                    self.list = Some((entity_id.index(), entity_id.index()));
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
    /// let world = World::new();
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
}

impl UnknownStorage for Entities {
    fn delete(&mut self, _entity: EntityId, _: &mut Vec<TypeId>) {}
    fn unpack(&mut self, _entity: EntityId) {}
}

#[test]
fn entities() {
    use std::num::NonZeroU64;

    let mut entities = Entities::default();

    let key00 = entities.generate();
    let key10 = entities.generate();

    assert_eq!(key00.index(), 0);
    assert_eq!(key00.version(), 0);
    assert_eq!(key10.index(), 1);
    assert_eq!(key10.version(), 0);

    assert!(entities.delete_unchecked(key00));
    assert!(!entities.delete_unchecked(key00));
    let key01 = entities.generate();

    assert_eq!(key01.index(), 0);
    assert_eq!(key01.version(), 1);

    assert!(entities.delete_unchecked(key10));
    assert!(entities.delete_unchecked(key01));
    let key11 = entities.generate();
    let key02 = entities.generate();

    assert_eq!(key11.index(), 1);
    assert_eq!(key11.version(), 1);
    assert_eq!(key02.index(), 0);
    assert_eq!(key02.version(), 2);

    let last_key = EntityId(NonZeroU64::new(!(!0 >> 15) + 1).unwrap());
    entities.data[0] = last_key;
    assert!(entities.delete_unchecked(last_key));
    assert_eq!(entities.list, None);
    let dead = entities.generate();
    assert_eq!(dead.index(), 2);
    assert_eq!(dead.version(), 0);
}
