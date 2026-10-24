//! Entity and tracking operations only visit the storages they need. Per entity work only reaches
//! components and custom storages, insertion and modification tracking reaches uniques too.

use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::component::Unique;
use crate::entity_id::EntityId;
use crate::storage::{Storage, StorageId};
use crate::tracking::TrackingTimestamp;
use crate::unique::UniqueStorage;
use crate::world::World;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

/// A call an operation made on a [`Recorder`], with its arguments.
#[derive(Debug, PartialEq)]
enum Call {
    Delete(EntityId, u64),
    Clear(u64),
    ClearInserted(u64),
    ClearModified(u64),
    ClearRemovedAndDeleted,
    ClearRemovedAndDeletedOlderThan(u64),
    Move(EntityId, EntityId, u64, u64),
    Clone(EntityId, EntityId, u64),
}

/// Custom storage that records every call it receives.
struct Recorder(Arc<Mutex<Vec<Call>>>);

impl Recorder {
    fn record(&self, call: Call) {
        self.0.lock().unwrap().push(call);
    }
}

impl Storage for Recorder {
    fn delete(&mut self, entity: EntityId, current: TrackingTimestamp) {
        self.record(Call::Delete(entity, current.get()));
    }

    fn clear(&mut self, current: TrackingTimestamp) {
        self.record(Call::Clear(current.get()));
    }

    fn clear_all_inserted(&mut self, current: TrackingTimestamp) {
        self.record(Call::ClearInserted(current.get()));
    }

    fn clear_all_modified(&mut self, current: TrackingTimestamp) {
        self.record(Call::ClearModified(current.get()));
    }

    fn clear_all_removed_and_deleted(&mut self) {
        self.record(Call::ClearRemovedAndDeleted);
    }

    fn clear_all_removed_and_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.record(Call::ClearRemovedAndDeletedOlderThan(timestamp.get()));
    }

    fn move_component_from(
        &mut self,
        _other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        current: TrackingTimestamp,
        other_current: TrackingTimestamp,
    ) {
        self.record(Call::Move(from, to, current.get(), other_current.get()));
    }

    fn clone_component_to(
        &self,
        _other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        other_current: TrackingTimestamp,
    ) {
        self.record(Call::Clone(from, to, other_current.get()));
    }
}

const RECORDER: StorageId = StorageId::Custom(0);

/// Returns a world holding a [`Recorder`], and the calls it records.
fn recorder_world() -> (World, Arc<Mutex<Vec<Call>>>) {
    let world = World::new_for_test();
    let calls = Arc::new(Mutex::new(Vec::new()));
    world
        .add_custom_storage(RECORDER, Recorder(calls.clone()))
        .unwrap();

    (world, calls)
}

/// Returns the calls recorded so far and forgets them.
fn take(calls: &Mutex<Vec<Call>>) -> Vec<Call> {
    core::mem::take(&mut *calls.lock().unwrap())
}

/// Returns the timestamp the next operation will use as its current one.
fn next_timestamp(all_storages: &AllStorages) -> u64 {
    all_storages.get_tracking_timestamp().get()
}

struct Value(u32);

impl Unique for Value {}

/// Asserts that deleting an entity deletes it from custom storages, at the current timestamp.
#[test]
fn delete_entity_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let entity = all_storages.add_entity(());
    let now = next_timestamp(all_storages);

    all_storages.delete_entity(entity);

    assert_eq!(take(&calls), [Call::Delete(entity, now)]);
}

/// Asserts that a failed deletion doesn't reach custom storages.
#[test]
fn delete_dead_entity_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let entity = all_storages.add_entity(());
    all_storages.delete_entity(entity);
    take(&calls);

    all_storages.delete_entity(entity);

    assert_eq!(take(&calls), []);
}

/// Asserts that bulk stripping deletes every entity from custom storages, at the same timestamp.
#[test]
fn bulk_strip_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let first = all_storages.add_entity(());
    let second = all_storages.add_entity(());
    let now = next_timestamp(all_storages);

    all_storages.bulk_strip([first, second]);

    assert_eq!(
        take(&calls),
        [Call::Delete(first, now), Call::Delete(second, now)]
    );
}

