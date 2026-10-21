//! Moves components to the groups they belong to, in two phases.
//!
//! 1. `analysis::analyze` decides what changes. It reads every grouped storage as it is and only
//!    looks at what can have changed: the unevaluated regions and the untrusted suffixes.
//!    It returns early for a clean world, and otherwise counts, per group and per storage,
//!    what leaves and joins each range.
//! 2. `layout::build` moves entity ids only, to their final rows. Components and timestamps don't
//!    move, and sparse arrays are frozen: each entry still holds the row its entity's component
//!    started in, see `original_row`. `Groups` also stay as regroup found them, apart from new
//!    empty unions, so a frozen row read against them tells where a component was.
//!    The members of each group copy the order of one of them, its reference.
//! 3. `layout::finish` repairs each storage once all ids are final, since building reads other
//!    storages' frozen rows. Components and timestamps follow their ids along the permutation's
//!    cycles over the rows whose id changed, and sparse entries point at the new rows.
//!    Then the final ranges are published, fully trusted, with nothing left unevaluated.

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

// Layout works on every grouped storage at once, so it gets to them through raw pointers.
// This is sound because:
// - `&mut AllStorages` guarantees nobody else borrows any storage,
// - group members are distinct storages, `WorldBuilder::with_group` rejects groups listing a storage twice,
// - storages can't be removed while we hold the map mutably, so the pointers stay valid,
// - getting a fresh pointer to a storage invalidates the previous pointers and references to that
//   storage only, the other storages live in their own allocations,
// - layout makes exactly one exclusive reference per storage, each lending its ids apart from its
//   sparse array and groups. It holds the map mutably while its references live,
//   so no new pointer can be made meanwhile.

impl AllStorages {
    /// Moves components to the groups they belong to, to speed up iteration.
    ///
    /// Groups are declared with [`WorldBuilder::with_group`](crate::world::WorldBuilder::with_group).\
    /// Components only move to their group during a regroup.\
    /// Iteration stays correct between regroups, but adding or removing components slows it down.
    pub fn regroup(&mut self) {
        let storages = self.storages.get_mut();

        // Analysis decides what changes without moving anything, a clean world stops here.
        let mut selected = Selected::all(storages);
        let Some(plan) = analysis::analyze(&mut selected) else {
            return;
        };
        let storage_ids = selected.ids().to_vec();
        drop(selected);

        // We only move ids first, components and timestamps follow when each storage is finished.
        // SAFE: `plan` was just made from these storages, and `finish` completes every one of
        // them before anything else uses them.
        unsafe {
            let layouts = layout::build(storages, &storage_ids, &plan);
            layout::finish(storages, &layouts);
        }
    }
}

/// The storages a regroup works on, the only ones it can reach.
///
/// It holds an exclusive reference to each of them, sorted by id.
pub(super) struct Selected<'a> {
    /// Sorted and distinct.
    ids: Vec<StorageId>,
    /// The storage of each id, at the same position.
    storages: Vec<&'a mut (dyn Storage + 'static)>,
}

impl<'a> Selected<'a> {
    /// Selects every storage of `storages`.
    fn all(storages: &'a mut ShipHashMap<StorageId, SBox>) -> Selected<'a> {
        let mut selected: Vec<(StorageId, &'a mut (dyn Storage + 'static))> = storages
            .iter_mut()
            .map(|(&storage_id, sbox)| (storage_id, storage_of(sbox)))
            .collect();

        // The map's iteration order can change between runs. Sorting keeps the layout,
        // and therefore grouped iteration order, reproducible within a build.
        // Storage ids don't promise the same order across compiler builds.
        selected.sort_unstable_by_key(|&(storage_id, _)| storage_id);
        let (ids, storages) = selected.into_iter().unzip();

        Selected { ids, storages }
    }

    /// Returns the ids of the selected storages, sorted.
    pub(super) fn ids(&self) -> &[StorageId] {
        &self.ids
    }

    /// Returns the selected storage with id `storage_id`.
    ///
    /// ### Panics
    ///
    /// - The storage isn't selected.
    #[track_caller]
    pub(super) fn get(&self, storage_id: StorageId) -> &(dyn Storage + 'static) {
        &*self.storages[self.position(storage_id)]
    }

    /// Returns the selected storage with id `storage_id`.
    ///
    /// ### Panics
    ///
    /// - The storage isn't selected.
    #[track_caller]
    pub(super) fn get_mut(&mut self, storage_id: StorageId) -> &mut (dyn Storage + 'static) {
        let position = self.position(storage_id);

        &mut *self.storages[position]
    }

    /// Returns the position of `storage_id` in `ids`.
    ///
    /// ### Panics
    ///
    /// - The storage isn't selected.
    #[track_caller]
    fn position(&self, storage_id: StorageId) -> usize {
        self.ids
            .binary_search(&storage_id)
            .expect("regroup can only reach the storages it selected")
    }
}

/// Returns the storage `sbox` holds.
fn storage_of(sbox: &mut SBox) -> &mut (dyn Storage + 'static) {
    // SAFE: `sbox.0` comes from a `Box` that `sbox` owns, it lives as long as `sbox`.
    // We borrow `sbox` mutably for as long as the reference lives, no other reference
    // to this storage can exist.
    unsafe { &mut *sbox.0 }.get_mut()
}

/// Returns a pointer to the storage with id `storage_id`.
///
/// Calling this again for the same storage invalidates its previous pointer.
fn storage_ptr(storages: &mut ShipHashMap<StorageId, SBox>, storage_id: StorageId) -> StoragePtr {
    let sbox = storages
        .get_mut(&storage_id)
        .expect("group members cannot be deleted");

    storage_of(sbox)
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
