mod iterator;

pub use iterator::EntitiesIter;

use crate::add_component::AddComponent;
use crate::add_distinct_component::AddDistinctComponent;
use crate::add_entity::AddEntity;
use crate::entity_id::EntityId;
use crate::error;
use crate::memory_usage::StorageMemoryUsage;
use crate::reserve::{BulkEntityIter, BulkReserve};
use crate::storage::Storage;
use crate::tracking::TrackingTimestamp;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::iter::repeat_with;
use core::mem::size_of;

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
/// Generation -> Deletion -> Dead  
/// &nbsp; &nbsp; &nbsp; &nbsp; &nbsp;
///       ⬑----------↵
// An entity starts with a generation at 0, each removal will increase it by 1
// until genration::MAX() where the entity is considered dead.
// Removed entities form a linked list inside the vector, using their index part to point to the next.
// Removed entities are added to one end and removed from the other.
// Dead entities are simply never added to the linked list.
pub struct Entities {
    pub(crate) data: Vec<EntityId>,
    list: Option<(usize, usize)>,
    on_deletion: Option<Box<dyn FnMut(EntityId) + Send + Sync>>,
}

impl Entities {
    #[inline]
    pub(crate) fn new() -> Self {
        Entities {
            data: Vec::new(),
            list: None,
            on_deletion: None,
        }
    }
    /// Returns `true` if `entity` matches a living entity.
    #[inline]
    pub fn is_alive(&self, entity: EntityId) -> bool {
        if let Some(&self_entity) = self.data.get(entity.uindex()) {
            entity == self_entity
        } else {
            false
        }
    }
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// `Entities` is only borrowed immutably.  
    ///
    /// ### Panics
    ///
    /// - `entity` is not alive.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, EntitiesView, ViewMut, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity(());
    ///
    /// let (entities, mut u32s) = world.borrow::<(EntitiesView, ViewMut<U32>)>().unwrap();
    ///
    /// entities.add_component(entity, &mut u32s, U32(0));
    /// ```
    #[track_caller]
    #[inline]
    pub fn add_component<C, S: AddComponent<C>>(
        &self,
        entity: EntityId,
        mut storages: S,
        component: C,
    ) {
        if self.is_alive(entity) {
            storages.add_component_unchecked(entity, component);
        } else {
            panic!("{:?}", error::AddComponent::EntityIsNotAlive);
        }
    }
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// If the entity already has this component, it won't be replaced. Very useful if you want accurate modification tracking.  
    /// `Entities` is only borrowed immutably.  
    ///
    /// Returns `true` if the component was added.
    ///
    /// ### Panics
    ///
    /// - `entity` is not alive.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, EntitiesView, ViewMut, World};
    ///
    /// #[derive(Component, PartialEq)]
    /// struct U32(u32);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity(());
    ///
    /// let (entities, mut u32s) = world.borrow::<(EntitiesView, ViewMut<U32>)>().unwrap();
    ///
    /// assert!(entities.add_distinct_component(entity, &mut u32s, U32(0)));
    /// assert!(!entities.add_distinct_component(entity, &mut u32s, U32(0)));
    /// ```
    #[track_caller]
    #[inline]
    pub fn add_distinct_component<S: AddDistinctComponent>(
        &self,
        entity: EntityId,
        mut storages: S,
        component: S::Component,
    ) -> bool {
        if self.is_alive(entity) {
            storages.add_distinct_component_unchecked(entity, component)
        } else {
            panic!("{:?}", error::AddComponent::EntityIsNotAlive);
        }
    }
    pub(crate) fn generate(&mut self) -> EntityId {
        if let Some((new, ref mut old)) = self.list {
            let old_index = *old;

            if new == *old {
                self.list = None;
            } else {
                // SAFE old_index is always valid
                *old = unsafe { self.data.get_unchecked(old_index).uindex() };
            }
            // SAFE old_index is always valid
            unsafe {
                self.data
                    .get_unchecked_mut(old_index)
                    .set_index(old_index as u64);
                *self.data.get_unchecked(old_index)
            }
        } else {
            let entity_id = EntityId::new(self.data.len() as u64);
            self.data.push(entity_id);
            entity_id
        }
    }
    pub(crate) fn bulk_generate(&mut self, count: usize) -> &[EntityId] {
        self.data
            .extend((self.data.len() as u64..(self.data.len() + count) as u64).map(EntityId::new));

        &self.data[self.data.len() - count..self.data.len()]
    }
    /// Deletes an entity, returns true if the entity was alive.  
    /// If the entity has components, they will not be deleted and still be accessible using this id.
    pub fn delete_unchecked(&mut self, entity_id: EntityId) -> bool {
        if self.is_alive(entity_id) {
            // SAFE we checked for OOB
            if unsafe {
                self.data
                    .get_unchecked_mut(entity_id.uindex())
                    .bump_gen()
                    .is_ok()
            } {
                if let Some((ref mut new, _)) = self.list {
                    // SAFE new is always in bound
                    unsafe {
                        self.data
                            .get_unchecked_mut(*new)
                            .set_index(entity_id.index())
                    };
                    unsafe {
                        self.data
                            .get_unchecked_mut(entity_id.uindex())
                            .set_index(EntityId::max_index())
                    };
                    *new = entity_id.uindex();
                } else {
                    unsafe {
                        self.data
                            .get_unchecked_mut(entity_id.uindex())
                            .set_index(EntityId::max_index())
                    };
                    self.list = Some((entity_id.uindex(), entity_id.uindex()));
                }
            }

            if let Some(on_deletion) = &mut self.on_deletion {
                (on_deletion)(entity_id)
            }

            true
        } else {
            false
        }
    }
    /// Stores `component` in a new entity and returns its [`EntityId`].  
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, ViewMut, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world
    ///     .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
    ///     .unwrap();
    ///
    /// let entity = entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
    /// assert_eq!(usizes[entity], USIZE(0));
    /// assert_eq!(u32s[entity], U32(1));
    /// ```
    ///
    /// [`EntityId`]: crate::entity_id::EntityId
    #[inline]
    pub fn add_entity<T: AddEntity>(
        &mut self,
        mut storages: T,
        component: T::Component,
    ) -> EntityId {
        let entity_id = self.generate();
        AddEntity::add_entity(&mut storages, entity_id, component);
        entity_id
    }
    /// Creates multiple new entities and returns an iterator yielding the new [`EntityId`]s.  
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, ViewMut, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world
    ///     .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
    ///     .unwrap();
    ///
    /// let new_entities =
    ///     entities.bulk_add_entity((&mut u32s, &mut usizes), (10..20).map(|i| (U32(i as u32), USIZE(i))));
    /// ```
    ///
    /// [`EntityId`]: crate::entity_id::EntityId
    pub fn bulk_add_entity<T: AddEntity + BulkReserve, I: IntoIterator<Item = T::Component>>(
        &mut self,
        mut storages: T,
        component: I,
    ) -> BulkEntityIter<'_> {
        let mut iter = component.into_iter();
        let len = iter.size_hint().0;

