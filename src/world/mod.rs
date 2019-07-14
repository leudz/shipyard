mod hasher;

use crate::atomic_refcell::AtomicRefCell;
use crate::component_storage::ComponentStorage;
use crate::entity::Entities;
use hasher::TypeIdHasher;
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// `World` holds all components and keeps track of entities and what they own.
pub struct World {
    entities: AtomicRefCell<Entities>,
    components: AtomicRefCell<HashMap<TypeId, ComponentStorage, BuildHasherDefault<TypeIdHasher>>>,
}

impl Default for World {
    fn default() -> Self {
        World {
            entities: AtomicRefCell::new(Default::default()),
            components: AtomicRefCell::new(Default::default()),
        }
    }
}
