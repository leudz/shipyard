use crate::ShipHashMap;
use crate::storage::{SBox, StorageId};

/// The storages registered in [`AllStorages`](super::AllStorages), split in two maps.
///
/// Both maps sit under the same registry lock and form a single namespace: a storage id is in at
/// most one of them. The canonical entities id is in neither, the entities have their own field.
///
/// Where a storage goes depends on how it was registered, not on its type:
/// - the unique APIs (`add_unique*` and the views that call them) insert in `uniques`
/// - everything else, components and custom storages, goes in `components_and_custom`,
///   including a unique storage registered through a custom path
pub(crate) struct StorageMaps {
    pub(crate) components_and_custom: ShipHashMap<StorageId, SBox>,
    pub(crate) uniques: ShipHashMap<StorageId, SBox>,
}

impl StorageMaps {
    pub(crate) fn new() -> StorageMaps {
        StorageMaps {
            components_and_custom: ShipHashMap::new(),
            uniques: ShipHashMap::new(),
        }
    }

    /// Returns the component or custom storage registered under `storage_id`.
    ///
    /// Never looks at the uniques.
    #[inline]
    pub(crate) fn component(&self, storage_id: StorageId) -> Option<&SBox> {
        self.components_and_custom.get(&storage_id)
    }

    /// Returns the unique storage registered under `storage_id`.
    ///
    /// Looks at the uniques first, then at the custom storages, where a unique registered through
    /// a custom path lives.
    #[inline]
    pub(crate) fn unique(&self, storage_id: StorageId) -> Option<&SBox> {
        self.uniques
            .get(&storage_id)
            .or_else(|| self.components_and_custom.get(&storage_id))
    }

    /// Returns the storage registered under `storage_id`, whatever its kind.
    ///
    /// Looks at the components and custom storages first, then at the uniques.
    #[inline]
    pub(crate) fn any(&self, storage_id: StorageId) -> Option<&SBox> {
        self.components_and_custom
            .get(&storage_id)
            .or_else(|| self.uniques.get(&storage_id))
    }

    /// Returns how many storages are registered, in both maps.
    pub(crate) fn len(&self) -> usize {
        self.components_and_custom.len() + self.uniques.len()
    }

    /// Iterates over every registered storage, the components and custom storages first.
    pub(crate) fn values(&self) -> impl Iterator<Item = &SBox> {
        self.components_and_custom
            .values()
            .chain(self.uniques.values())
    }

    /// Iterates mutably over every registered storage, the components and custom storages first.
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut SBox> {
        self.components_and_custom
            .values_mut()
            .chain(self.uniques.values_mut())
    }
}
