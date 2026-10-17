use crate::all_storages::{member_groups, AllStorages};
use crate::entity_id::EntityId;
use crate::sparse_set::{GroupRange, GroupRegion};
use crate::storage::{SBox, Storage, StorageId};
use crate::ShipHashMap;
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
    /// Moves every grouped component to where it belongs.
    #[doc(hidden)]
    pub fn regroup(&mut self) {
        let storages = self.storages.get_mut();
        let storage_ids = sorted_storage_ids(storages);

        // Evaluation appends the same entity to every member of a group,
        // which only keeps them aligned when they're aligned to begin with.
        repair_all(storages, &storage_ids);

        let mut groups: Vec<Arc<[StorageId]>> = Vec::new();
        let mut members: Vec<StoragePtr> = Vec::new();

        for storage_id in storage_ids {
            let storage = storage_ptr(storages, storage_id);
            // SAFE: `storage` was just created and is only used for this read,
            // see the comment at the top of this file.
            range_groups(unsafe { &*storage }, &mut groups);

            // Evaluation only handles a single group per storage until unions land.
            let [group] = &groups[..] else {
                continue;
            };

            // This refreshes the pointer to the storage we're visiting too, `storage` isn't used anymore.
            members.clear();
            members.extend(
                group
                    .iter()
                    .map(|&member_id| storage_ptr(storages, member_id)),
            );

            // Group ids are sorted, so we can find which member is the storage we're visiting.
            let position = group
                .binary_search(&storage_id)
                .expect("a storage belongs to the group of its range");

            evaluate(group, &members, position);
        }
    }
}

/// Returns the id of every storage, sorted.
///
/// The map's iteration order can change between runs.
/// Sorting keeps the layout, and therefore grouped iteration order, reproducible.
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