        let entities_len = self.data.len();
        let new_entities = self.bulk_generate(len);

        storages.bulk_reserve(new_entities);
        for (component, id) in (&mut iter).zip(new_entities.iter().copied()) {
            AddEntity::add_entity(&mut storages, id, component);
        }

        // have to use two loops because of self borrow
        for (component, id) in iter.zip(repeat_with(|| self.generate())) {
            AddEntity::add_entity(&mut storages, id, component);
        }

        BulkEntityIter {
            iter: self.data[entities_len..].iter().copied(),
            slice: &self.data[entities_len..],
        }
    }
    /// Creates an iterator over all entities.
    #[inline]
    pub fn iter(&self) -> EntitiesIter<'_> {
        self.into_iter()
    }
    /// Make the given entity alive.  
    /// Does nothing if an entity with a greater generation is already at this index.  
    /// Returns `true` if the entity is successfully spawned.
    pub fn spawn(&mut self, entity: EntityId) -> bool {
        if let Some(&old_entity) = self.data.get(entity.index() as usize) {
            if self.is_alive(old_entity) {
                if old_entity.gen() <= entity.gen() {
                    self.data[entity.uindex()] = entity;

                    true
                } else {
                    false
                }
            } else if let Some((new, old)) = self.list {
                if old_entity.gen() <= entity.gen() + 1 {
                    // pop from removed list
                    if entity.uindex() == old {
                        if new == old {
                            self.list = None;
                        } else {
                            self.list = Some((new, self.data[entity.uindex()].uindex()));
                        }
                    } else {
                        let mut current_index = old;

                        while self.data[current_index].index() != entity.index()
                            && self.data[current_index].uindex() != new
                        {
                            current_index = self.data[current_index].uindex();
                        }

                        if self.data[current_index].uindex() == new {
                            self.data[current_index].set_index(EntityId::max_index());
                            self.list = Some((current_index, old));
                        } else {
                            let next_index = self.data[self.data[current_index].uindex()].index();
                            self.data[current_index].set_index(next_index);
                        }
                    }

                    self.data[entity.uindex()] = entity;

                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            let old_len = self.data.len();
            self.data.resize(entity.uindex() + 1, EntityId::new(0));

            if self.data.len() - old_len > 1 {
                // add to removed list
                if let Some((new, _)) = &mut self.list {
                    self.data[*new].set_index(old_len as u64);

                    *new = entity.uindex() - 1;
                } else {
                    self.list = Some((entity.uindex() - 1, old_len));
                }

                for (e, index) in self.data[old_len..entity.uindex() - 1]
                    .iter_mut()
                    .zip(old_len as u64 + 1..)
                {
                    e.set_index(index);
                }

                self.data[entity.uindex() - 1].set_index(EntityId::max_index());
            }

            self.data[entity.uindex()] = entity;

            true
        }
    }

    /// Sets the on entity deletion callback.
    pub fn on_deletion(&mut self, f: impl FnMut(EntityId) + Send + Sync + 'static) {
        self.on_deletion = Some(Box::new(f));
    }

    /// Remove the on entity deletion callback.
    pub fn take_on_deletion(&mut self) -> Option<Box<dyn FnMut(EntityId) + Send + Sync + 'static>> {
        self.on_deletion.take()
    }
}

