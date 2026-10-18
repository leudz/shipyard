mod declaration;
mod overlap;
#[cfg(feature = "thread_local")]
mod thread_local;

use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::{Component, EntityId, Get, IntoIter, View, World, track};
use alloc::sync::Arc;
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

fn id<T: Component + Send + Sync>() -> StorageId {
    StorageId::of::<SparseSet<T>>()
}

/// Returns `ids` sorted, a group's identity.
fn group_ids(ids: &[StorageId]) -> Vec<StorageId> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids
}

/// Returns the members of each of `T`'s groups, in range order.
fn groups_of<T: Component + Send + Sync>(world: &World) -> Vec<Vec<StorageId>> {
    let view = world.borrow::<View<'_, T>>().unwrap();

    view.groups
        .ranges
        .iter()
        .map(|range| range.group.to_vec())
        .collect()
}

/// Returns the entities of `T`'s range for the group made of `ids`, sorted.
fn range_of<T: Component + Send + Sync>(world: &World, ids: &[StorageId]) -> Vec<EntityId> {
    let view = world.borrow::<View<'_, T>>().unwrap();
    let ids = group_ids(ids);
    let range = view
        .groups
        .ranges
        .iter()
        .find(|range| range.group[..] == ids[..])
        .expect("the group is attached to the storage");

    let mut entities = view.dense[range.start..range.end()].to_vec();
    entities.sort_unstable();
    entities
}

/// Returns `true` when `T` has no unevaluated component.
fn is_evaluated<T: Component + Send + Sync>(world: &World) -> bool {
    let view = world.borrow::<View<'_, T>>().unwrap();

    view.groups.unevaluated_range(view.len()).is_empty()
}
