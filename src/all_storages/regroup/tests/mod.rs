mod analysis;
mod churn;
mod clone;
mod declaration;
mod discovery;
mod guarded;
mod layout;
mod original_row;
mod promotion;
mod regroup_set;
mod removals;
mod selected;

use super::Selected;
use super::analysis::{Discovery, Frozen, Plan, analyze};
use crate::all_storages::{LockPresent, ThreadIdPresent};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::IntoIter;
use crate::sparse_set::{GroupRegion, SparseSet};
use crate::storage::{Storage, StorageId};
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::{World, WorldBuilder};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Clone, Debug, PartialEq)]
struct B(u32);
impl Component for B {
    type Tracking = track::Untracked;
}

#[derive(Clone)]
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

        // Every component has to stay reachable from its entity.
        let sparse = storage.sparse_array().unwrap();
        for (index, &entity) in dense.iter().enumerate() {
            assert_eq!(sparse.get(entity).map(EntityId::uindex), Some(index));
        }

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

/// Panics unless every component of a grouped storage sits where its entity's components say:
/// in the union of the `declared` groups it owns every member of that connect to the storage,
/// or in the ungrouped region when none holds the storage.
///
/// The expected union comes from component ownership alone, never from regroup's own discovery.
/// Every grouped storage has to be fully evaluated, see [`assert_regrouped`].
fn assert_memberships(world: &World, declared: &[&[StorageId]]) {
    let all_storages = world.all_storages().unwrap();
    let storages = all_storages.storages.read();
    // SAFE: `sbox.0` comes from a `Box` that lives as long as `all_storages`,
    // and `borrow` checks no one borrows the storage exclusively.
    let storage = |storage_id: &StorageId| unsafe { &*storages[storage_id].0 }.borrow().unwrap();

    for (storage_id, sbox) in storages.iter() {
        // SAFE: same as above.
        let grouped = unsafe { &*sbox.0 }.borrow().unwrap();
        let Some(groups) = grouped.groups().filter(|groups| groups.has_ranges()) else {
            continue;
        };

        for (index, &entity) in grouped.dense().unwrap().iter().enumerate() {
            let expected = expected_union(declared, *storage_id, |member_id| {
                storage(member_id).sparse_array().unwrap().contains(entity)
            });

            let actual = match groups.region_of(index) {
                GroupRegion::Range(group) => Some(group.to_vec()),
                GroupRegion::Ungrouped => None,
                GroupRegion::Unevaluated => panic!("{entity:?} is unevaluated"),
            };

            assert_eq!(actual, expected, "{entity:?} in {storage_id:?}");
        }
    }
}

/// Returns a world with `count` entities owning `A(i)` and `B(i)`, grouped as `(A, B)`.
fn pairs(count: u32) -> (World, Vec<EntityId>) {
    pairs_in(World::builder_for_test().with_group::<(A, B)>(), count)
}

/// Returns the world `builder` makes, with `count` entities owning `A(i)` and `B(i)`, regrouped.
fn pairs_in(
    builder: WorldBuilder<LockPresent, ThreadIdPresent>,
    count: u32,
) -> (World, Vec<EntityId>) {
    let mut world = builder.build();
    let entities = (0..count).map(|i| world.add_entity((A(i), B(i)))).collect();
    world.regroup();

    (world, entities)
}

/// The `(A, B)` group, for [`assert_memberships`] and [`assert_plan`].
fn ab() -> [StorageId; 2] {
    [id::<A>(), id::<B>()]
}

/// Returns the sorted members of the union an entity has to be grouped in from `storage_id`,
/// `None` when it stays ungrouped there.
///
/// That's the union of the `declared` groups the entity owns every member of that connect to
/// the storage, `owns` telling whether it owns a component in a storage.
/// It comes from component ownership alone, never from regroup's own discovery.
fn expected_union(
    declared: &[&[StorageId]],
    storage_id: StorageId,
    owns: impl Fn(&StorageId) -> bool,
) -> Option<Vec<StorageId>> {
    let satisfied: Vec<&[StorageId]> = declared
        .iter()
        .copied()
        .filter(|group| group.iter().all(&owns))
        .collect();

    // Satisfied groups sharing a storage with the union so far join it.
    let mut union = vec![storage_id];
    while let Some(group) = satisfied.iter().find(|group| {
        group.iter().any(|id| union.contains(id)) && group.iter().any(|id| !union.contains(id))
    }) {
        for &id in group.iter() {
            if !union.contains(&id) {
                union.push(id);
            }
        }
    }

    (union.len() > 1).then(|| sorted_ids(union))
}

/// Analyzes `world` and returns the plan.
fn analyze_world(world: &World) -> Option<Plan> {
    let mut all_storages = world.all_storages_mut().unwrap();

    analyze(&mut Selected::all(all_storages.storages.get_mut()))
}

