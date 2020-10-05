mod add_component;
mod entity_id;
mod iterator;

pub use entity_id::EntityId;
pub use iterator::EntitiesIter;

// #[cfg(feature = "serde1")]
// use crate::atomic_refcell::AtomicRefCell;
use crate::error;
// #[cfg(feature = "serde1")]
// use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig};
use crate::sparse_set::ViewAddEntity;
// #[cfg(feature = "serde1")]
// use crate::storage::Storage;
use crate::type_id::TypeId;
use crate::unknown_storage::UnknownStorage;
use add_component::AddComponent;
// #[cfg(feature = "serde1")]
// use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::any::Any;
// #[cfg(feature = "serde1")]
// use hashbrown::HashMap;

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
    data: Vec<EntityId>,
    list: Option<(usize, usize)>,
}

impl Entities {
    pub(crate) fn new() -> Self {
        Entities {
            data: Vec::new(),
            list: None,
        }
    }
    pub(super) fn delete(&mut self, entity: EntityId) -> bool {
        self.delete_unchecked(entity)
    }
    /// Returns true if `entity` matches a living entity.
    pub fn is_alive(&self, entity: EntityId) -> bool {
        // SAFE we're in bound
        entity.uindex() < self.data.len()
            && entity == unsafe { *self.data.get_unchecked(entity.uindex()) }
    }
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// `Entities` is only borrowed immutably.
    ///
    /// ### Example
    /// ```
    /// # use shipyard::*;
    /// use shipyard::{World, EntitiesViewMut, EntitiesView, ViewMut};
    ///
    /// let world = World::new();
    ///
    /// let entity = world.borrow::<EntitiesViewMut>().add_entity((), ());
    ///
    /// world.run(|entities: EntitiesView, mut u32s: ViewMut<u32>| {
    ///     entities.try_add_component(&mut u32s, 0, entity).unwrap();
    /// });
    /// ```
    pub fn try_add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: EntityId,
    ) -> Result<(), error::AddComponent> {
        storages.try_add_component(component, entity, &self)
    }
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// `Entities` is only borrowed immutably.  
    /// Unwraps errors.
    ///
    /// ### Example
    /// ```
    /// # use shipyard::*;
    /// use shipyard::{World, EntitiesViewMut, EntitiesView, ViewMut};
    ///
    /// let world = World::new();
    ///
    /// let entity = world.borrow::<EntitiesViewMut>().add_entity((), ());
    ///
    /// world.run(|entities: EntitiesView, mut u32s: ViewMut<u32>| {
    ///     entities.add_component(&mut u32s, 0, entity);
    /// });
    /// ```
    #[track_caller]
    pub fn add_component<C, S: AddComponent<C>>(
        &self,
        storages: S,
        component: C,
        entity: EntityId,
    ) {
        match storages.try_add_component(component, entity, &self) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
    pub(super) fn generate(&mut self) -> EntityId {
        let index = self.list.map(|(_, old)| old);
        if let Some((new, ref mut old)) = self.list {
            if new == *old {
                self.list = None;
            } else {
                // SAFE old is always valid
                *old = unsafe { self.data.get_unchecked(*old).uindex() };
            }
        }
        if let Some(index) = index {
            // SAFE index is always in bound
            unsafe { self.data.get_unchecked_mut(index).set_index(index as u64) };
            unsafe { *self.data.get_unchecked(index) }
        } else {
            let entity_id = EntityId::new(self.data.len() as u64);
            self.data.push(entity_id);
            entity_id
        }
    }
    /// Delete an entity, returns true if the entity was alive.  
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
            true
        } else {
            false
        }
    }
    /// Stores `component` in a new entity, the `EntityId` to this entity is returned.  
    /// Multiple components can be added at the same time using a tuple.
    /// ### Example:
    /// ```
    /// use shipyard::{EntitiesViewMut, Get, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         let entity = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///         assert_eq!(usizes.get(entity), Ok(&0));
    ///         assert_eq!(u32s.get(entity), Ok(&1));
    ///     },
    /// );
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
    pub fn iter(&self) -> EntitiesIter<'_> {
        self.into_iter()
    }
}

