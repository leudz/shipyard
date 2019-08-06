use super::{hasher::TypeIdHasher, ComponentStorage};
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

/// View of all component storages.
/// Let you remove entities.
pub struct AllStoragesViewMut<'a> {
    pub(super) data: &'a mut HashMap<TypeId, ComponentStorage, BuildHasherDefault<TypeIdHasher>>,
}