/// Moves every unevaluated component of the member at `position` to the group's range or to its ungrouped region.
///
/// The group must have been repaired.
fn evaluate(group: &Arc<[StorageId]>, members: &[StoragePtr], position: usize) {
    let storage = members[position];

    // Every iteration moves one component out of the unevaluated region and nothing adds to it,
    // so the loop ends.
    loop {
        // SAFE: `members` comes from `regroup`, and this reference only lives until we read `entity`.
        let storage_ref = unsafe { &*storage };
        let unevaluated_start = storage_ref
            .groups()
            .expect("group members are SparseSet")
            .unevaluated_start;
        let dense = storage_ref.dense().expect("group members are SparseSet");

        let Some(&entity) = dense.get(unevaluated_start) else {
            break;
        };

        let in_every_member = members.iter().all(|&member| {
            // SAFE: `members` comes from `regroup`, and this reference only lives for this check.
            let member = unsafe { &*member };

            member
                .sparse_array()
                .expect("group members are SparseSet")
                .contains(entity)
        });

        if in_every_member {
            // Every member appends the same entity at the same time, so their ranges stay identical.
            for &member in members {
                // SAFE: `members` comes from `regroup`, and this reference only lives for this move.
                let member = unsafe { &mut *member };

                // The group was repaired so every member's range holds the same entities,
                // and `entity` isn't in our range since it's unevaluated.
                debug_assert!(!is_in_range(member, group, entity));

                member.move_to(entity, GroupRegion::Range(group));
            }
        } else {
            // SAFE: `members` comes from `regroup`, and this reference only lives for this move.
            unsafe { &mut *storage }.move_to(entity, GroupRegion::Ungrouped);
        }
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

/// Returns `true` if `entity`'s component is inside `group`'s range in `storage`.
fn is_in_range(storage: &dyn Storage, group: &Arc<[StorageId]>, entity: EntityId) -> bool {
    let range = group_range(storage, group);
    let index = storage
        .sparse_array()
        .expect("group members are SparseSet")
        .get(entity)
        .filter(|sparse_entity| sparse_entity.gen() == entity.gen())
        .map(|sparse_entity| sparse_entity.uindex());

    index.is_some_and(|index| range.start <= index && index < range.end())
}

#[cfg(test)]
mod tests {
    use super::{repair_all, sorted_storage_ids, storage_ptr, union_group, Discovery};
    use crate::sparse_set::{GroupRegion, SparseSet};
    use crate::storage::{Storage, StorageId};
    use crate::{track, Component, EntityId, Get, IntoIter, View, ViewMut, World};
    use alloc::sync::Arc;
    use alloc::vec;
    use alloc::vec::Vec;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    impl Component for A {
        type Tracking = track::Untracked;
    }

    #[derive(Debug, PartialEq)]
    struct B(u32);
    impl Component for B {
        type Tracking = track::Untracked;
    }

    struct C;
    impl Component for C {
        type Tracking = track::Untracked;
    }

    struct D;
    impl Component for D {
        type Tracking = track::Untracked;
    }

    struct E;
    impl Component for E {
        type Tracking = track::Untracked;
    }

    struct TrackedA(u32);
    impl Component for TrackedA {
        type Tracking = track::All;
    }

    struct TrackedB;
    impl Component for TrackedB {
        type Tracking = track::All;
    }

    /// Returns the entities of `group`'s range in `storage`, in logical order.
    fn logical(storage: &dyn Storage, group: &Arc<[StorageId]>) -> Vec<EntityId> {
        let ring = storage.groups().unwrap().range(group).unwrap().ring();
        let dense = storage.dense().unwrap();

        (0..ring.len)
            .map(|logical| dense[ring.physical_of(logical)])
            .collect()
    }

    /// Panics unless every grouped storage is fully evaluated
    /// and every group's members hold identical, fully trusted ranges.
    fn assert_regrouped(world: &World) {
        let all_storages = world.all_storages().unwrap();
        let storages = all_storages.storages.read();

        for sbox in storages.values() {
            // SAFE: `sbox.0` comes from a `Box` that lives as long as `all_storages`,
            // and `borrow` checks no one borrows the storage exclusively.
            let storage = unsafe { &*sbox.0 }.borrow().unwrap();
            let Some(groups) = storage.groups() else {
                continue;
            };
            if !groups.has_ranges() {
                continue;
            }
            let dense = storage.dense().unwrap();

            #[cfg(debug_assertions)]
            groups.assert_invariants(dense.len());
            assert_eq!(groups.unevaluated_start, dense.len());

            for range in &groups.ranges {
                assert_eq!(range.trust, range.len);

                for member_id in range.group.iter() {
                    // SAFE: same as above.
                    let member = unsafe { &*storages[member_id].0 }.borrow().unwrap();

                    assert_eq!(
                        logical(&*member, &range.group),
                        logical(&*storage, &range.group)
                    );
                }
            }
        }
    }

    /// Returns the entities in `T`'s range, in order.
    fn range_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
        let view = world.borrow::<View<'_, T>>().unwrap();

        view.dense[..view.groups.grouped_end()].to_vec()
    }

    /// Returns the entities in `T`'s ungrouped region, sorted.
    fn ungrouped_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
        let view = world.borrow::<View<'_, T>>().unwrap();

        let mut entities =
            view.dense[view.groups.grouped_end()..view.groups.unevaluated_start].to_vec();
        entities.sort_unstable();
        entities
    }

    /// Returns the entities in `T`'s unevaluated region, sorted.
    fn unevaluated_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
        let view = world.borrow::<View<'_, T>>().unwrap();

        let mut entities = view.dense[view.groups.unevaluated_start..].to_vec();
        entities.sort_unstable();
        entities
    }

    /// Returns `entities` sorted.
    fn sorted(mut entities: Vec<EntityId>) -> Vec<EntityId> {
        entities.sort_unstable();
        entities
    }

    /// Asserts that declaring a group places the entities holding both components in both ranges,
    /// and the entities holding only one in that storage's ungrouped region.
    #[test]
    fn declare() {
        let mut world = World::new();
        let ab0 = world.add_entity((A(0), B(0)));
        let a1 = world.add_entity(A(1));
        let b2 = world.add_entity(B(2));
        let ab3 = world.add_entity((A(3), B(3)));

        world.group::<(A, B)>();

        assert_regrouped(&world);
        assert_eq!(sorted(range_of::<A>(&world)), [ab0, ab3]);
        assert_eq!(ungrouped_of::<A>(&world), [a1]);
        assert_eq!(ungrouped_of::<B>(&world), [b2]);
    }

    /// Asserts that every component keeps its value after declaring a group moves it.
    #[test]
    fn declare_values() {
        let mut world = World::new();
        let ab0 = world.add_entity((A(0), B(0)));
        let a1 = world.add_entity(A(1));
        let b2 = world.add_entity(B(2));
        let ab3 = world.add_entity((A(3), B(3)));

        world.group::<(A, B)>();

        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        assert_eq!(a.get(ab0), Ok(&A(0)));
        assert_eq!(a.get(a1), Ok(&A(1)));
        assert_eq!(a.get(ab3), Ok(&A(3)));
        assert_eq!(b.get(ab0), Ok(&B(0)));
        assert_eq!(b.get(b2), Ok(&B(2)));
        assert_eq!(b.get(ab3), Ok(&B(3)));
    }

    /// Asserts that declaring a group that already exists doesn't regroup,
    /// components added since the first declaration stay unevaluated.
    #[test]
    fn declare_duplicate() {
        let mut world = World::new();
        world.group::<(A, B)>();
        let ab = world.add_entity((A(0), B(0)));

        world.group::<(A, B)>();

        assert_eq!(unevaluated_of::<A>(&world), [ab]);
        assert_eq!(unevaluated_of::<B>(&world), [ab]);
    }

    /// Asserts that new entities land in both unevaluated regions,
    /// and that regroup appends them to both ranges without moving the previous range contents.
    #[test]
    fn new_components() {
        let mut world = World::new();
        world.add_entity((A(0), B(0)));
        world.add_entity(A(1));
        world.add_entity((A(2), B(2)));
        world.group::<(A, B)>();
        let range_before = range_of::<A>(&world);

        let ab3 = world.add_entity((A(3), B(3)));
        let ab4 = world.add_entity((A(4), B(4)));

        assert_eq!(unevaluated_of::<A>(&world), [ab3, ab4]);
        assert_eq!(unevaluated_of::<B>(&world), [ab3, ab4]);

        world.regroup();

        assert_regrouped(&world);
        let range_after = range_of::<A>(&world);
        assert_eq!(range_after[..2], range_before);
        assert_eq!(sorted(range_after[2..].to_vec()), [ab3, ab4]);
    }

    /// Asserts that adding B to an entity whose A sits in A's ungrouped region puts B in B's unevaluated region,
    /// and that regroup then moves the entity into both ranges.
    #[test]
    fn late_component() {
        let mut world = World::new();
        let ab0 = world.add_entity((A(0), B(0)));
        let a1 = world.add_entity(A(1));
        world.group::<(A, B)>();

        world.add_component(a1, B(1));

        assert_eq!(ungrouped_of::<A>(&world), [a1]);
        assert_eq!(unevaluated_of::<B>(&world), [a1]);

        world.regroup();

        assert_regrouped(&world);
        assert_eq!(sorted(range_of::<A>(&world)), [ab0, a1]);
        assert!(ungrouped_of::<A>(&world).is_empty());
    }

    /// Asserts that removing a grouped A resets A while B keeps its range,
    /// and that regroup then evicts B's suffix and places the remaining entities again.
    #[test]
    fn removal() {
        let mut world = World::new();
        let ab0 = world.add_entity((A(0), B(0)));
        let ab1 = world.add_entity((A(1), B(1)));
        let ab2 = world.add_entity((A(2), B(2)));
        world.group::<(A, B)>();

        world.remove::<(A,)>(ab1);

        assert!(range_of::<A>(&world).is_empty());
        assert_eq!(unevaluated_of::<A>(&world), sorted(vec![ab0, ab2]));
        assert_eq!(range_of::<B>(&world).len(), 3);

        world.regroup();

        assert_regrouped(&world);
        assert_eq!(sorted(range_of::<B>(&world)), [ab0, ab2]);
        assert_eq!(ungrouped_of::<B>(&world), [ab1]);
    }

    /// Asserts that regroup moves insertion and modification timestamps with their components.
    #[test]
    fn tracking() {
        let mut world = World::new();
        // Holding only A, it sits before the others in A and forces them to move.
        world.add_entity(TrackedA(0));
        let old = world.add_entity((TrackedA(1), TrackedB));
        world.run(|a: ViewMut<'_, TrackedA, track::All>| a.clear_all_inserted());
        let new = world.add_entity((TrackedA(2), TrackedB));
        world.run(|mut a: ViewMut<'_, TrackedA, track::All>| {
            (&mut a).get(old).unwrap().0 = 10;
        });

        let state = |world: &World| {
            let a = world.borrow::<View<'_, TrackedA, track::All>>().unwrap();
            [
                a.is_inserted(old),
                a.is_inserted(new),
                a.is_modified(old),
                a.is_modified(new),
            ]
        };

        let before = state(&world);
        world.group::<(TrackedA, TrackedB)>();
        let after = state(&world);

        assert_eq!(before, [false, true, true, false]);
        assert_eq!(after, before);
    }

    /// Builds a world with a group, then adds, removes and regroups components.
    fn build_world() -> World {
        let mut world = World::new();
        let mut entities = Vec::new();
        for index in 0..30 {
            let entity = match index % 3 {
                0 => world.add_entity((A(index), B(index))),
                1 => world.add_entity((A(index), C)),
                _ => world.add_entity((B(index), C)),
            };
            entities.push(entity);
        }

        world.group::<(A, B)>();

        for index in 30..40 {
            world.add_entity((A(index), B(index)));
        }
        world.remove::<(A,)>(entities[3]);
        world.add_component(entities[1], B(1));
        world.regroup();

        world
    }

    /// Returns `T`'s entities in physical order.
    fn dense_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
        world.borrow::<View<'_, T>>().unwrap().dense.clone()
    }

    /// Asserts that two worlds built with identical operations end up with identical layouts,
    /// even though their storage maps may iterate in different orders.
    #[test]
    fn determinism() {
        let world0 = build_world();
        let world1 = build_world();

        assert_regrouped(&world0);
        assert_eq!(dense_of::<A>(&world0), dense_of::<A>(&world1));
        assert_eq!(dense_of::<B>(&world0), dense_of::<B>(&world1));
        assert_eq!(dense_of::<C>(&world0), dense_of::<C>(&world1));
    }

    /// Asserts that regroup leaves storages without a range untouched.
    #[test]
    fn ungrouped_storage() {
        let mut world = World::new();
        world.add_entity((A(0), C));
        world.add_entity((B(1), C));
        world.add_entity((A(2), B(2), C));
        let dense_before = dense_of::<C>(&world);

        world.group::<(A, B)>();

        let c = world.borrow::<View<'_, C>>().unwrap();
        assert_eq!(c.dense, dense_before);
        assert!(!c.groups.has_ranges());
        assert_eq!(c.groups.unevaluated_start, 0);
    }

    /// Asserts that iterating `(A, B)` yields the same items before and after regroup, in any order.
    #[test]
    fn iteration() {
        let mut world = World::new();
        world.add_entity((A(0), B(0)));
        world.add_entity(A(1));
        world.group::<(A, B)>();
        world.add_entity((A(2), B(2)));
        world.add_entity(B(3));
        world.add_entity((A(4), B(4)));

        let collect = |world: &World| {
            let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
            let mut items = (&a, &b).iter().map(|(a, b)| (a.0, b.0)).collect::<Vec<_>>();
            items.sort_unstable();
            items
        };

        let before = collect(&world);
        world.regroup();
        let after = collect(&world);

        assert_eq!(before, [(0, 0), (2, 2), (4, 4)]);
        assert_eq!(after, before);
    }

    /// Asserts that a group with a `NonSend` member is regrouped like any other.
    #[cfg(feature = "thread_local")]
    #[test]
    fn non_send() {
        use crate::borrow::NonSend;
        use crate::AddComponent;

        let mut world = World::new();
        let ab0 = world.add_entity(A(0));
        let a1 = world.add_entity(A(1));
        let ab2 = world.add_entity(A(2));
        {
            let mut b = world.borrow::<NonSend<ViewMut<'_, B>>>().unwrap();
            b.add_component_unchecked(ab2, B(2));
            b.add_component_unchecked(ab0, B(0));
        }

        world.group::<(A, NonSend<B>)>();

        assert_regrouped(&world);
        assert_eq!(sorted(range_of::<A>(&world)), [ab0, ab2]);
        assert_eq!(ungrouped_of::<A>(&world), [a1]);
        let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
        assert_eq!(b.dense[..b.groups.grouped_end()], range_of::<A>(&world));
        assert_eq!(b.get(ab0), Ok(&B(0)));
        assert_eq!(b.get(ab2), Ok(&B(2)));
    }

    /// Attaches the group of `T` and `U` to both storages without regrouping,
    /// like a declaration would before it regroups.
    ///
    /// Public declarations still reject a storage in two groups, tests build them this way.
    fn attach<T: Component + Send + Sync, U: Component + Send + Sync>(
        world: &World,
    ) -> Arc<[StorageId]> {
        let t_id = StorageId::of::<SparseSet<T>>();
        let u_id = StorageId::of::<SparseSet<U>>();
        let mut ids = vec![t_id, u_id];
        ids.sort_unstable();
        let group: Arc<[StorageId]> = Arc::from(ids);

        let (mut t, mut u) = world.borrow::<(ViewMut<'_, T>, ViewMut<'_, U>)>().unwrap();
        t.groups.attach(group.clone(), t_id);
        u.groups.attach(group.clone(), u_id);

        group
    }

    /// Moves `entity`'s `T` and `U` components to the end of `group`'s range.
    fn place<T: Component + Send + Sync, U: Component + Send + Sync>(
        world: &World,
        group: &Arc<[StorageId]>,
        entity: EntityId,
    ) {
        let (mut t, mut u) = world.borrow::<(ViewMut<'_, T>, ViewMut<'_, U>)>().unwrap();
        t.private_move_to(entity, GroupRegion::Range(group));
        u.private_move_to(entity, GroupRegion::Range(group));
    }

    /// Moves `entity`'s `T` component to `T`'s unevaluated region, damaging the range it leaves.
    fn evict<T: Component + Send + Sync>(world: &World, entity: EntityId) {
        let mut t = world.borrow::<ViewMut<'_, T>>().unwrap();
        t.private_move_to(entity, GroupRegion::Unevaluated);
    }

    /// Runs the repair pass alone, without evaluating anything.
    fn repair(world: &World) {
        let mut all_storages = world.all_storages_mut().unwrap();
        let storages = all_storages.storages.get_mut();
        let storage_ids = sorted_storage_ids(storages);

        repair_all(storages, &storage_ids);
    }

    /// Returns `group`'s range in `T` in logical order, along with its trust.
    fn group_of<T: Component + Send + Sync>(
        world: &World,
        group: &Arc<[StorageId]>,
    ) -> (Vec<EntityId>, usize) {
        let view = world.borrow::<View<'_, T>>().unwrap();
        let range = view.groups.range(group).unwrap();
        let entities = (0..range.len)
            .map(|logical| view.dense[range.ring().physical_of(logical)])
            .collect();

        (entities, range.trust)
    }

    /// Returns a world with AB holding `e0`, `e1` and `e2`, AC holding `e3` and `e4`,
    /// all aligned and fully trusted, with the entities in index order.
    ///
    /// AC is filled first, so each AB append passes a hole through it and rotates it in A.
    fn overlapping() -> (World, Arc<[StorageId]>, Arc<[StorageId]>, Vec<EntityId>) {
        let mut world = World::new();
        let mut entities = Vec::new();
        for index in 0..3 {
            entities.push(world.add_entity((A(index), B(index))));
        }
        for index in 3..5 {
            entities.push(world.add_entity((A(index), C)));
        }

        let ab = attach::<A, B>(&world);
        let ac = attach::<A, C>(&world);
        place::<A, C>(&world, &ac, entities[3]);
        place::<A, C>(&world, &ac, entities[4]);
        for &entity in &entities[..3] {
            place::<A, B>(&world, &ab, entity);
        }

        (world, ab, ac, entities)
    }

    /// Asserts that the `overlapping` fixture holds the layout tests rely on: AC is rotated in A
    /// and not in C, both groups are aligned and fully trusted.
    #[test]
    fn overlapping_fixture() {
        let (world, ab, ac, e) = overlapping();

        assert_ne!(
            world
                .borrow::<View<'_, A>>()
                .unwrap()
                .groups
                .range(&ac)
                .unwrap()
                .shift,
            0
        );
        assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
        assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
        assert_eq!(group_of::<A>(&world, &ac), (vec![e[3], e[4]], 2));
        assert_eq!(group_of::<C>(&world, &ac), (vec![e[3], e[4]], 2));
    }

    /// Asserts that repairing aligned, fully trusted groups doesn't move anything.
    #[test]
    fn repair_aligned() {
        let (world, _, _, _) = overlapping();
        let before = (
            dense_of::<A>(&world),
            dense_of::<B>(&world),
            dense_of::<C>(&world),
        );

        repair(&world);

        let after = (
            dense_of::<A>(&world),
            dense_of::<B>(&world),
            dense_of::<C>(&world),
        );
        assert_eq!(after, before);
    }

    /// Asserts that repair shrinks each of two overlapping groups to the prefix all its members
    /// trust, in every member, even with B shorter than A and AC rotated in A.
    #[test]
    fn repair_overlapping() {
        let (world, ab, ac, e) = overlapping();
        // B's AB becomes [e0, e2] and only trusts e0. C's AC becomes [e3] and trusts it.
        evict::<B>(&world, e[1]);
        evict::<C>(&world, e[4]);

        repair(&world);

        assert_eq!(group_of::<A>(&world, &ab), (vec![e[0]], 1));
        assert_eq!(group_of::<B>(&world, &ab), (vec![e[0]], 1));
        assert_eq!(group_of::<A>(&world, &ac), (vec![e[3]], 1));
        assert_eq!(group_of::<C>(&world, &ac), (vec![e[3]], 1));
    }

    /// Asserts that repair evicts each damaged suffix to its own storage's unevaluated region
    /// and that every component stays reachable with its value.
    #[test]
    fn repair_evictions() {
        let (world, _, _, e) = overlapping();
        evict::<B>(&world, e[1]);
        evict::<C>(&world, e[4]);

        repair(&world);

        assert_eq!(unevaluated_of::<A>(&world), [e[1], e[2], e[4]]);
        assert_eq!(unevaluated_of::<B>(&world), [e[1], e[2]]);
        assert_eq!(unevaluated_of::<C>(&world), [e[4]]);
        let (a, b) = world.borrow::<(View<'_, A>, View<'_, B>)>().unwrap();
        for (index, &entity) in e.iter().enumerate() {
            assert_eq!(a.get(entity), Ok(&A(index as u32)));
        }
        for (index, &entity) in e[..3].iter().enumerate() {
            assert_eq!(b.get(entity), Ok(&B(index as u32)));
        }
    }

    /// Asserts that evicting from AB in A passes holes through the rotated AC
    /// without changing its order or trust.
    #[test]
    fn repair_keeps_other_groups() {
        let (world, ab, ac, e) = overlapping();
        evict::<B>(&world, e[2]);

        repair(&world);

        assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1]], 2));
        assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1]], 2));
        assert_eq!(group_of::<A>(&world, &ac), (vec![e[3], e[4]], 2));
        assert_eq!(group_of::<C>(&world, &ac), (vec![e[3], e[4]], 2));
    }

    /// Asserts that a member reset by a removal empties its groups in every member,
    /// while groups it isn't part of stay as they were.
    #[test]
    fn repair_reset_member() {
        let (mut world, ab, ac, e) = overlapping();
        world.remove::<(C,)>(e[4]);

        repair(&world);

        assert_eq!(group_of::<A>(&world, &ac), (vec![], 0));
        assert_eq!(group_of::<C>(&world, &ac), (vec![], 0));
        assert_eq!(group_of::<A>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
        assert_eq!(group_of::<B>(&world, &ab), (vec![e[0], e[1], e[2]], 3));
        assert_eq!(unevaluated_of::<A>(&world), [e[3], e[4]]);
        assert_eq!(unevaluated_of::<C>(&world), [e[3]]);
    }

    fn id<T: Component + Send + Sync>() -> StorageId {
        StorageId::of::<SparseSet<T>>()
    }

    /// Attaches the group made of `ids` to each of its members without regrouping.
    ///
    /// Every member storage must exist.
    fn attach_ids(world: &World, ids: &[StorageId]) -> Arc<[StorageId]> {
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        let group: Arc<[StorageId]> = Arc::from(ids);

        let mut all_storages = world.all_storages_mut().unwrap();
        let storages = all_storages.storages.get_mut();
        for &member_id in group.iter() {
            let member = storage_ptr(storages, member_id);
            // SAFE: `member` was just created and is the only reference to this storage.
            let groups = unsafe { &mut *member }.groups_mut().unwrap();

            groups.attach(group.clone(), member_id);
        }

        group
    }

    /// Returns the storages `entity` has to be grouped in, starting from its component in `storage_id`.
    fn discover(world: &World, storage_id: StorageId, entity: EntityId) -> Option<Vec<StorageId>> {
        let mut all_storages = world.all_storages_mut().unwrap();
        let storages = all_storages.storages.get_mut();

        Discovery::default()
            .discover(storages, storage_id, entity)
            .map(<[StorageId]>::to_vec)
    }

    /// Returns the group made of `members`, creating it when it doesn't exist yet.
    fn union(world: &World, members: &[StorageId]) -> Arc<[StorageId]> {
        let mut all_storages = world.all_storages_mut().unwrap();
        let storages = all_storages.storages.get_mut();
        let mut members = members.to_vec();
        members.sort_unstable();

        union_group(storages, &members)
    }

    /// Returns the groups of `T`'s ranges, in range order.
    fn groups_of<T: Component + Send + Sync>(world: &World) -> Vec<Arc<[StorageId]>> {
        let view = world.borrow::<View<'_, T>>().unwrap();

        view.groups
            .ranges
            .iter()
            .map(|range| range.group.clone())
            .collect()
    }

    /// Returns `ids` sorted.
    fn sorted_ids(mut ids: Vec<StorageId>) -> Vec<StorageId> {
        ids.sort_unstable();
        ids
    }

    /// Asserts that with AB and AC, an entity holding A, B and C has to be grouped in all three,
    /// from any of them.
    #[test]
    fn discover_overlapping() {
        let mut world = World::new();
        let entity = world.add_entity((A(0), B(0), C));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<A>(), id::<C>()]);
        let abc = sorted_ids(vec![id::<A>(), id::<B>(), id::<C>()]);

        assert_eq!(discover(&world, id::<A>(), entity), Some(abc.clone()));
        assert_eq!(discover(&world, id::<C>(), entity), Some(abc));
    }

    /// Asserts that with AB and AC, an entity holding only A and B has to be grouped in A and B.
    #[test]
    fn discover_one_satisfied() {
        let mut world = World::new();
        world.add_entity((A(0), B(0), C));
        let entity = world.add_entity((A(1), B(1)));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<A>(), id::<C>()]);

        assert_eq!(
            discover(&world, id::<A>(), entity),
            Some(sorted_ids(vec![id::<A>(), id::<B>()]))
        );
    }

    /// Asserts that an entity satisfying none of a storage's groups has nowhere to be grouped.
    #[test]
    fn discover_none() {
        let mut world = World::new();
        world.add_entity((A(0), B(0), C));
        let entity = world.add_entity(A(1));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<A>(), id::<C>()]);

        assert_eq!(discover(&world, id::<A>(), entity), None);
    }

    /// Asserts that with AB, BC and CD, an entity holding all four is grouped in all of them,
    /// from either end of the chain.
    #[test]
    fn discover_chain() {
        let mut world = World::new();
        let entity = world.add_entity((A(0), B(0), C, D));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<B>(), id::<C>()]);
        attach_ids(&world, &[id::<C>(), id::<D>()]);
        let abcd = sorted_ids(vec![id::<A>(), id::<B>(), id::<C>(), id::<D>()]);

        assert_eq!(discover(&world, id::<A>(), entity), Some(abcd.clone()));
        assert_eq!(discover(&world, id::<D>(), entity), Some(abcd));
    }

    /// Asserts that groups an entity doesn't satisfy don't connect anything:
    /// with AB, BE, EC and CD and no E, AB and CD stay apart.
    #[test]
    fn discover_unsatisfied_link() {
        let mut world = World::new();
        world.add_entity((B(0), C, E));
        let entity = world.add_entity((A(0), B(0), C, D));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<B>(), id::<E>()]);
        attach_ids(&world, &[id::<E>(), id::<C>()]);
        attach_ids(&world, &[id::<C>(), id::<D>()]);

        assert_eq!(
            discover(&world, id::<A>(), entity),
            Some(sorted_ids(vec![id::<A>(), id::<B>()]))
        );
        assert_eq!(
            discover(&world, id::<D>(), entity),
            Some(sorted_ids(vec![id::<C>(), id::<D>()]))
        );
    }

    /// Asserts that owning the components of two disjoint groups doesn't merge them.
    #[test]
    fn discover_disjoint() {
        let mut world = World::new();
        let entity = world.add_entity((A(0), B(0), C, D));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<C>(), id::<D>()]);

        assert_eq!(
            discover(&world, id::<A>(), entity),
            Some(sorted_ids(vec![id::<A>(), id::<B>()]))
        );
        assert_eq!(
            discover(&world, id::<C>(), entity),
            Some(sorted_ids(vec![id::<C>(), id::<D>()]))
        );
    }

    /// Asserts that with AB and ABC, the larger group adds C only to entities holding C.
    #[test]
    fn discover_subset() {
        let mut world = World::new();
        let abc_entity = world.add_entity((A(0), B(0), C));
        let ab_entity = world.add_entity((A(1), B(1)));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<A>(), id::<B>(), id::<C>()]);

        assert_eq!(
            discover(&world, id::<A>(), abc_entity),
            Some(sorted_ids(vec![id::<A>(), id::<B>(), id::<C>()]))
        );
        assert_eq!(
            discover(&world, id::<A>(), ab_entity),
            Some(sorted_ids(vec![id::<A>(), id::<B>()]))
        );
    }

    /// Asserts that a component left by a deleted entity doesn't satisfy a group
    /// for the entity now using its index.
    #[test]
    fn discover_stale_generation() {
        use crate::AddComponent;

        let mut world = World::new();
        let deleted = world.add_entity((A(0), B(0)));
        world.delete_entity(deleted);
        let entity = world.add_entity(A(1));
        assert_eq!(entity.index(), deleted.index());
        world
            .borrow::<ViewMut<'_, B>>()
            .unwrap()
            .add_component_unchecked(deleted, B(0));
        attach_ids(&world, &[id::<A>(), id::<B>()]);

        assert_eq!(discover(&world, id::<A>(), entity), None);
    }

    /// Asserts that a missing union is created once, as a single shared definition attached
    /// as an empty range after the last range of every member, and then reused.
    #[test]
    fn union_created_once() {
        let mut world = World::new();
        world.add_entity((A(0), B(0), C));
        let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);
        let ac = attach_ids(&world, &[id::<A>(), id::<C>()]);

        let abc = union(&world, &[id::<A>(), id::<B>(), id::<C>()]);
        let again = union(&world, &[id::<C>(), id::<A>(), id::<B>()]);

        assert!(Arc::ptr_eq(&abc, &again));
        let a = groups_of::<A>(&world);
        assert!(Arc::ptr_eq(&a[0], &ab) && Arc::ptr_eq(&a[1], &ac) && Arc::ptr_eq(&a[2], &abc));
        assert_eq!(a.len(), 3);
        assert!(groups_of::<B>(&world)
            .iter()
            .any(|group| Arc::ptr_eq(group, &abc)));
        assert!(groups_of::<C>(&world)
            .iter()
            .any(|group| Arc::ptr_eq(group, &abc)));
        let view = world.borrow::<View<'_, A>>().unwrap();
        assert_eq!(
            (view.groups.ranges[2].len, view.groups.ranges[2].trust),
            (0, 0)
        );
    }

    /// Asserts that a group satisfied alone is its own union, nothing new is attached.
    #[test]
    fn union_existing_group() {
        let mut world = World::new();
        world.add_entity((A(0), B(0)));
        let ab = attach_ids(&world, &[id::<A>(), id::<B>()]);

        assert!(Arc::ptr_eq(&union(&world, &[id::<A>(), id::<B>()]), &ab));
        assert_eq!(groups_of::<A>(&world).len(), 1);
    }

    /// Asserts that a declared group equal to the union is reused instead of creating a new one.
    #[test]
    fn union_declared() {
        let mut world = World::new();
        world.add_entity((A(0), B(0), C));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<A>(), id::<C>()]);
        let abc = attach_ids(&world, &[id::<A>(), id::<B>(), id::<C>()]);

        assert!(Arc::ptr_eq(
            &union(&world, &[id::<A>(), id::<B>(), id::<C>()]),
            &abc
        ));
        assert_eq!(groups_of::<A>(&world).len(), 3);
    }

    /// Asserts that a transitive union is created directly, without any intermediate union:
    /// with AB, BC and CD, only ABCD is added.
    #[test]
    fn union_direct() {
        let mut world = World::new();
        let entity = world.add_entity((A(0), B(0), C, D));
        attach_ids(&world, &[id::<A>(), id::<B>()]);
        attach_ids(&world, &[id::<B>(), id::<C>()]);
        attach_ids(&world, &[id::<C>(), id::<D>()]);

        let members = discover(&world, id::<A>(), entity).unwrap();
        let abcd = union(&world, &members);

        assert_eq!(abcd.len(), 4);
        assert_eq!(groups_of::<A>(&world).len(), 2);
        assert_eq!(groups_of::<B>(&world).len(), 3);
        assert_eq!(groups_of::<C>(&world).len(), 3);
        assert_eq!(groups_of::<D>(&world).len(), 2);
    }
}