/// Asserts that stripping in parallel deletes every entity from custom storages, at the same
/// timestamp.
#[cfg(all(feature = "parallel", not(feature = "thread_local")))]
#[test]
fn par_strip_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let first = all_storages.add_entity(());
    let second = all_storages.add_entity(());
    let now = next_timestamp(all_storages);

    all_storages.par_strip([first, second]);

    assert_eq!(
        take(&calls),
        [Call::Delete(first, now), Call::Delete(second, now)]
    );
}

/// Asserts that retaining other storages deletes the entity from custom storages.
#[test]
fn retain_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let entity = all_storages.add_entity(());
    let now = next_timestamp(all_storages);

    all_storages.retain_storage_by_id(entity, &[]);

    assert_eq!(take(&calls), [Call::Delete(entity, now)]);
}

/// Asserts that retaining a custom storage leaves it alone.
#[test]
fn retain_excluded_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let entity = all_storages.add_entity(());

    all_storages.retain_storage_by_id(entity, &[RECORDER]);

    assert_eq!(take(&calls), []);
}

/// Asserts that clearing the world clears custom storages, at the current timestamp.
#[test]
fn clear_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let now = next_timestamp(all_storages);

    all_storages.clear();

    assert_eq!(take(&calls), [Call::Clear(now)]);
}

/// Asserts that clearing deletion and removal tracking reaches custom storages.
#[test]
fn clear_removed_calls() {
    let (mut world, calls) = recorder_world();

    world.all_storages.get_mut().clear_all_removed_and_deleted();

    assert_eq!(take(&calls), [Call::ClearRemovedAndDeleted]);
}

/// Asserts that clearing old deletion and removal tracking reaches custom storages with the
/// requested timestamp.
#[test]
fn clear_removed_older_calls() {
    let (mut world, calls) = recorder_world();

    world
        .all_storages
        .get_mut()
        .clear_all_removed_and_deleted_older_than_timestamp(TrackingTimestamp::new(5));

    assert_eq!(take(&calls), [Call::ClearRemovedAndDeletedOlderThan(5)]);
}

/// Asserts that clearing insertion tracking reaches custom storages, at the current timestamp.
#[test]
fn clear_inserted_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let now = next_timestamp(all_storages);

    all_storages.clear_all_inserted();

    assert_eq!(take(&calls), [Call::ClearInserted(now)]);
}

/// Asserts that clearing modification tracking reaches custom storages, at the current timestamp.
#[test]
fn clear_modified_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let now = next_timestamp(all_storages);

    all_storages.clear_all_modified();

    assert_eq!(take(&calls), [Call::ClearModified(now)]);
}

/// Asserts that clearing insertion and modification tracking together reaches custom storages
/// for both, at the current timestamp.
#[test]
fn clear_inserted_and_modified_calls() {
    let (mut world, calls) = recorder_world();
    let all_storages = world.all_storages.get_mut();
    let now = next_timestamp(all_storages);

    all_storages.clear_all_inserted_and_modified();

    assert_eq!(
        take(&calls),
        [Call::ClearInserted(now), Call::ClearModified(now)]
    );
}

/// Asserts that moving an entity to another world moves it out of custom storages, with both
/// worlds' current timestamps.
#[test]
fn move_entity_calls() {
    let (mut world, calls) = recorder_world();
    let mut other = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    let other_storages = other.all_storages.get_mut();
    let entity = all_storages.add_entity(());
    let now = next_timestamp(all_storages);
    let other_now = next_timestamp(other_storages);

    all_storages.move_entity(other_storages, entity);

    assert_eq!(take(&calls), [Call::Move(entity, entity, now, other_now)]);
}

/// Asserts that moving components to another world's entity moves them out of custom storages,
/// with both worlds' current timestamps.
#[test]
fn move_components_calls() {
    let (mut world, calls) = recorder_world();
    let mut other = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    let other_storages = other.all_storages.get_mut();
    let from = all_storages.add_entity(());
    other_storages.add_entity(());
    let to = other_storages.add_entity(());
    let now = next_timestamp(all_storages);
    let other_now = next_timestamp(other_storages);

    all_storages.move_components(other_storages, from, to);

    assert_eq!(take(&calls), [Call::Move(from, to, now, other_now)]);
}

