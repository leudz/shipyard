use crate::all_storages::AllStorages;
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

impl AllStorages {
    /// Moves every grouped component to where it belongs.
    #[doc(hidden)]
    pub fn regroup(&mut self) {
        let storages = self.storages.get_mut();

        // The map's iteration order can change between runs.
        // Sorting keeps the layout, and therefore grouped iteration order, reproducible.
        let mut storage_ids: Vec<StorageId> = storages.keys().copied().collect();
        storage_ids.sort_unstable();

        let mut repaired_groups: Vec<Arc<[StorageId]>> = Vec::new();
        // Regroup works on all members of a group at once, so we hold them as raw pointers.
        // This is sound because:
        // - `&mut self` guarantees nobody else borrows any storage,
        // - members are distinct storages, `AllStorages::group` rejects groups listing a storage twice,
        // - storages can't be removed while we hold the map mutably, so the pointers stay valid,
        // - we only turn a pointer into a reference for one operation at a time,
        //   so two references to the same member never coexist.
        let mut members: Vec<StoragePtr> = Vec::new();

        for storage_id in storage_ids {
            let storage = storage_ptr(storages, storage_id);
            // SAFE: `storage` was just created and is only used for this read, see the comment on `members`.
            let Some(group) = range_group(unsafe { &*storage }) else {
                continue;
            };

            // This refreshes the pointer to the storage we're visiting too, `storage` isn't used anymore.
            members.clear();
            members.extend(
                group
                    .iter()
                    .map(|&member_id| storage_ptr(storages, member_id)),
            );

            // Repair runs the first time we visit any member, before any member is evaluated.
            if !repaired_groups
                .iter()
                .any(|repaired_group| Arc::ptr_eq(repaired_group, &group))
            {
                repair(&group, &members);
                repaired_groups.push(group.clone());
            }

            // Group ids are sorted, so we can find which member is the storage we're visiting.
            let position = group
                .binary_search(&storage_id)
                .expect("a storage belongs to the group of its range");

            evaluate(&group, &members, position);
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

/// Returns the group of `storage`'s range, if it has one.
fn range_group(storage: &dyn Storage) -> Option<Arc<[StorageId]>> {
    let range = storage.groups()?.ranges.first()?;

    Some(range.group.clone())
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
/// Afterwards every member's range holds exactly the group's aligned part and is fully trusted.
fn repair(group: &Arc<[StorageId]>, members: &[StoragePtr]) {
    let aligned_len = members
        .iter()
        // SAFE: `members` comes from `regroup`, and this reference only lives for this read.
        .map(|&member| group_range(unsafe { &*member }, group).trust)
        .min()
        .expect("a group has at least two members");

    for &member in members {
        // SAFE: `members` comes from `regroup`, and this is the only reference to this member until the next iteration.
        let member = unsafe { &mut *member };

        loop {
            let range = group_range(member, group);
            if range.len <= aligned_len {
                break;
            }

            // Evicting the range's last component lowers its trust to at most the new length,
            // so once we're down to `aligned_len` the range is fully trusted again.
            let last_index = range.end() - 1;
            let last = member.dense().expect("group members are SparseSet")[last_index];

            member.move_to(last, GroupRegion::Unevaluated);
        }
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
    use crate::{track, Component, EntityId, Get, IntoIter, View, ViewMut, World};
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

    struct TrackedA(u32);
    impl Component for TrackedA {
        type Tracking = track::All;
    }

    struct TrackedB;
    impl Component for TrackedB {
        type Tracking = track::All;
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
            let Some(range) = groups.ranges.first() else {
                continue;
            };
            let dense = storage.dense().unwrap();

            #[cfg(debug_assertions)]
            groups.assert_invariants(dense.len());
            assert_eq!(groups.unevaluated_start, dense.len());
            assert_eq!(range.trust, range.len);

            for member_id in range.group.iter() {
                // SAFE: same as above.
                let member = unsafe { &*storages[member_id].0 }.borrow().unwrap();
                let member_range = member.groups().unwrap().range(&range.group).unwrap();
                let member_dense = member.dense().unwrap();

                assert_eq!(
                    member_dense[member_range.start..member_range.end()],
                    dense[range.start..range.end()]
                );
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
}
