#[cfg(test)]
mod tests;

use crate::ShipHashMap;
use crate::all_storages::{AllStorages, member_groups};
use crate::entity_id::EntityId;
use crate::sparse_set::{GroupRange, GroupRegion};
use crate::storage::{SBox, Storage, StorageId};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// A pointer to a storage, regroup holds one per group member.
///
/// The `'static` is spelled out, inside a reference it would default to the reference's lifetime.
type StoragePtr = *mut (dyn Storage + 'static);

// Regroup works on all members of a group at once, so it holds them as raw pointers.
// This is sound because:
// - `&mut AllStorages` guarantees nobody else borrows any storage,
// - members are distinct storages, `AllStorages::group` rejects groups listing a storage twice,
// - storages can't be removed while we hold the map mutably, so the pointers stay valid,
// - we only turn a pointer into a reference for one operation at a time,
//   so two references to the same member never coexist,
// - a set of pointers is rebuilt before each use, since getting a fresh pointer to a storage
//   invalidates the previous one.

impl AllStorages {
    /// Moves components to the groups they belong to, to speed up iteration.
    ///
    /// Components only move to their group during a regroup, explicit or from declaring a new group.\
    /// Iteration stays correct between regroups, but adding or removing components slows it down.
    pub fn regroup(&mut self) {
        let storages = self.storages.get_mut();
        let storage_ids = sorted_storage_ids(storages);

        // Evaluation moves the same entity the same way in every member of a group,
        // which only keeps them aligned when they're aligned to begin with.
        repair_all(storages, &storage_ids);

        let mut discovery = Discovery::default();
        let mut sources: Vec<Arc<[StorageId]>> = Vec::new();

        for storage_id in storage_ids {
            evaluate(storages, storage_id, &mut discovery, &mut sources);
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

/// Repairs every group attached to the storages with ids `storage_ids`, each one once.
///
/// Afterwards every range of every member is aligned with the other members' and fully trusted.
fn repair_all(storages: &mut ShipHashMap<StorageId, SBox>, storage_ids: &[StorageId]) {
    let mut repaired_groups: Vec<Arc<[StorageId]>> = Vec::new();
    let mut groups: Vec<Arc<[StorageId]>> = Vec::new();
    let mut members: Vec<StoragePtr> = Vec::new();

    for &storage_id in storage_ids {
        let storage = storage_ptr(storages, storage_id);
        // SAFE: `storage` was just created and is only used for this read,
        // see the comment at the top of this file.
        range_groups(unsafe { &*storage }, &mut groups);

        for group in &groups {
            // Every member lists the group, we repair it from the first one we visit.
            if repaired_groups
                .iter()
                .any(|repaired_group| Arc::ptr_eq(repaired_group, group))
            {
                continue;
            }

            members.clear();
            members.extend(
                group
                    .iter()
                    .map(|&member_id| storage_ptr(storages, member_id)),
            );

            repair(group, &members);
            repaired_groups.push(group.clone());
        }
    }
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

/// Replaces the content of `groups` with the group of each of `storage`'s ranges, in range order.
///
/// We copy the groups out, moves and new unions can change the storage's ranges.
fn range_groups(storage: &dyn Storage, groups: &mut Vec<Arc<[StorageId]>>) {
    groups.clear();

    if let Some(storage_groups) = storage.groups() {
        groups.extend(
            storage_groups
                .ranges
                .iter()
                .map(|range| range.group.clone()),
        );
    }
}

/// Returns `group`'s range in `storage`.
fn group_range<'a>(storage: &'a dyn Storage, group: &Arc<[StorageId]>) -> &'a GroupRange {
    storage
        .groups()
        .and_then(|groups| groups.range(group))
        .expect("every member holds a range for its group")
}

/// Evicts every member's damaged suffix to its unevaluated region.
///
/// Afterwards every member's range holds exactly the group's aligned part and is fully trusted.\
/// Other ranges of the members keep their logical order and trust.
fn repair(group: &Arc<[StorageId]>, members: &[StoragePtr]) {
    let aligned_len = members
        .iter()
        // SAFE: `members` were just rebuilt by our caller, and this reference only lives for this read.
        .map(|&member| group_range(unsafe { &*member }, group).trust)
        .min()
        .expect("a group has at least two members");

    for &member in members {
        // SAFE: `members` were just rebuilt by our caller,
        // and this is the only reference to this member until the next iteration.
        let member = unsafe { &mut *member };

        loop {
            let range = group_range(member, group);
            if range.len <= aligned_len {
                break;
            }

            // Evicting the logical last component lowers the trust to at most the new length,
            // so once we're down to `aligned_len` the range is fully trusted again.
            // Positions below `aligned_len` never move, they're the prefix every member agrees on.
            let last_index = range.ring().physical_of(range.len - 1);
            let last = member.dense().expect("group members are SparseSet")[last_index];

            member.move_to(last, GroupRegion::Unevaluated);
        }

        debug_assert_eq!(group_range(member, group).trust, aligned_len);
    }
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

/// Moves every unevaluated component of the storage with id `storage_id` where it belongs.
///
/// Every group must have been repaired.\
/// A component satisfying none of the storage's groups goes to the ungrouped region.
/// Otherwise its entity is placed in the union of the groups it connects, in every member at once.
fn evaluate(
    storages: &mut ShipHashMap<StorageId, SBox>,
    storage_id: StorageId,
    discovery: &mut Discovery,
    sources: &mut Vec<Arc<[StorageId]>>,
) {
    // Each iteration takes the first unevaluated component out of the unevaluated region,
    // and nothing adds to it, so the loop ends.
    loop {
        let storage = storage_mut(storages, storage_id);
        // Storages without group are implicitly ungrouped, they have nothing to evaluate.
        let Some(groups) = storage.groups().filter(|groups| groups.has_ranges()) else {
            return;
        };
        let dense = storage.dense().expect("group members are SparseSet");
        let Some(&entity) = dense.get(groups.unevaluated_start) else {
            return;
        };

        let Some(members) = discovery.discover(storages, storage_id, entity) else {
            storage_mut(storages, storage_id).move_to(entity, GroupRegion::Ungrouped);
            continue;
        };

        let union = union_group(storages, members);
        place(storages, members, &union, entity, sources);
    }
}

/// Places `entity` in `union`'s range, in every one of its sorted `members`.
///
/// `entity` can already be in the range of smaller groups, if it gained a component since
/// the last regroup for example. We take it out of each of them in all their members at once,
/// so they stay aligned, then append it to `union` in every member.\
/// `sources` is scratch space.
fn place(
    storages: &mut ShipHashMap<StorageId, SBox>,
    members: &[StorageId],
    union: &Arc<[StorageId]>,
    entity: EntityId,
    sources: &mut Vec<Arc<[StorageId]>>,
) {
    // We find every source group before moving anything, moves change the regions.
    // The entity satisfies every group holding it, so they're all among the groups discovery
    // connected and their members are all in `members`.
    sources.clear();
    for &member_id in members {
        let Some(group) = range_group_of(storage_mut(storages, member_id), entity) else {
            continue;
        };

        // `union` contains the storage being evaluated, where `entity` is unevaluated.
        // Its members are aligned so none of them holds `entity` in `union`.
        debug_assert!(!Arc::ptr_eq(&group, union));

        if !sources.iter().any(|source| Arc::ptr_eq(source, &group)) {
            sources.push(group);
        }
    }

    for source in sources.iter() {
        take_out(storages, source, entity);
    }

    // Every member appends the same entity at the same logical end, `union` stays aligned.
    for &member_id in members {
        storage_mut(storages, member_id).move_to(entity, GroupRegion::Range(union));
    }
}

/// Returns the group whose range holds `entity`'s component in `storage`, if any.
///
/// `entity` must own a component in `storage`.
fn range_group_of(storage: &dyn Storage, entity: EntityId) -> Option<Arc<[StorageId]>> {
    let index = storage
        .sparse_array()
        .expect("group members are SparseSet")
        .get(entity)
        .expect("the entity owns a component in this storage")
        .uindex();

    match storage.groups()?.region_of(index) {
        GroupRegion::Range(group) => Some(group.clone()),
        GroupRegion::Ungrouped | GroupRegion::Unevaluated => None,
    }
}

/// Takes `entity` out of `group`'s range in every member, to their ungrouped region.
///
/// `group` must be aligned and fully trusted, and hold `entity`.
fn take_out(
    storages: &mut ShipHashMap<StorageId, SBox>,
    group: &Arc<[StorageId]>,
    entity: EntityId,
) {
    // Members hold `entity` at the same logical position and the same entity at their logical end,
    // so a logical swap-remove gives each of them the same sequence, whatever their rotation.
    // Each move only lowers its own member's trust, it doesn't know the others did the same.
    for &member_id in group.iter() {
        let storage = storage_mut(storages, member_id);
        debug_assert!(
            group_range(storage, group).trust == group_range(storage, group).len,
            "groups are repaired before evaluation and stay fully trusted"
        );

        storage.move_to(entity, GroupRegion::Ungrouped);
    }

    // Now that they all did it, they're aligned again.
    for &member_id in group.iter() {
        let range = member_groups(storages, member_id)
            .range_mut(group)
            .expect("every member holds a range for its group");

        range.trust = range.len;
    }
}

/// Scratch buffers finding the storages an entity has to be grouped in, reused across entities.
#[derive(Default)]
struct Discovery {
    /// Members of the groups found so far, sorted once discovery is complete.
    members: Vec<StorageId>,
    /// Groups whose satisfaction we already checked, compared by pointer.
    checked: Vec<Arc<[StorageId]>>,
    /// Groups of the storage we're inspecting.
    groups: Vec<Arc<[StorageId]>>,
}

impl Discovery {
    /// Returns the sorted ids of the storages `entity` has to be grouped in, starting from its
    /// component in `storage_id`.
    ///
    /// These are the members of every group `entity` satisfies that's connected to `storage_id`
    /// through other groups it satisfies. Groups it doesn't satisfy don't connect anything,
    /// and owning the components of two disjoint groups doesn't merge them.\
    /// Returns `None` when `entity` satisfies none of `storage_id`'s groups.
    fn discover(
        &mut self,
        storages: &mut ShipHashMap<StorageId, SBox>,
        storage_id: StorageId,
        entity: EntityId,
    ) -> Option<&[StorageId]> {
        self.members.clear();
        self.checked.clear();
        self.members.push(storage_id);

        // `members` grows while we go through it, each member's groups are inspected once.
        // A group we check from one member is skipped from the others, it has the same answer.
        let mut next = 0;
        while let Some(&member_id) = self.members.get(next) {
            next += 1;

            let storage = storage_ptr(storages, member_id);
            // SAFE: `storage` was just created and is only used for this read,
            // see the comment at the top of this file.
            range_groups(unsafe { &*storage }, &mut self.groups);

            for group in &self.groups {
                if self
                    .checked
                    .iter()
                    .any(|checked| Arc::ptr_eq(checked, group))
                {
                    continue;
                }
                self.checked.push(group.clone());

                if !satisfies(storages, group, entity) {
                    continue;
                }

                for &id in group.iter() {
                    if !self.members.contains(&id) {
                        self.members.push(id);
                    }
                }
            }
        }

        // A satisfied group would have added at least one other storage.
        if self.members.len() == 1 {
            return None;
        }

        self.members.sort_unstable();

        Some(&self.members)
    }
}

/// Returns `true` when `entity` owns a component in every member of `group`.
///
/// The generation has to match, a component of a deleted entity reusing the same index doesn't count.
fn satisfies(
    storages: &mut ShipHashMap<StorageId, SBox>,
    group: &Arc<[StorageId]>,
    entity: EntityId,
) -> bool {
    group.iter().all(|&member_id| {
        let member = storage_ptr(storages, member_id);

        // SAFE: `member` was just created and is only used for this read,
        // see the comment at the top of this file.
        unsafe { &*member }
            .sparse_array()
            .expect("group members are SparseSet")
            .contains(entity)
    })
}

/// Returns the group made of the sorted storage ids `members`,
/// attaching it as an empty range to each of them when it doesn't exist yet.
///
/// An existing group is reused whether it was declared or created as a union before,
/// so each set of members has a single identity.
fn union_group(
    storages: &mut ShipHashMap<StorageId, SBox>,
    members: &[StorageId],
) -> Arc<[StorageId]> {
    // Every member holds a range for each of its groups, so checking one of them is enough.
    if let Some(group) = member_groups(storages, members[0]).find_by_value(members) {
        return group.clone();
    }

    let group: Arc<[StorageId]> = Arc::from(members);
    for &member_id in members {
        member_groups(storages, member_id).attach(group.clone(), member_id);
    }

    group
}
