//! Entity deletion: deleting 10,000 of 100,000 entities one at a time, with and without unrelated storages.
//!
//! `World::delete_entity` visits every storage of the world for each deleted entity. These two cases isolate
//! what unrelated storages add to that cost.
//!
//! Both cases start from the same candidates: 100,000 entities owning `Value` alone, created first so their ids
//! and the deletion order match. The deleted entities are a fixed seeded selection. Then come 31 owner
//! entities. With `unrelated-31` each owns one of 31 distinct components, so 31 more storages exist and stay
//! populated during the measurement. With `unrelated-0` they own nothing, so both worlds allocate as many
//! entities. Nothing is grouped or tracked.
//!
//! Each function is a complete benchmark for one case, with its own setup, validation and measurement.
//!
//! - `cargo bench --bench entity_deletion` runs both cases.
//! - `cargo bench --bench entity_deletion -- --test` validates both cases and runs each measurement once.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use shipyard::{Component, EntityId, Get, View, World, track};
use std::collections::HashSet;

/// Candidate entities, all owning `Value`.
const ENTITIES: u32 = 100_000;
/// Deleted candidates.
const SELECTED: u32 = 10_000;
/// Owner entities created after the candidates.
const OWNERS: usize = 31;
/// Seed of the selection shuffle.
const SEED: u64 = 0x0DE1_E7E5_5EED_2026;

/// 16-byte component of every candidate, different for each one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Value([u32; 4]);

impl Component for Value {
    type Tracking = track::Untracked;
}

/// Starting `Value` of candidate `entity`.
fn value(entity: u32) -> Value {
    Value([entity; 4])
}

/// The first `SELECTED` entries of a fixed seeded shuffle of the `ENTITIES` candidates.
fn selection() -> Vec<u32> {
    let mut entities: Vec<u32> = (0..ENTITIES).collect();

    // Fisher-Yates, driven by a splitmix64 counter.
    let mut counter = SEED;
    for last in (1..entities.len()).rev() {
        counter = counter.wrapping_add(1);
        let other = (splitmix64(counter) % (last as u64 + 1)) as usize;
        entities.swap(last, other);
    }

    entities.truncate(SELECTED as usize);
    entities
}

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}

/// Asserts that only the selected candidates were deleted,
/// and that every other candidate still owns exactly its starting `Value`.
fn check_candidates(world: &mut World, candidates: &[EntityId], selected: &HashSet<u32>) {
    for (entity, &id) in candidates.iter().enumerate() {
        let deleted = selected.contains(&(entity as u32));
        assert_eq!(world.is_entity_alive(id), !deleted, "entity {entity}");
    }

    let values = world.borrow::<View<Value>>().unwrap();
    assert_eq!(values.len(), (ENTITIES - SELECTED) as usize);
    for (entity, &id) in candidates.iter().enumerate() {
        if !selected.contains(&(entity as u32)) {
            let expected = value(entity as u32);
            assert_eq!((&values).get(id).ok(), Some(&expected), "entity {entity}");
        }
    }
}

macro_rules! unrelated_components {
    ($($name:ident = $index:literal),* $(,)?) => {
        $(
            /// Component only one owner has, never a candidate.
            #[derive(Clone, Copy, Debug, PartialEq)]
            struct $name(u32);

            impl Component for $name {
                type Tracking = track::Untracked;
            }
        )*

        /// Gives each unrelated component to its own new entity, returning the owners in component order.
        fn add_unrelated_owners(world: &mut World) -> Vec<EntityId> {
            vec![$(world.add_entity(($name($index),))),*]
        }

        /// Asserts that each unrelated storage still holds only its owner's component, with its value.
        fn check_unrelated_owners(world: &World, owners: &[EntityId]) {
            $(
                let view = world.borrow::<View<$name>>().unwrap();
                assert_eq!(view.len(), 1, stringify!($name));
                assert_eq!((&view).get(owners[$index]).ok(), Some(&$name($index)));
            )*
        }
    };
}

unrelated_components!(
    U0 = 0,
    U1 = 1,
    U2 = 2,
    U3 = 3,
    U4 = 4,
    U5 = 5,
    U6 = 6,
    U7 = 7,
    U8 = 8,
    U9 = 9,
    U10 = 10,
    U11 = 11,
    U12 = 12,
    U13 = 13,
    U14 = 14,
    U15 = 15,
    U16 = 16,
    U17 = 17,
    U18 = 18,
    U19 = 19,
    U20 = 20,
    U21 = 21,
    U22 = 22,
    U23 = 23,
    U24 = 24,
    U25 = 25,
    U26 = 26,
    U27 = 27,
    U28 = 28,
    U29 = 29,
    U30 = 30,
);

