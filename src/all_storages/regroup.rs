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
use crate::all_storages::{AllStorages, ConnectedSet, CustomStorageAccess, GroupMember};
use crate::atomic_refcell::ARefMut;
use crate::entity_id::EntityId;
use crate::error;
use crate::sparse_set::SparseArray;
use crate::storage::{SBox, Storage, StorageId};
use alloc::vec::Vec;

impl AllStorages {
    /// Moves components to the groups they belong to, to speed up iteration.
    ///
    /// Groups are declared with [`WorldBuilder::with_group`](crate::world::WorldBuilder::with_group).\
    /// Components only move to their group during a regroup.\
    /// Iteration stays correct between regroups, but adding or removing components slows it down.
    pub fn regroup(&mut self) {
        regroup_selected(&mut Selected::all(self.storages.get_mut()));
    }

    /// Regroups the [connected set] holding `T`'s storage, leaving the rest of the world alone.
    ///
    /// Every storage of the set is borrowed exclusively until the regroup ends, the other storages
    /// can stay borrowed meanwhile.\
    /// `T` can be any member of the set, all of them give the same result.
    ///
    /// ### Errors
    ///
    /// - [`NotGrouped`] when `T`'s storage isn't part of any group.
    /// - [`Storage`] when a storage of the set is already borrowed, or is `!Send` and this isn't
    ///   the world's thread. It can be a member other than `T`. Nothing moves in that case.
    ///
    /// [connected set]: crate::world::WorldBuilder::with_group
    /// [`NotGrouped`]: error::RegroupSet::NotGrouped
    /// [`Storage`]: error::RegroupSet::Storage
    pub fn regroup_set<T: GroupMember>(&self) -> Result<(), error::RegroupSet> {
        let storage_id = T::storage_id();

        let Some(set) = self.grouping.set_of(storage_id) else {
            return Err(error::RegroupSet::NotGrouped {
                name: T::storage_name(),
                id: storage_id,
            });
        };

        self.regroup_connected_set(set)
            .map_err(error::RegroupSet::Storage)
    }

    /// Regroups the storages of `set`, borrowing only them.
    ///
    /// Every member is borrowed exclusively, in order, before anything moves. When a borrow fails,
    /// the ones already taken are released and nothing changes.
    pub(crate) fn regroup_connected_set(
        &self,
        set: &ConnectedSet,
    ) -> Result<(), error::GetStorage> {
        // Dropping `guards` on an early return releases the borrows already taken.
        let mut guards = Vec::with_capacity(set.members.len());
        for &storage_id in &set.members {
            match self.custom_storage_mut_by_id(storage_id) {
                Ok(guard) => guards.push(guard),
                Err(mut error) => {
                    // Borrowing by id doesn't know the storage's type, so we take its name
                    // from the group declaration.
                    if let error::GetStorage::StorageBorrow { name, .. }
                    | error::GetStorage::MissingStorage { name, .. } = &mut error
                    {
                        *name = self
                            .grouping
                            .member(storage_id)
                            .map(|member| member.storage_name);
                    }

                    return Err(error);
                }
            }
        }

        regroup_selected(&mut Selected::guarded(set, &mut guards));

        Ok(())
    }
}

/// Moves the components of the `selected` storages to the groups they belong to.
fn regroup_selected(selected: &mut Selected<'_>) {
    // Analysis decides what changes without moving anything, a clean set stops here.
    let Some(plan) = analysis::analyze(selected) else {
        return;
    };

    // We only move ids first, components and timestamps follow when each storage is finished.
    // SAFE: `plan` was just made from these storages, and `finish` completes every one of
    // them before anything else uses them.
    unsafe {
        let layouts = layout::build(selected, &plan);
        layout::finish(selected, &layouts);
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

    /// Selects the members of `set`, `guards` borrowing each of them in member order.
    ///
    /// A connected set's members are sorted and distinct, as `Selected` needs.
    fn guarded(
        set: &ConnectedSet,
        guards: &'a mut [ARefMut<'_, &mut (dyn Storage + 'static)>],
    ) -> Selected<'a> {
        Selected {
            ids: set.members.clone(),
            storages: guards.iter_mut().map(|guard| &mut **guard).collect(),
        }
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

    /// Returns every selected storage with its id, sorted by id.
    pub(super) fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (StorageId, &mut (dyn Storage + 'static))> {
        self.ids
            .iter()
            .copied()
            .zip(self.storages.iter_mut().map(|storage| &mut **storage))
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
