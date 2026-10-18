mod declaration;
mod discovery;
mod promotion;
mod repair;

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

/// Returns `entities` sorted.
fn sorted(mut entities: Vec<EntityId>) -> Vec<EntityId> {
    entities.sort_unstable();
    entities
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

/// Returns `ids` sorted.
fn sorted_ids(mut ids: Vec<StorageId>) -> Vec<StorageId> {
    ids.sort_unstable();
    ids
}

fn id<T: Component + Send + Sync>() -> StorageId {
    StorageId::of::<SparseSet<T>>()
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

/// Returns `T`'s entities in physical order.
fn dense_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
    world.borrow::<View<'_, T>>().unwrap().dense.clone()
}