/// Deleting 10,000 of 100,000 entities, without unrelated storages.
/// Every candidate owns `Value`, the 31 owners own nothing.
///
/// Measured: `World::delete_entity` on every selected entity, in selection order.
/// Each iteration starts from a freshly populated world.
/// Populating it, selecting the entities and dropping the world happen outside timing.
fn unrelated_0(c: &mut Criterion) {
    /// A world with every candidate then the empty owners, returning both in creation order.
    fn populate() -> (World, Vec<EntityId>, Vec<EntityId>) {
        let mut world = World::new();
        let candidates: Vec<_> = world
            .bulk_add_entity((0..ENTITIES).map(|entity| (value(entity),)))
            .collect();
        let owners: Vec<_> = (0..OWNERS).map(|_| world.add_entity(())).collect();

        (world, candidates, owners)
    }

    // Generated and validated the first time Criterion runs the case, so a filtered out case never builds it.
    let mut fixture: Option<Vec<u32>> = None;

    let mut group = c.benchmark_group("entity-deletion");
    group.bench_function("unrelated-0", |b| {
        let selected = fixture.get_or_insert_with(|| {
            // No entity may come twice.
            let selected = selection();
            let distinct: HashSet<_> = selected.iter().copied().collect();
            assert_eq!(distinct.len(), SELECTED as usize);

            // One deletion pass: each selected entity must be deleted by its own call.
            let (mut world, candidates, owners) = populate();
            for &entity in &selected {
                let deleted = world.delete_entity(candidates[entity as usize]);
                assert!(deleted, "entity {entity}");
            }

            check_candidates(&mut world, &candidates, &distinct);
            assert!(owners.iter().all(|&id| world.is_entity_alive(id)));

            selected
        });

        b.iter_batched(
            // A populated world and the ids to delete.
            || {
                let (world, candidates, _) = populate();
                let selected: Vec<_> = selected
                    .iter()
                    .map(|&entity| candidates[entity as usize])
                    .collect();
                (world, selected)
            },
            |(mut world, selected)| {
                for &id in &selected {
                    world.delete_entity(id);
                }

                // Returned so it's dropped after timing.
                (world, selected)
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

/// Deleting 10,000 of 100,000 entities, with 31 unrelated storages.
/// Every candidate owns `Value`, each of the 31 owners owns a distinct component no candidate owns.
///
/// Measured: `World::delete_entity` on every selected entity, in selection order.
/// Each iteration starts from a freshly populated world.
/// Populating it, selecting the entities and dropping the world happen outside timing.
fn unrelated_31(c: &mut Criterion) {
    /// A world with every candidate then the owners, returning both in creation order.
    fn populate() -> (World, Vec<EntityId>, Vec<EntityId>) {
        let mut world = World::new();
        let candidates: Vec<_> = world
            .bulk_add_entity((0..ENTITIES).map(|entity| (value(entity),)))
            .collect();
        let owners = add_unrelated_owners(&mut world);

        (world, candidates, owners)
    }

    // Generated and validated the first time Criterion runs the case, so a filtered out case never builds it.
    let mut fixture: Option<Vec<u32>> = None;

    let mut group = c.benchmark_group("entity-deletion");
    group.bench_function("unrelated-31", |b| {
        let selected = fixture.get_or_insert_with(|| {
            // No entity may come twice.
            let selected = selection();
            let distinct: HashSet<_> = selected.iter().copied().collect();
            assert_eq!(distinct.len(), SELECTED as usize);

            // One deletion pass: each selected entity must be deleted by its own call.
            let (mut world, candidates, owners) = populate();
            assert_eq!(owners.len(), OWNERS);
            for &entity in &selected {
                let deleted = world.delete_entity(candidates[entity as usize]);
                assert!(deleted, "entity {entity}");
            }

            check_candidates(&mut world, &candidates, &distinct);
            assert!(owners.iter().all(|&id| world.is_entity_alive(id)));
            check_unrelated_owners(&world, &owners);

            selected
        });

        b.iter_batched(
            // A populated world and the ids to delete.
            || {
                let (world, candidates, _) = populate();
                let selected: Vec<_> = selected
                    .iter()
                    .map(|&entity| candidates[entity as usize])
                    .collect();
                (world, selected)
            },
            |(mut world, selected)| {
                for &id in &selected {
                    world.delete_entity(id);
                }

                // Returned so it's dropped after timing.
                (world, selected)
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

criterion_group!(entity_deletion, unrelated_0, unrelated_31);
criterion_main!(entity_deletion);
