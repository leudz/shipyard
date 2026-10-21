mod analysis;
mod layout;
#[cfg(test)]
mod tests;

use crate::ShipHashMap;
use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseArray;
use crate::storage::{SBox, Storage, StorageId};
use alloc::vec::Vec;

/// A pointer to a storage.
///
/// The `'static` is spelled out, inside a reference it would default to the reference's lifetime.
type StoragePtr = *mut (dyn Storage + 'static);

// Regroup works on every grouped storage at once, so it gets to them through raw pointers.
// This is sound because:
// - `&mut AllStorages` guarantees nobody else borrows any storage,
// - group members are distinct storages, `AllStorages::group` rejects groups listing a storage twice,
// - storages can't be removed while we hold the map mutably, so the pointers stay valid,
// - getting a fresh pointer to a storage invalidates the previous pointers and references to that
//   storage only, the other storages live in their own allocations,
// - analysis only makes shared references, and layout makes exactly one exclusive reference per
//   storage, each lending its ids apart from its sparse array and groups. Both hold the map
//   mutably while their references live, so no new pointer can be made meanwhile.

impl AllStorages {
    /// Moves components to the groups they belong to, to speed up iteration.
    ///
    /// Components only move to their group during a regroup, explicit or from declaring a new group.\
    /// Iteration stays correct between regroups, but adding or removing components slows it down.
    pub fn regroup(&mut self) {
        let storages = self.storages.get_mut();
        let storage_ids = sorted_storage_ids(storages);

        // Analysis decides what changes without moving anything, a clean world stops here.
        let Some(plan) = analysis::analyze(storages, &storage_ids) else {
            return;
        };

        // We only move ids first, components and timestamps follow when each storage is finished.
        // SAFE: `plan` was just made from these storages, and `finish` completes every one of
        // them before anything else uses them.
        unsafe {
            let layouts = layout::build(storages, &storage_ids, &plan);
            layout::finish(storages, &layouts);
        }
    }
}

/// Returns the id of every storage, sorted.
///
/// The map's iteration order can change between runs.\
/// Sorting keeps the layout, and therefore grouped iteration order, reproducible within a build.\
/// Storage IDs do not promise the same order across compiler builds.
fn sorted_storage_ids(storages: &ShipHashMap<StorageId, SBox>) -> Vec<StorageId> {
    let mut storage_ids: Vec<StorageId> = storages.keys().copied().collect();
    storage_ids.sort_unstable();

    storage_ids
}

/// Returns a pointer to the storage with id `storage_id`.
///
/// Calling this again for the same storage invalidates its previous pointer.
fn storage_ptr(storages: &mut ShipHashMap<StorageId, SBox>, storage_id: StorageId) -> StoragePtr {
    let sbox = storages
        .get_mut(&storage_id)
        .expect("group members cannot be deleted");

    // SAFE: `sbox.0` comes from a `Box` that lives as long as its entry in the map,
    // and we borrow the map mutably so no other reference to this storage can exist.
    unsafe { &mut *sbox.0 }.get_mut()
}

/// Returns the storage with id `storage_id`.
///
/// Unlike `storage_ptr`, the borrow checker makes sure this is the only reference to any storage
/// while it lives.
fn storage_mut(
    storages: &mut ShipHashMap<StorageId, SBox>,
    storage_id: StorageId,
) -> &mut (dyn Storage + 'static) {
    let storage = storage_ptr(storages, storage_id);

    // SAFE: `storage` was just created, and the returned reference borrows `storages` mutably,
    // so nothing else can get to a storage until it's dropped.
    unsafe { &mut *storage }
}

/// Returns the row `entity`'s component had when regroup started in the storage with the sparse
/// array `sparse`, `None` when `entity` doesn't own a component there.
///
/// The generation has to match, a component of a deleted entity reusing the same index doesn't count.
///
/// Regroup orders ids before moving any component. Until it repairs the rows, sparse arrays are
/// frozen, so this is the component's starting row. Regroup also leaves every storage's `Groups`
/// as it found them until it publishes the final layout, apart from attaching new empty union ranges
/// that hold no row, so their regions and rings still tell where that row was.
fn original_row(sparse: &SparseArray, entity: EntityId) -> Option<usize> {
    let stored = sparse.get(entity)?;

    (stored.generation() == entity.generation()).then(|| stored.uindex())
}
