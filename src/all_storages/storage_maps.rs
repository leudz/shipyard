use crate::ShipHashMap;
use crate::storage::{SBox, Storage, StorageId};
#[cfg(test)]
use alloc::vec;
use alloc::vec::Vec;

/// The storages registered in [`AllStorages`](super::AllStorages), split in two maps.
///
/// Both maps sit under the same registry lock and form a single namespace: a storage id is in at
/// most one of them. The canonical entities id is in neither, the entities have their own field.
///
/// Where a storage goes depends on how it was registered, not on its type:
/// - the unique APIs (`add_unique*` and the views that call them) insert in `uniques`
/// - everything else, components and custom storages, goes in `components_and_custom`,
///   including a unique storage registered through a custom path
///
/// `remove_unique` only removes from `uniques`. An entry only leaves `components_and_custom` when
/// a unique takes over its id: `add_unique` replacing a foreign storage, or `clone_storages_to`
/// replacing it with a unique's clone. Both move the id to `uniques`.
pub(crate) struct StorageMaps {
    pub(crate) components_and_custom: ComponentsAndCustom,
    pub(crate) uniques: ShipHashMap<StorageId, SBox>,
}

impl StorageMaps {
    pub(crate) fn new() -> StorageMaps {
        StorageMaps {
            components_and_custom: ComponentsAndCustom::new(),
            uniques: ShipHashMap::new(),
        }
    }

    /// Returns the component or custom storage registered under `storage_id`.
    ///
    /// Never looks at the uniques.
    #[inline]
    pub(crate) fn component(&self, storage_id: StorageId) -> Option<&SBox> {
        self.components_and_custom.get(storage_id)
    }

    /// Returns the unique storage registered under `storage_id`.
    ///
    /// Looks at the uniques first, then at the custom storages, where a unique registered through
    /// a custom path lives.
    #[inline]
    pub(crate) fn unique(&self, storage_id: StorageId) -> Option<&SBox> {
        self.uniques
            .get(&storage_id)
            .or_else(|| self.components_and_custom.get(storage_id))
    }