/// Asserts that cloning an entity to another world clones it from custom storages, with the
/// other world's current timestamp.
#[test]
fn clone_entity_calls() {
    let (mut world, calls) = recorder_world();
    let mut other = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    let other_storages = other.all_storages.get_mut();
    let entity = all_storages.add_entity(());
    let other_now = next_timestamp(other_storages);

    all_storages.clone_entity_to(other_storages, entity);

    assert_eq!(take(&calls), [Call::Clone(entity, entity, other_now)]);
}

/// Asserts that cloning components to another world's entity clones them from custom storages,
/// with the other world's current timestamp.
#[test]
fn clone_components_calls() {
    let (mut world, calls) = recorder_world();
    let mut other = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    let other_storages = other.all_storages.get_mut();
    let from = all_storages.add_entity(());
    other_storages.add_entity(());
    let to = other_storages.add_entity(());
    let other_now = next_timestamp(other_storages);

    all_storages.clone_components_to(other_storages, from, to);

    assert_eq!(take(&calls), [Call::Clone(from, to, other_now)]);
}

/// Asserts that cloning an entity fails when a custom storage is borrowed exclusively, skipping
/// uniques doesn't skip custom storages.
#[test]
#[should_panic]
fn clone_entity_borrowed_custom() {
    let (mut world, _calls) = recorder_world();
    let mut other = World::new_for_test();
    let entity = world.add_entity(());
    let all_storages = world.all_storages().unwrap();

    let _recorder = all_storages.custom_storage_mut_by_id(&RECORDER).unwrap();

    all_storages.clone_entity_to(other.all_storages.get_mut(), entity);
}

/// Asserts that cloning components fails when a custom storage is borrowed exclusively, skipping
/// uniques doesn't skip custom storages.
#[test]
#[should_panic]
fn clone_components_borrowed_custom() {
    let (mut world, _calls) = recorder_world();
    let mut other = World::new_for_test();
    let from = world.add_entity(());
    let to = other.add_entity(());
    let all_storages = world.all_storages().unwrap();

    let _recorder = all_storages.custom_storage_mut_by_id(&RECORDER).unwrap();

    all_storages.clone_components_to(other.all_storages.get_mut(), from, to);
}

/// Asserts that a unique keeps its value through entity operations, deletion, stripping,
/// retention, clearing and moves included.
#[test]
fn entity_ops_keep_unique() {
    let mut world = World::new_for_test();
    let mut other = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    all_storages.add_unique(Value(7));
    let entities = [(); 5].map(|()| all_storages.add_entity(()));

    all_storages.delete_entity(entities[0]);
    all_storages.strip(entities[1]);
    all_storages.bulk_strip([entities[2]]);
    all_storages.retain_storage_by_id(entities[3], &[]);
    all_storages.move_entity(other.all_storages.get_mut(), entities[4]);
    all_storages.clear();

    let unique = all_storages
        .unique_storage::<UniqueStorage<Value>>()
        .unwrap();
    assert_eq!(unique.value.0, 7);
}

/// Asserts that clearing insertion tracking still reaches uniques, it moves their insertion to
/// the current timestamp.
#[test]
fn clear_inserted_unique() {
    let mut world = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    all_storages.add_unique(Value(0));
    all_storages.get_current();
    let now = next_timestamp(all_storages);

    all_storages.clear_all_inserted();

    let unique = all_storages
        .unique_storage::<UniqueStorage<Value>>()
        .unwrap();
    assert_eq!(unique.insert.get(), now);
}

/// Asserts that clearing modification tracking still reaches uniques, it moves their
/// modification to the current timestamp.
#[test]
fn clear_modified_unique() {
    let mut world = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    all_storages.add_unique(Value(0));
    all_storages.get_current();
    let now = next_timestamp(all_storages);

    all_storages.clear_all_modified();

    let unique = all_storages
        .unique_storage::<UniqueStorage<Value>>()
        .unwrap();
    assert_eq!(unique.modification.get(), now);
}

/// Asserts that clearing insertion and modification tracking together still reaches uniques for
/// both.
#[test]
fn clear_inserted_and_modified_unique() {
    let mut world = World::new_for_test();
    let all_storages = world.all_storages.get_mut();
    all_storages.add_unique(Value(0));
    all_storages.get_current();
    let now = next_timestamp(all_storages);

    all_storages.clear_all_inserted_and_modified();

    let unique = all_storages
        .unique_storage::<UniqueStorage<Value>>()
        .unwrap();
    assert_eq!((unique.insert.get(), unique.modification.get()), (now, now));
}