impl Storage for Entities {
    fn clear(&mut self, _current: TrackingTimestamp) {
        if self.data.is_empty() {
            return;
        }

        // the first value can be anything but self.data.len() - 1
        // otherwise we would set data[len - 1].index to len - 1 and not delete it
        let mut last_alive = if self.data.len() as u64 == EntityId::max_index() {
            0
        } else {
            EntityId::max_index()
        };
        for (i, id) in self.data.iter_mut().enumerate().rev() {
            let target = last_alive;
            let id_before_bump = *id;

            if id.bump_gen().is_ok() {
                last_alive = i as u64;

                if let Some(on_deletion) = &mut self.on_deletion {
                    (on_deletion)(id_before_bump)
                }
            }

            id.set_index(target);
        }

        let begin = self
            .data
            .iter()
            .position(|id| id.gen() < EntityId::max_gen())
            .unwrap();
        let end = self
            .data
            .iter()
            .rev()
            .position(|id| id.gen() < EntityId::max_gen())
            .unwrap();
        self.list = Some((self.data.len() - end - 1, begin));
    }
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(StorageMemoryUsage::Entities {
            allocated_memory_bytes: (self.data.capacity() * size_of::<EntityId>())
                + size_of::<Entities>(),
            used_memory_bytes: (self.data.len() * size_of::<EntityId>()) + size_of::<Entities>(),
            entity_count: self.data.len(),
        })
    }
    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    fn move_component_from(
        &mut self,
        _other_all_storages: &mut crate::AllStorages,
        _from: EntityId,
        _to: EntityId,
        _current: TrackingTimestamp,
        _other_current: TrackingTimestamp,
    ) {
        // Do nothing here so this function can be used to implement both move component and move entity.
    }
}

#[test]
fn entities() {
    let mut entities = Entities::new();

    let key00 = entities.generate();
    let key10 = entities.generate();

    assert_eq!(key00.index(), 0);
    assert_eq!(key00.gen(), 0);
    assert_eq!(key10.index(), 1);
    assert_eq!(key10.gen(), 0);

    assert!(entities.delete_unchecked(key00));
    assert!(!entities.delete_unchecked(key00));
    let key01 = entities.generate();

    assert_eq!(key01.index(), 0);
    assert_eq!(key01.gen(), 1);

    assert!(entities.delete_unchecked(key10));
    assert!(entities.delete_unchecked(key01));
    let key11 = entities.generate();
    let key02 = entities.generate();

    assert_eq!(key11.index(), 1);
    assert_eq!(key11.gen(), 1);
    assert_eq!(key02.index(), 0);
    assert_eq!(key02.gen(), 2);

    let last_key = EntityId::new_from_index_and_gen(0, EntityId::max_gen());
    entities.data[0] = last_key;
    assert!(entities.delete_unchecked(last_key));
    assert_eq!(entities.list, None);
    let dead = entities.generate();
    assert_eq!(dead.index(), 2);
    assert_eq!(dead.gen(), 0);
}

#[test]
fn iterator() {
    let mut entities = Entities::new();

    entities.add_entity((), ());
    entities.add_entity((), ());
    entities.add_entity((), ());

    let mut iter = entities.iter();

    let id0 = iter.next().unwrap();
    assert_eq!(id0.index(), 0);
    assert_eq!(id0.gen(), 0);

    let id1 = iter.next().unwrap();
    assert_eq!(id1.index(), 1);
    assert_eq!(id1.gen(), 0);

    let id2 = iter.next().unwrap();
    assert_eq!(id2.index(), 2);
    assert_eq!(id2.gen(), 0);

    assert!(iter.next().is_none());

    entities.delete_unchecked(id0);
    entities.delete_unchecked(id1);
    entities.add_entity((), ());

    let mut iter = entities.iter();

    let id = iter.next().unwrap();
    assert_eq!(id.index(), 0);
    assert_eq!(id.gen(), 1);

    let id = iter.next().unwrap();
    assert_eq!(id.index(), 2);
    assert_eq!(id.gen(), 0);

    assert!(iter.next().is_none());
}