/// Analyzes `world` and returns the plan, panicking unless:
/// - every group keeps the prefix all its members trust,
/// - its `leaving` count is the number of entities of that prefix that belong elsewhere,
/// - every storage's `promoted` count is the number of its ungrouped components
///   that belong to a group,
/// - every group ends up with exactly the entities that belong to it,
/// - every new union is needed by an entity, and attached to all its members.
///
/// Where components belong comes from their ownership and the `declared` groups,
/// see [`expected_union`].
fn assert_plan(world: &World, declared: &[&[StorageId]]) -> Plan {
    let mut all_storages = world.all_storages_mut().unwrap();
    let mut selected = Selected::all(all_storages.storages.get_mut());

    let frozen = Frozen::new(&selected);
    let existing: Vec<Arc<[StorageId]>> = frozen
        .memberships
        .storages
        .iter()
        .flat_map(|membership| membership.groups.ranges.iter())
        .map(|range| range.group.clone())
        .collect();
    drop(frozen);

    let plan = analyze(&mut selected).expect("the world has something to regroup");

    let frozen = Frozen::new(&selected);
    let memberships = &frozen.memberships;
    // Both counted in each group's first member.
    let mut expected_lens: BTreeMap<Vec<StorageId>, usize> = BTreeMap::new();
    let mut expected_leaving: BTreeMap<Vec<StorageId>, usize> = BTreeMap::new();

    for (membership, dense) in memberships.storages.iter().zip(&frozen.dense) {
        let storage_id = membership.storage_id;
        let groups = membership.groups;
        let mut promoted = 0;

        for (row, &entity) in dense.iter().enumerate() {
            let expected = expected_union(declared, storage_id, |member_id| {
                memberships.get(*member_id).sparse.contains(entity)
            });

            if let Some(range_index) = groups.range_index_of(row) {
                let range = &groups.ranges[range_index];
                let kept = plan.groups[plan.group_index(&range.group)].kept;
                let leaves = range.ring().logical_of(row) < kept
                    && expected.as_deref() != Some(&range.group[..]);

                if leaves && range.group[0] == storage_id {
                    *expected_leaving.entry(range.group.to_vec()).or_default() += 1;
                }
            } else if row < groups.unevaluated_start && expected.is_some() {
                promoted += 1;
            }

            if let Some(expected) = expected
                && expected[0] == storage_id
            {
                *expected_lens.entry(expected).or_default() += 1;
            }
        }

        assert_eq!(
            plan.storage(storage_id).promoted,
            promoted,
            "promoted in {storage_id:?}"
        );
    }

    for group_plan in &plan.groups {
        let group = &group_plan.group;
        let expected_len = expected_lens.remove(&group[..]).unwrap_or(0);
        let leaving = expected_leaving.remove(&group[..]).unwrap_or(0);
        let trust = group
            .iter()
            .map(|&member_id| {
                memberships
                    .get(member_id)
                    .groups
                    .range(group)
                    .unwrap()
                    .trust
            })
            .min();

        assert_eq!(Some(group_plan.kept), trust, "kept in {group:?}");
        assert_eq!(group_plan.leaving, leaving, "leaving in {group:?}");
        assert_eq!(
            group_plan.kept - group_plan.leaving + group_plan.incoming,
            expected_len,
            "len of {group:?}"
        );
        assert!(
            expected_len > 0 || existing.iter().any(|existing| Arc::ptr_eq(existing, group)),
            "{group:?} isn't needed"
        );
    }
    assert!(expected_lens.is_empty(), "no plan for {expected_lens:?}");
    assert!(
        expected_leaving.is_empty(),
        "no plan for {expected_leaving:?}"
    );
    drop(frozen);

    plan
}

/// Returns `entities` sorted.
fn sorted(mut entities: Vec<EntityId>) -> Vec<EntityId> {
    entities.sort_unstable();
    entities
}

/// Attaches the group of `T` and `U` to both storages without regrouping.
///
/// Lets tests attach a group to storages already holding components, the public API can't.
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
    let mut selected = Selected::all(all_storages.storages.get_mut());
    for &member_id in group.iter() {
        selected
            .get_mut(member_id)
            .groups_mut()
            .unwrap()
            .attach(group.clone(), member_id);
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

    let mut entities = view.dense[view.groups.ungrouped_range(view.len())].to_vec();
    entities.sort_unstable();
    entities
}

/// Returns the entities in `T`'s unevaluated region, sorted.
fn unevaluated_of<T: Component + Send + Sync>(world: &World) -> Vec<EntityId> {
    let view = world.borrow::<View<'_, T>>().unwrap();

    let mut entities = view.dense[view.groups.unevaluated_range(view.len())].to_vec();
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

/// What regroup can change in `T`'s storage: its ids in physical order, its ranges as
/// `(start, len, shift, trust)` and where its unevaluated region starts.
type Snapshot = (Vec<EntityId>, Vec<(usize, usize, isize, usize)>, usize);

fn snapshot<T: Component + Send + Sync>(world: &World) -> Snapshot {
    let view = world.borrow::<View<'_, T>>().unwrap();
    let ranges = view
        .groups
        .ranges
        .iter()
        .map(|range| (range.start, range.len, range.shift, range.trust))
        .collect();

    (view.dense.to_vec(), ranges, view.groups.unevaluated_start)
}

/// Returns a world with the disjoint AB and CD, three unevaluated entities in each.
fn two_sets() -> World {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    for i in 0..3 {
        world.add_entity((A(i), B(i)));
        world.add_entity((C, D));
    }

    world
}
