//! Focused Miri scenarios for entity allocation.
//!
//! `Entities` indexes its vector unchecked while generating and deleting entities, following a
//! recycle list threaded through the deleted slots. These tests walk that list with a few live,
//! deleted and recycled ids.

use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::storage::Storage;
use crate::tracking::TrackingTimestamp;
use alloc::vec::Vec;

/// Returns the alive entities, in index order.
fn alive(entities: &Entities) -> Vec<EntityId> {
    entities.iter().collect()
}

/// Asserts that deleted entities are recycled in deletion order with a bumped generation, that
/// stale ids don't read as alive, and that iteration skips the holes deleted entities leave.
#[test]
fn recycle() {
    let mut entities = Entities::new();
    let e: Vec<EntityId> = (0..4).map(|_| entities.generate()).collect();

    assert!(entities.delete_unchecked(e[1]));
    assert!(entities.delete_unchecked(e[2]));
    assert!(!entities.delete_unchecked(e[1]));
    assert!(!entities.is_alive(e[1]));
    assert_eq!(alive(&entities), [e[0], e[3]]);

    let recycled1 = entities.generate();
    let recycled2 = entities.generate();
    let fresh = entities.generate();
    assert_eq!((recycled1.index(), recycled1.generation()), (1, 1));
    assert_eq!((recycled2.index(), recycled2.generation()), (2, 1));
    assert_eq!((fresh.index(), fresh.generation()), (4, 0));

    assert!(!entities.is_alive(e[1]));
    assert!(entities.is_alive(recycled1));
    assert_eq!(alive(&entities), [e[0], recycled1, recycled2, e[3], fresh]);
}

/// Asserts that spawning past the end leaves the skipped indexes as recyclable holes: iteration
/// skips them, and the next generated entities fill them before the vector grows.
#[test]
fn spawn_holes() {
    let mut entities = Entities::new();
    let spawned = EntityId::new_from_index_and_gen(3, 0);

    assert!(entities.spawn(spawned));
    assert_eq!(alive(&entities), [spawned]);

    let filled: Vec<EntityId> = (0..3).map(|_| entities.generate()).collect();
    let fresh = entities.generate();
    let filled_indexes: Vec<u64> = filled.iter().map(|entity| entity.index()).collect();
    assert_eq!(filled_indexes, [0, 1, 2]);
    assert_eq!(fresh.index(), 4);

    assert_eq!(
        alive(&entities),
        [filled[0], filled[1], filled[2], spawned, fresh]
    );
}

/// Asserts that bulk generation appends fresh entities past the end without touching the recycle
/// list, which the next single generation still reuses.
#[test]
fn bulk() {
    let mut entities = Entities::new();
    let e0 = entities.generate();
    let e1 = entities.generate();
    entities.delete_unchecked(e0);

    let bulk = entities.bulk_generate(2).to_vec();
    let bulk_indexes: Vec<u64> = bulk.iter().map(|entity| entity.index()).collect();
    assert_eq!(bulk_indexes, [2, 3]);

    let recycled = entities.generate();
    assert_eq!((recycled.index(), recycled.generation()), (0, 1));
    assert_eq!(alive(&entities), [recycled, e1, bulk[0], bulk[1]]);
}

/// Asserts that clearing kills every entity and turns the whole vector into the recycle list:
/// the next generations reuse every index with a bumped generation before the vector grows.
#[test]
fn clear() {
    let mut entities = Entities::new();
    let e: Vec<EntityId> = (0..3).map(|_| entities.generate()).collect();
    entities.delete_unchecked(e[1]);

    Storage::clear(&mut entities, TrackingTimestamp::new(0));
    assert!(alive(&entities).is_empty());
    assert!(!entities.is_alive(e[0]));

    let mut recycled: Vec<EntityId> = (0..3).map(|_| entities.generate()).collect();
    recycled.sort_unstable_by_key(|entity| entity.index());
    let recycled: Vec<(u64, u16)> = recycled
        .iter()
        .map(|entity| (entity.index(), entity.generation()))
        .collect();
    assert_eq!(recycled, [(0, 1), (1, 2), (2, 1)]);
    assert_eq!(entities.generate().index(), 3);
}
