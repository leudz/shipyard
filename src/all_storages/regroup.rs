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

use crate::all_storages::{AllStorages, ComponentsAndCustom, ConnectedSet, GroupMember};
use crate::atomic_refcell::ARefMut;
use crate::entity_id::EntityId;
use crate::error;
use crate::sparse_set::SparseArray;
use crate::storage::{Storage, StorageId};
use alloc::vec::Vec;

impl AllStorages {
    /// Moves components to the groups they belong to, to speed up iteration.
    ///
    /// Groups are declared with [`WorldBuilder::with_group`](crate::world::WorldBuilder::with_group).\
    /// Components only move to their group during a regroup.\
    /// Iteration stays correct between regroups, but adding or removing components slows it down.
    ///
    /// Workloads added to the world regroup storages they use automatically.\
    /// This is for the rest, like iterating in [`AllStorages::run`].
    pub fn regroup(&mut self) {
        regroup_selected(&mut Selected::all(
            &mut self.storages.get_mut().components_and_custom,
        ));
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
            .map(|_| ())
            .map_err(error::RegroupSet::Storage)
    }

    /// Regroups the storages of `set`, borrowing only them.
    ///
    /// Every member is borrowed exclusively, in order, before anything moves. When a borrow fails,
    /// the ones already taken are released and nothing changes.
    ///
    /// Returns `false` when the set was already clean, nothing had to move.
    pub(crate) fn regroup_connected_set(
        &self,
        set: &ConnectedSet,
    ) -> Result<bool, error::GetStorage> {
        let mut guards = self.borrow_set(set)?;
        let regrouped = regroup_selected(&mut Selected::guarded(set, &mut guards));

        Ok(regrouped)
    }

    /// Returns `true` when regrouping `set` would have nothing to do.
    ///
    /// Borrows the set like [`regroup_connected_set`](AllStorages::regroup_connected_set),
    /// only for the time of the check.
    #[cfg(feature = "parallel")]
    pub(crate) fn is_set_clean(&self, set: &ConnectedSet) -> Result<bool, error::GetStorage> {
        let mut guards = self.borrow_set(set)?;

        Ok(analysis::is_clean(&Selected::guarded(set, &mut guards)))
    }

    /// Borrows every member of `set` exclusively, in order.
    ///
    /// When a borrow fails, the ones already taken are released.
    fn borrow_set(
        &self,
        set: &ConnectedSet,
    ) -> Result<Vec<ARefMut<'_, &mut (dyn Storage + 'static)>>, error::GetStorage> {
        // Dropping `guards` on an early return releases the borrows already taken.
        let mut guards = Vec::with_capacity(set.members.len());
        for &storage_id in &set.members {
            match self.component_storage_mut_by_id(storage_id) {
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

        Ok(guards)
    }
}

/// Moves the components of the `selected` storages to the groups they belong to.
///
/// Returns `false` when they were already clean, nothing had to move.
fn regroup_selected(selected: &mut Selected<'_>) -> bool {
    // Analysis decides what changes without moving anything, a clean set stops here.
    let Some(plan) = analysis::analyze(selected) else {
        return false;
    };

    // We only move ids first, components and timestamps follow when each storage is finished.
    // SAFE: `plan` was just made from these storages, and `finish` completes every one of
    // them before anything else uses them.
    unsafe {
        let layouts = layout::build(selected, &plan);
        layout::finish(selected, &layouts);
    }

    true
}

/// The storages a regroup works on, the only ones it can reach.
///
/// It holds an exclusive reference to each of them, sorted by id.
pub(super) struct Selected<'a> {
    /// Each storage with its id, sorted by id, ids distinct.
    ///
    /// Every regroup builds one, even a clean one, so we keep it to a single allocation.
    storages: Vec<(StorageId, &'a mut (dyn Storage + 'static))>,
}

impl<'a> Selected<'a> {
    /// Selects every storage of `storages`.
    fn all(storages: &'a mut ComponentsAndCustom) -> Selected<'a> {
        let mut storages = storages.collect_storages_mut();

        // Slot order depends on registration and takeovers. Sorting keeps the layout,
        // and therefore grouped iteration order, reproducible within a build.
        // Storage ids don't promise the same order across compiler builds.
        storages.sort_unstable_by_key(|&(storage_id, _)| storage_id);

        Selected { storages }
    }

    /// Selects the members of `set`, `guards` borrowing each of them in member order.
    ///
    /// A connected set's members are sorted and distinct, as `Selected` needs.
    fn guarded(
        set: &ConnectedSet,
        guards: &'a mut [ARefMut<'_, &mut (dyn Storage + 'static)>],
    ) -> Selected<'a> {
        let storages = set
            .members
            .iter()
            .copied()
            .zip(guards.iter_mut().map(|guard| &mut **guard))
            .collect();

        Selected { storages }
    }

    /// Returns the ids of the selected storages, sorted.
    pub(super) fn ids(&self) -> impl Iterator<Item = StorageId> {
        self.storages.iter().map(|&(storage_id, _)| storage_id)
    }

    /// Returns the selected storage with id `storage_id`.
    ///
    /// ### Panics
    ///
    /// - The storage isn't selected.
    #[track_caller]
    pub(super) fn get(&self, storage_id: StorageId) -> &(dyn Storage + 'static) {
        &*self.storages[self.position(storage_id)].1
    }

    /// Returns the selected storage with id `storage_id`.
    ///
    /// ### Panics
    ///
    /// - The storage isn't selected.
    #[track_caller]
    pub(super) fn get_mut(&mut self, storage_id: StorageId) -> &mut (dyn Storage + 'static) {
        let position = self.position(storage_id);

        &mut *self.storages[position].1
    }

    /// Returns every selected storage with its id, sorted by id.
    pub(super) fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (StorageId, &mut (dyn Storage + 'static))> {
        self.storages
            .iter_mut()
            .map(|(storage_id, storage)| (*storage_id, &mut **storage))
    }

    /// Returns the position of `storage_id` in `storages`.
    ///
    /// ### Panics
    ///
    /// - The storage isn't selected.
    #[track_caller]
    fn position(&self, storage_id: StorageId) -> usize {
        self.storages
            .binary_search_by_key(&storage_id, |&(storage_id, _)| storage_id)
            .expect("regroup can only reach the storages it selected")
    }
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