    /// Returns the storage registered under `storage_id`, whatever its kind.
    ///
    /// Looks at the components and custom storages first, then at the uniques.
    #[inline]
    pub(crate) fn any(&self, storage_id: StorageId) -> Option<&SBox> {
        self.components_and_custom
            .get(storage_id)
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

/// The component and custom storages, each registered under its own id.
///
/// The storages sit densely in `storages`, so visiting all of them is a walk over a vector.
/// `indices` finds a storage's slot from its id.
///
/// Invariants:
/// - `indices` and `storages` have the same length
/// - each id maps to a distinct slot of `storages`, so every slot has exactly one id
/// - registering a storage only appends, a slot keeps its index while its storage is registered.
///   The one exception is [`remove`](Self::remove), which moves the last storage into the hole.
///
/// Slots hold `SBox`es, pointers to boxed storages. Either container reallocating, or a removal
/// moving a slot, moves the pointer and not the storage. So a borrow of a storage never depends on
/// where its slot is.
///
/// Only the operations `AllStorages` needs are here. Entries are only removed when a unique takes
/// over their id, see [`StorageMaps`].
pub(crate) struct ComponentsAndCustom {
    indices: ShipHashMap<StorageId, usize>,
    storages: Vec<SBox>,
}

impl ComponentsAndCustom {
    fn new() -> ComponentsAndCustom {
        ComponentsAndCustom {
            indices: ShipHashMap::new(),
            storages: Vec::new(),
        }
    }

    /// Returns the storage registered under `storage_id`.
    #[inline]
    pub(crate) fn get(&self, storage_id: StorageId) -> Option<&SBox> {
        let index = *self.indices.get(&storage_id)?;

        Some(&self.storages[index])
    }

    /// Returns the storage registered under `storage_id`.
    #[inline]
    pub(crate) fn get_mut(&mut self, storage_id: StorageId) -> Option<&mut SBox> {
        let index = *self.indices.get(&storage_id)?;

        Some(&mut self.storages[index])
    }

    /// Returns `true` when a storage is registered under `storage_id`.
    #[cfg(test)]
    pub(crate) fn contains(&self, storage_id: StorageId) -> bool {
        self.indices.contains_key(&storage_id)
    }

    /// Returns how many storages are registered.
    pub(crate) fn len(&self) -> usize {
        self.storages.len()
    }

    /// Returns the storage registered under `storage_id`, registering `new_storage()` first if there
    /// isn't one. `new_storage` only runs when the storage is missing.
    #[inline]
    pub(crate) fn get_or_insert_with(
        &mut self,
        storage_id: StorageId,
        new_storage: impl FnOnce() -> SBox,
    ) -> &mut SBox {
        if let Some(&index) = self.indices.get(&storage_id) {
            return &mut self.storages[index];
        }

        // We build the storage before touching the registry, if `new_storage` panics nothing changed.
        let storage = new_storage();
        let index = self.push(storage_id, storage);

        &mut self.storages[index]
    }

    /// Registers `storage` under `storage_id`, replacing and returning the storage already there.
    ///
    /// A replaced storage's slot is reused, `storage` takes its index.
    pub(crate) fn insert(&mut self, storage_id: StorageId, storage: SBox) -> Option<SBox> {
        if let Some(&index) = self.indices.get(&storage_id) {
            return Some(core::mem::replace(&mut self.storages[index], storage));
        }

        self.push(storage_id, storage);

        None
    }

    /// Appends `storage` under `storage_id`, which mustn't be registered, and returns its index.
    fn push(&mut self, storage_id: StorageId, storage: SBox) -> usize {
        // We reserve both containers before changing either. If an allocation panics, we haven't
        // published a slot without an id or an id without a slot. After this, neither push nor
        // insert allocates, and nothing in between runs user code.
        self.indices.reserve(1);
        self.storages.reserve(1);

        let index = self.storages.len();
        self.storages.push(storage);
        let previous = self.indices.insert(storage_id, index);
        debug_assert!(previous.is_none());

        index
    }

    /// Takes out the storage registered under `storage_id`.
    ///
    /// Only for a unique taking over the id, see [`StorageMaps`].
    /// The last storage moves into the freed slot, it's the only one whose index changes.
    pub(crate) fn remove(&mut self, storage_id: StorageId) -> Option<SBox> {
        let index = self.indices.remove(&storage_id)?;
        let last = self.storages.len() - 1;

        // `swap_remove` moves the last storage into the hole, its id has to follow it.
        // We don't keep each slot's id, so we look for it in the map. Removals are rare enough
        // that this scan is cheaper than keeping a reverse index up to date.
        if index != last {
            let moved = self
                .indices
                .values_mut()
                .find(|other| **other == last)
                .expect("every slot has an id");
            *moved = index;
        }

        Some(self.storages.swap_remove(index))
    }

    /// Iterates over every storage.
    pub(crate) fn values(&self) -> impl Iterator<Item = &SBox> {
        self.storages.iter()
    }

    /// Iterates mutably over every storage.
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut SBox> {
        self.storages.iter_mut()
    }

    /// Iterates mutably over every storage, in parallel.
    #[cfg(all(feature = "parallel", not(feature = "thread_local")))]
    pub(crate) fn par_values_mut(
        &mut self,
    ) -> impl rayon::iter::ParallelIterator<Item = &mut SBox> {
        use rayon::iter::IntoParallelRefMutIterator;

        self.storages.par_iter_mut()
    }

    /// Iterates over every storage with its id.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (StorageId, &SBox)> {
        self.indices
            .iter()
            .map(|(&storage_id, &index)| (storage_id, &self.storages[index]))
    }

    /// Collects exclusive storage references with their ids in slot order, in one allocation.
    ///
    /// See [`for_each_mut`](Self::for_each_mut) to visit storages without allocating.
    pub(crate) fn collect_storages_mut(
        &mut self,
    ) -> Vec<(StorageId, &mut (dyn Storage + 'static))> {
        // Slice iteration gives us disjoint references. Fill ids in the final vector instead of
        // allocating a separate id buffer. Every slot has exactly one id, so every placeholder
        // is overwritten before returning; the default id needn't be reserved or unique.
        let mut selected: Vec<_> = self
            .storages
            .iter_mut()
            .map(|sbox| (StorageId::default(), sbox.storage_mut()))
            .collect();

        for (&storage_id, &index) in &self.indices {
            selected[index].0 = storage_id;
        }

        selected
    }

    /// Calls `visit` on every storage with its id.
    ///
    /// Unlike [`collect_storages_mut`](Self::collect_storages_mut), only one storage is reachable
    /// at a time.
    #[inline]
    pub(crate) fn for_each_mut(&mut self, mut visit: impl FnMut(StorageId, &mut SBox)) {
        for (&storage_id, &index) in &self.indices {
            visit(storage_id, &mut self.storages[index]);
        }
    }

    /// Returns how many entries `indices` and `storages` hold before reallocating.
    #[cfg(test)]
    pub(crate) fn capacities(&self) -> (usize, usize) {
        (self.indices.capacity(), self.storages.capacity())
    }

    /// Panics unless the invariants hold: same lengths, and each id has a distinct, valid slot.
    #[cfg(test)]
    pub(crate) fn assert_consistent(&self) {
        assert_eq!(self.indices.len(), self.storages.len());

        let mut taken = vec![false; self.storages.len()];
        for &index in self.indices.values() {
            assert!(!taken[index], "slot {index} has two ids");
            taken[index] = true;
        }
    }
}