impl UnknownStorage for Entities {
    fn delete(&mut self, _entity: EntityId, _: &mut Vec<TypeId>) {}
    fn clear(&mut self) {
        if self.data.is_empty() {
            return;
        }
        let mut last_alive = self.data.len() as u64 - 1;
        for (i, id) in self.data.iter_mut().enumerate().rev() {
            let target = last_alive;

            if id.bump_gen().is_ok() {
                last_alive = i as u64;
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
    fn unpack(&mut self, _entity: EntityId) {}
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
    // #[cfg(feature = "serde1")]
    // fn should_serialize(&self, ser_config: GlobalSerConfig) -> bool {
    //     ser_config.with_entities
    // }
    // #[cfg(feature = "serde1")]
    // fn serialize(
    //     &self,
    //     _: GlobalSerConfig,
    //     serializer: &mut dyn crate::erased_serde::Serializer,
    // ) -> crate::erased_serde::Result<crate::erased_serde::Ok> {
    //     crate::erased_serde::Serialize::erased_serialize(self, serializer)
    // }
    // #[cfg(feature = "serde1")]
    // fn serialize_identifier(&self) -> Cow<'static, str> {
    //     // use @ to avoid conflict
    //     "Entities @@@".into()
    // }
    // #[cfg(feature = "serde1")]
    // #[allow(unused)]
    // fn deserialize(
    //     &self,
    // ) -> Option<
    //     fn(
    //         GlobalDeConfig,
    //         &HashMap<EntityId, EntityId>,
    //         &mut dyn crate::erased_serde::Deserializer<'_>,
    //     ) -> Result<Storage, crate::erased_serde::Error>,
    // > {
    //     Some(
    //         |de_config: GlobalDeConfig,
    //          entities_map: &HashMap<EntityId, EntityId>,
    //          deserializer: &mut dyn crate::erased_serde::Deserializer<'_>| {
    //             #[cfg(feature = "std")]
    //             {
    //                 Ok(Storage(Box::new(AtomicRefCell::new(
    //                     <Entities as serde::Deserialize>::deserialize(deserializer)?,
    //                     None,
    //                     true,
    //                 ))))
    //             }
    //             #[cfg(not(feature = "std"))]
    //             {
    //                 Ok(Storage(Box::new(AtomicRefCell::new(
    //                     <Entities as serde::Deserialize>::deserialize(deserializer)?,
    //                 ))))
    //             }
    //         },
    //     )
    // }
}

// #[cfg(feature = "serde1")]
// impl serde::Serialize for Entities {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         use serde::ser::SerializeStruct;

//         let mut state = serializer.serialize_struct("Entities", 2)?;

//         state.serialize_field("data", &self.data)?;
//         state.serialize_field("list", &self.list)?;
//         state.end()
//     }
// }

// #[cfg(feature = "serde1")]
// impl<'de> serde::Deserialize<'de> for Entities {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         const FIELDS: &'static [&'static str] = &["data", "list"];

//         enum Field {
//             Data,
//             List,
//         }

//         struct FieldVisitor;
//         impl<'de> serde::de::Visitor<'de> for FieldVisitor {
//             type Value = Field;
//             fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//                 formatter.write_str("field identifier")
//             }
//             fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
//             where
//                 E: serde::de::Error,
//             {
//                 match value {
//                     0u64 => Ok(Field::Data),
//                     1u64 => Ok(Field::List),
//                     _ => Err(serde::de::Error::invalid_value(
//                         serde::de::Unexpected::Unsigned(value),
//                         &"field index 0 <= i < 2",
//                     )),
//                 }
//             }
//             fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
//             where
//                 E: serde::de::Error,
//             {
//                 match value {
//                     "data" => Ok(Field::Data),
//                     "list" => Ok(Field::List),
//                     _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
//                 }
//             }
//             fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
//             where
//                 E: serde::de::Error,
//             {
//                 match value {
//                     b"data" => Ok(Field::Data),
//                     b"list" => Ok(Field::List),
//                     _ => Err(serde::de::Error::invalid_value(
//                         serde::de::Unexpected::Bytes(value),
//                         &"field bytes `data` or `list`",
//                     )),
//                 }
//             }
//         }
//         impl<'de> serde::Deserialize<'de> for Field {
//             #[inline]
//             fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//             where
//                 D: serde::Deserializer<'de>,
//             {
//                 deserializer.deserialize_identifier(FieldVisitor)
//             }
//         }

//         struct Visitor;
//         impl<'de> serde::de::Visitor<'de> for Visitor {
//             type Value = Entities;
//             fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//                 formatter.write_str("struct Entities")
//             }
//             #[inline]
//             fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//             where
//                 A: serde::de::SeqAccess<'de>,
//             {
//                 let data = seq.next_element::<Vec<EntityId>>()?.ok_or_else(|| {
//                     serde::de::Error::invalid_length(0, &"struct Entities with 2 elements")
//                 })?;
//                 let list = seq
//                     .next_element::<Option<(usize, usize)>>()?
//                     .ok_or_else(|| {
//                         serde::de::Error::invalid_length(1, &"struct Entities with 2 elements")
//                     })?;
//                 Ok(Entities { data, list })
//             }
//             #[inline]
//             fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//             where
//                 A: serde::de::MapAccess<'de>,
//             {
//                 let mut data: Option<Vec<EntityId>> = None;
//                 let mut list: Option<Option<(usize, usize)>> = None;
//                 while let Some(key) = map.next_key::<Field>()? {
//                     match key {
//                         Field::Data => {
//                             if data.is_some() {
//                                 return Err(serde::de::Error::duplicate_field("data"));
//                             }
//                             data = Some(map.next_value::<Vec<EntityId>>()?);
//                         }
//                         Field::List => {
//                             if list.is_some() {
//                                 return Err(serde::de::Error::duplicate_field("list"));
//                             }
//                             list = Some(map.next_value::<Option<(usize, usize)>>()?);
//                         }
//                     }
//                 }
//                 let data = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;
//                 let list = list.ok_or_else(|| serde::de::Error::missing_field("list"))?;

//                 Ok(Entities { data, list })
//             }
//         }

//         deserializer.deserialize_struct("Entities", FIELDS, Visitor)
//     }
// }

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

    let last_key = EntityId::new_from_parts(0, EntityId::max_gen() as u16 - 1, 0);
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

    entities.delete(id0);
    entities.delete(id1);
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
