mod add_component;
mod entity_id;
mod view;

use crate::unknown_storage::UnknownStorage;
pub use entity_id::EntityId;
use std::any::TypeId;
pub use view::{EntitiesView, EntitiesViewMut};

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
    pub(crate) fn view(&self) -> EntitiesView {
        EntitiesView { data: &self.data }
    }
    pub(crate) fn view_mut(&mut self) -> EntitiesViewMut {
        EntitiesViewMut {
            data: &mut self.data,
            list: &mut self.list,
        }
    }
    pub(super) fn delete(&mut self, entity: EntityId) -> bool {
        self.view_mut().delete(entity)
    }
}

impl UnknownStorage for Entities {
    fn delete(&mut self, _entity: EntityId) -> &[TypeId] {
        &[]
    }
    fn unpack(&mut self, _entity: EntityId) {}
}

#[test]
fn entities() {
    use std::num::NonZeroU64;

    let mut entities = Entities::default();

    let key00 = entities.view_mut().generate();
    let key10 = entities.view_mut().generate();

    assert_eq!(key00.index(), 0);
    assert_eq!(key00.version(), 0);
    assert_eq!(key10.index(), 1);
    assert_eq!(key10.version(), 0);

    assert!(entities.view_mut().delete(key00));
    assert!(!entities.view_mut().delete(key00));
    let key01 = entities.view_mut().generate();

    assert_eq!(key01.index(), 0);
    assert_eq!(key01.version(), 1);

    assert!(entities.view_mut().delete(key10));
    assert!(entities.view_mut().delete(key01));
    let key11 = entities.view_mut().generate();
    let key02 = entities.view_mut().generate();

    assert_eq!(key11.index(), 1);
    assert_eq!(key11.version(), 1);
    assert_eq!(key02.index(), 0);
    assert_eq!(key02.version(), 2);

    let last_key = EntityId(NonZeroU64::new(!(!0 >> 15) + 1).unwrap());
    entities.data[0] = last_key;
    assert!(entities.view_mut().delete(last_key));
    assert_eq!(entities.list, None);
    let dead = entities.view_mut().generate();
    assert_eq!(dead.index(), 2);
    assert_eq!(dead.version(), 0);
}
