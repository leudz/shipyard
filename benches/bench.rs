use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use shipyard::{track, Component, EntitiesView, EntityId, Group, IntoIter, View, ViewMut, World};
use std::hint::black_box;

// Lifecycle benchmarks operate on enough items to amortize Criterion and World API overhead.
// Iteration benchmarks use a larger population so that traversal dominates setup and borrowing.
const MUTATION_COUNT: usize = 10_000;
const ITERATION_COUNT: usize = 100_000;
const SPARSE_STEP: usize = 10;
const LARGE_WORDS: usize = 32;

#[derive(Component, Clone, Copy)]
struct Position(f32, f32);

// Deliberately large enough to make moves, swaps, and traversal bandwidth visible.
#[derive(Component)]
struct Large([u64; LARGE_WORDS]);

#[derive(Component, Clone, Copy)]
struct Velocity(f32, f32);

#[derive(Component, Clone, Copy)]
struct Health(u64);

#[derive(Component, Clone, Copy)]
struct Marker;

macro_rules! regroup_components {
    ($($component:ident),+ $(,)?) => {
        $(
            #[derive(Component, Clone, Copy)]
            struct $component;
        )+
    };
}

regroup_components!(G0, G1, G2, G3, G4, G5, G6, G7, G8, G9);

#[derive(Clone, Copy)]
struct Tracked(u64);

impl Component for Tracked {
    type Tracking = track::All;
}

fn empty_entities(count: usize) -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let entities = world.bulk_add_entity((0..count).map(|_| ())).collect();
    (world, entities)
}

fn positions(count: usize) -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let entities = world
        .bulk_add_entity((0..count).map(|i| (Position(i as f32, i as f32),)))
        .collect();
    (world, entities)
}

fn large_value(index: usize) -> Large {
    Large([index as u64; LARGE_WORDS])
}

fn large_components(count: usize) -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let entities = world
        .bulk_add_entity((0..count).map(|i| (large_value(i),)))
        .collect();
    (world, entities)
}

fn tracked_components(count: usize) -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let entities = world
        .bulk_add_entity((0..count).map(|i| (Tracked(i as u64),)))
        .collect();
    (world, entities)
}

fn three_components(count: usize) -> (World, Vec<EntityId>) {
    let mut world = World::new();
    let entities = world
        .bulk_add_entity((0..count).map(|i| {
            (
                Position(i as f32, i as f32),
                Velocity(1.0, -1.0),
                Health(i as u64),
            )
        }))
        .collect();
    (world, entities)
}

fn fragmented_empty_entities(count: usize) -> (World, Vec<EntityId>) {
    let (mut world, entities) = empty_entities(count * 2);

    for &entity in entities.iter().skip(1).step_by(2) {
        world.delete_entity(entity);
    }

    let live_entities = entities.into_iter().step_by(2).collect();
    (world, live_entities)
}

fn fragmented_positions(count: usize) -> (World, Vec<EntityId>) {
    let (mut world, entities) = positions(count * 2);

    for &entity in entities.iter().skip(1).step_by(2) {
        world.delete_entity(entity);
    }

    let live_entities = entities.into_iter().step_by(2).collect();
    (world, live_entities)
}

fn fragmented_large_components(count: usize) -> (World, Vec<EntityId>) {
    let (mut world, entities) = large_components(count * 2);

    for &entity in entities.iter().skip(1).step_by(2) {
        world.delete_entity(entity);
    }

    let live_entities = entities.into_iter().step_by(2).collect();
    (world, live_entities)
}

fn recycled_empty_world(count: usize) -> World {
    let (mut world, entities) = empty_entities(count);

    for entity in entities {
        world.delete_entity(entity);
    }

    world
}

fn partially_populated_world(count: usize) -> World {
    let (mut world, entities) = positions(count);

    for (i, entity) in entities.into_iter().enumerate().step_by(SPARSE_STEP) {
        world.add_component(entity, (Velocity(i as f32, -(i as f32)), Marker));
    }

    world
}

fn disjoint_world(count: usize) -> World {
    let mut world = World::new();
    let position_count = count - count / SPARSE_STEP;

    world
        .bulk_add_entity((0..position_count).map(|i| (Position(i as f32, i as f32),)))
        .count();
    world
        .bulk_add_entity((position_count..count).map(|i| (Velocity(i as f32, -(i as f32)),)))
        .count();

    world
}

fn regroup_world(entity_count: usize, overlapping_group_count: usize) -> World {
    let mut world = World::new();

    {
        let mut views = world
            .borrow::<(ViewMut<'_, Position>, ViewMut<'_, Velocity>)>()
            .unwrap();
        views.create_group();
    }

    if overlapping_group_count >= 2 {
        let mut views = world
            .borrow::<(ViewMut<'_, Velocity>, ViewMut<'_, Health>)>()
            .unwrap();
        views.create_group();
    }

    if overlapping_group_count >= 3 {
        let mut views = world
            .borrow::<(ViewMut<'_, Health>, ViewMut<'_, Marker>)>()
            .unwrap();
        views.create_group();
    }

    world
        .bulk_add_entity((0..entity_count).map(|i| {
            (
                Position(i as f32, i as f32),
                Velocity(1.0, -1.0),
                Health(i as u64),
                Marker,
            )
        }))
        .count();

    world
}

macro_rules! create_regroup_pair {
    ($world:expr, $left:ty, $right:ty) => {{
        let mut views = $world
            .borrow::<(ViewMut<'_, $left>, ViewMut<'_, $right>)>()
            .unwrap();
        views.create_group();
    }};
}

#[derive(Component, Clone, Copy)]
struct M0;

macro_rules! multiword_components {
    ($($component:ident),+ $(,)?) => {
        $(
            #[derive(Component, Clone, Copy)]
            struct $component;
        )+

        fn register_multiword_groups(world: &World) {
            $(
                create_regroup_pair!(world, M0, $component);
            )+
        }

        fn add_multiword_components(world: &mut World, entity: EntityId) {
            world.add_component(entity, (M0,));
            $(
                world.add_component(entity, ($component,));
            )+
        }
    };
}

multiword_components!(
    M1, M2, M3, M4, M5, M6, M7, M8, M9, M10, M11, M12, M13, M14, M15, M16, M17, M18, M19, M20, M21,
    M22, M23, M24, M25, M26, M27, M28, M29, M30, M31, M32, M33, M34, M35, M36, M37, M38, M39, M40,
    M41, M42, M43, M44, M45, M46, M47, M48, M49, M50, M51, M52, M53, M54, M55, M56, M57, M58, M59,
    M60, M61, M62, M63, M64,
);

fn register_overlap_groups(world: &World, group_count: usize) {
    if group_count >= 1 {
        create_regroup_pair!(world, G0, G1);
    }
    if group_count >= 2 {
        create_regroup_pair!(world, G1, G2);
    }
    if group_count >= 3 {
        create_regroup_pair!(world, G2, G3);
    }
    if group_count >= 4 {
        create_regroup_pair!(world, G3, G4);
    }
    if group_count >= 5 {
        create_regroup_pair!(world, G4, G5);
    }
    if group_count >= 6 {
        create_regroup_pair!(world, G5, G6);
    }
    if group_count >= 7 {
        create_regroup_pair!(world, G6, G7);
    }
    if group_count >= 8 {
        create_regroup_pair!(world, G7, G8);
    }
    if group_count >= 9 {
        create_regroup_pair!(world, G8, G9);
    }
    if group_count >= 10 {
        create_regroup_pair!(world, G9, G0);
    }
    if group_count >= 11 {
        create_regroup_pair!(world, G0, G2);
    }
    if group_count >= 12 {
        create_regroup_pair!(world, G1, G3);
    }
    if group_count >= 13 {
        create_regroup_pair!(world, G2, G4);
    }
    if group_count >= 14 {
        create_regroup_pair!(world, G3, G5);
    }
    if group_count >= 15 {
        create_regroup_pair!(world, G4, G6);
    }
    if group_count >= 16 {
        create_regroup_pair!(world, G5, G7);
    }
}

fn overlap_regroup_world(entity_count: usize, group_count: usize) -> World {
    let mut world = World::new();
    register_overlap_groups(&world, group_count);
    world
        .bulk_add_entity((0..entity_count).map(|_| (G0, G1, G2, G3, G4, G5, G6, G7, G8, G9)))
        .count();
    world
}

fn multiword_regroup_world(entity_count: usize) -> World {
    let mut world = World::new();
    register_multiword_groups(&world);

    for _ in 0..entity_count {
        let entity = world.add_entity(());
        add_multiword_components(&mut world, entity);
    }

    world
}

fn wide_regroup_world(entity_count: usize) -> World {
    let mut world = World::new();

    {
        let mut views = world
            .borrow::<(
                ViewMut<'_, G0>,
                ViewMut<'_, G1>,
                ViewMut<'_, G2>,
                ViewMut<'_, G3>,
                ViewMut<'_, G4>,
                ViewMut<'_, G5>,
                ViewMut<'_, G6>,
                ViewMut<'_, G7>,
                ViewMut<'_, G8>,
                ViewMut<'_, G9>,
            )>()
            .unwrap();
        views.create_group();
    }

    world
        .bulk_add_entity((0..entity_count).map(|_| (G0, G1, G2, G3, G4, G5, G6, G7, G8, G9)))
        .count();
    world
}

fn incomplete_regroup_world(entity_count: usize) -> World {
    let mut world = World::new();
    create_regroup_pair!(&world, G0, G1);
    world
        .bulk_add_entity((0..entity_count).map(|_| (G0,)))
        .count();
    world
}

fn incremental_regroup_world(entity_count: usize) -> World {
    let mut world = World::new();
    create_regroup_pair!(&world, G0, G1);
    create_regroup_pair!(&world, G0, G2);
    let entities: Vec<_> = world
        .bulk_add_entity((0..entity_count).map(|_| (G0, G2)))
        .collect();
    world.regroup();

    for entity in entities {
        world.add_component(entity, (G1,));
    }

    world
}

fn regroup_overlapping_groups(c: &mut Criterion) {
    let mut group = c.benchmark_group("regroup_overlapping_groups");

    for overlapping_group_count in 1..=3 {
        for entity_count in [100, 1_000, 10_000] {
            group.throughput(Throughput::Elements(entity_count as u64));
            group.bench_with_input(
                BenchmarkId::new(
                    format!("{overlapping_group_count}_overlapping_groups"),
                    entity_count,
                ),
                &(overlapping_group_count, entity_count),
                |b, &(overlapping_group_count, entity_count)| {
                    b.iter_batched(
                        || regroup_world(entity_count, overlapping_group_count),
                        |mut world| {
                            world.regroup();
                            black_box(&world);
                        },
                        BatchSize::LargeInput,
                    );
                },
            );
        }
    }

    let entity_count = MUTATION_COUNT;
    group.throughput(Throughput::Elements(entity_count as u64));

    group.bench_function("no_op_clean", |b| {
        let mut world = wide_regroup_world(entity_count);
        world.regroup();

        b.iter(|| {
            world.regroup();
            black_box(&world);
        });
    });

    group.bench_function("incomplete_group", |b| {
        b.iter_batched(
            || incomplete_regroup_world(entity_count),
            |mut world| {
                world.regroup();
                black_box(&world);
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("incremental_insertion", |b| {
        b.iter_batched(
            || incremental_regroup_world(entity_count),
            |mut world| {
                world.regroup();
                black_box(&world);
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("wide_10_storage_group", |b| {
        b.iter_batched(
            || wide_regroup_world(entity_count),
            |mut world| {
                world.regroup();
                black_box(&world);
            },
            BatchSize::LargeInput,
        );
    });

    for overlap_count in [8, 16] {
        group.bench_with_input(
            BenchmarkId::new("overlap_groups", overlap_count),
            &overlap_count,
            |b, &overlap_count| {
                b.iter_batched(
                    || overlap_regroup_world(entity_count, overlap_count),
                    |mut world| {
                        world.regroup();
                        black_box(&world);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    let multiword_entity_count = 100;
    group.throughput(Throughput::Elements(multiword_entity_count as u64));
    group.bench_function("multiword_65_storages", |b| {
        b.iter_batched(
            || multiword_regroup_world(multiword_entity_count),
            |mut world| {
                world.regroup();
                black_box(&world);
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn add_entities(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_entities");
    group.throughput(Throughput::Elements(MUTATION_COUNT as u64));

    // Add empty entities with fresh IDs.
    group.bench_function("empty_fresh_ids", |b| {
        b.iter_batched_ref(
            World::new,
            |world| {
                for _ in 0..MUTATION_COUNT {
                    black_box(world.add_entity(()));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Add entities with a small Position.
    group.bench_function("one_component_fresh_ids", |b| {
        b.iter_batched_ref(
            World::new,
            |world| {
                for i in 0..MUTATION_COUNT {
                    black_box(world.add_entity((Position(i as f32, i as f32),)));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Add entities with a 256-byte Large.
    group.bench_function("one_large_component_fresh_ids", |b| {
        b.iter_batched_ref(
            World::new,
            |world| {
                for i in 0..MUTATION_COUNT {
                    black_box(world.add_entity((large_value(i),)));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Add entities with three small components.
    group.bench_function("three_components_fresh_ids", |b| {
        b.iter_batched_ref(
            World::new,
            |world| {
                for i in 0..MUTATION_COUNT {
                    black_box(world.add_entity((
                        Position(i as f32, i as f32),
                        Velocity(1.0, -1.0),
                        Health(i as u64),
                    )));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Reuse IDs after entity deletion.
    group.bench_function("empty_recycled_ids", |b| {
        b.iter_batched_ref(
            || recycled_empty_world(MUTATION_COUNT),
            |world| {
                for _ in 0..MUTATION_COUNT {
                    black_box(world.add_entity(()));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Bulk-add entities with Position.
    group.bench_function("one_component_bulk", |b| {
        b.iter_batched_ref(
            World::new,
            |world| {
                let added = world
                    .bulk_add_entity((0..MUTATION_COUNT).map(|i| (Position(i as f32, i as f32),)))
                    .count();
                black_box(added);
            },
            BatchSize::LargeInput,
        );
    });

    // Bulk-add entities with Large.
    group.bench_function("one_large_component_bulk", |b| {
        b.iter_batched_ref(
            World::new,
            |world| {
                let added = world
                    .bulk_add_entity((0..MUTATION_COUNT).map(|i| (large_value(i),)))
                    .count();
                black_box(added);
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn delete_entities(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_entities");
    group.throughput(Throughput::Elements(MUTATION_COUNT as u64));

    // Delete empty entities.
    group.bench_function("empty", |b| {
        b.iter_batched_ref(
            || empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.delete_entity(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Delete entities carrying Position.
    group.bench_function("one_component", |b| {
        b.iter_batched_ref(
            || positions(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.delete_entity(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Delete entities carrying Large.
    group.bench_function("one_large_component", |b| {
        b.iter_batched_ref(
            || large_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.delete_entity(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Delete entities carrying three components.
    group.bench_function("three_components", |b| {
        b.iter_batched_ref(
            || three_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.delete_entity(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Delete Position entities with ID holes.
    group.bench_function("one_component_fragmented", |b| {
        b.iter_batched_ref(
            || fragmented_positions(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.delete_entity(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn add_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_components");
    group.throughput(Throughput::Elements(MUTATION_COUNT as u64));

    // Attach Position to empty entities.
    group.bench_function("one_missing", |b| {
        b.iter_batched_ref(
            || empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (Position(i as f32, i as f32),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Attach Large to empty entities.
    group.bench_function("one_large_missing", |b| {
        b.iter_batched_ref(
            || empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (large_value(i),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Attach three components to empty entities.
    group.bench_function("three_missing", |b| {
        b.iter_batched_ref(
            || empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(
                        entity,
                        (
                            Position(i as f32, i as f32),
                            Velocity(1.0, -1.0),
                            Health(i as u64),
                        ),
                    );
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Replace existing Position values.
    group.bench_function("one_replace_existing", |b| {
        b.iter_batched_ref(
            || positions(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (Position(-(i as f32), i as f32),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Replace existing Large values.
    group.bench_function("one_large_replace_existing", |b| {
        b.iter_batched_ref(
            || large_components(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (large_value(i + MUTATION_COUNT),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Attach Position across fragmented IDs.
    group.bench_function("one_missing_fragmented_entities", |b| {
        b.iter_batched_ref(
            || fragmented_empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (Position(i as f32, i as f32),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Attach Large across fragmented IDs.
    group.bench_function("one_large_missing_fragmented_entities", |b| {
        b.iter_batched_ref(
            || fragmented_empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (large_value(i),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Attach tracked components.
    group.bench_function("one_missing_tracked", |b| {
        b.iter_batched_ref(
            || empty_entities(MUTATION_COUNT),
            |(world, entities)| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.add_component(entity, (Tracked(i as u64),));
                }
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn remove_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_components");
    group.throughput(Throughput::Elements(MUTATION_COUNT as u64));

    // Remove Position and return values.
    group.bench_function("one_returned", |b| {
        b.iter_batched_ref(
            || positions(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.remove::<(Position,)>(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Remove Large and return values.
    group.bench_function("one_large_returned", |b| {
        b.iter_batched_ref(
            || large_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.remove::<(Large,)>(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Remove three values at once.
    group.bench_function("three_returned", |b| {
        b.iter_batched_ref(
            || three_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.remove::<(Position, Velocity, Health)>(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Delete Position without returning.
    group.bench_function("one_deleted", |b| {
        b.iter_batched_ref(
            || positions(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    world.delete_component::<(Position,)>(entity);
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Delete Large without returning.
    group.bench_function("one_large_deleted", |b| {
        b.iter_batched_ref(
            || large_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    world.delete_component::<(Large,)>(entity);
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Strip three small components.
    group.bench_function("all_stripped", |b| {
        b.iter_batched_ref(
            || three_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    world.strip(entity);
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Strip a large component.
    group.bench_function("large_stripped", |b| {
        b.iter_batched_ref(
            || large_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    world.strip(entity);
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Remove Position from fragmented IDs.
    group.bench_function("one_returned_fragmented_entities", |b| {
        b.iter_batched_ref(
            || fragmented_positions(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.remove::<(Position,)>(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Remove Large from fragmented IDs.
    group.bench_function("one_large_returned_fragmented_entities", |b| {
        b.iter_batched_ref(
            || fragmented_large_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.remove::<(Large,)>(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    // Remove tracked values.
    group.bench_function("one_returned_tracked", |b| {
        b.iter_batched_ref(
            || tracked_components(MUTATION_COUNT),
            |(world, entities)| {
                for &entity in entities.iter() {
                    black_box(world.remove::<(Tracked,)>(entity));
                }
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn iterate_entities(c: &mut Criterion) {
    let mut group = c.benchmark_group("iterate_entities");

    let (dense_world, _) = empty_entities(ITERATION_COUNT);
    let dense_entities = dense_world.borrow::<EntitiesView>().unwrap();
    group.throughput(Throughput::Elements(ITERATION_COUNT as u64));
    // Traverse a dense live entity storage.
    group.bench_function("all_alive", |b| {
        b.iter(|| {
            for entity in dense_entities.iter() {
                black_box(entity);
            }
        });
    });

    let (fragmented_world, _) = fragmented_empty_entities(ITERATION_COUNT / 2);
    let fragmented_entities = fragmented_world.borrow::<EntitiesView>().unwrap();
    group.throughput(Throughput::Elements((ITERATION_COUNT / 2) as u64));
    // Traverse entities with half the IDs deleted.
    group.bench_function("half_alive", |b| {
        b.iter(|| {
            for entity in fragmented_entities.iter() {
                black_box(entity);
            }
        });
    });

    group.finish();
}

fn iterate_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("iterate_components");
    group.throughput(Throughput::Elements(ITERATION_COUNT as u64));

    let (read_world, _) = positions(ITERATION_COUNT);
    let read_positions = read_world.borrow::<View<Position>>().unwrap();
    // Read a dense Position storage.
    group.bench_function("read_one_dense", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for position in (&read_positions).iter() {
                sum += position.0 + position.1;
            }
            black_box(sum);
        });
    });

    let (large_world, _) = large_components(ITERATION_COUNT);
    let large_values = large_world.borrow::<View<Large>>().unwrap();
    // Read each 256-byte Large payload.
    group.bench_function("read_one_large_dense", |b| {
        b.iter(|| {
            let mut sum = 0_u64;
            for component in (&large_values).iter() {
                for &word in component.0.iter() {
                    sum = sum.wrapping_add(word);
                }
            }
            black_box(sum);
        });
    });

    let (large_write_world, _) = large_components(ITERATION_COUNT);
    let mut large_write_values = large_write_world.borrow::<ViewMut<Large>>().unwrap();
    // Update each 256-byte Large payload.
    group.bench_function("write_one_large_dense", |b| {
        b.iter(|| {
            for component in (&mut large_write_values).iter() {
                for word in component.0.iter_mut() {
                    *word = word.wrapping_add(1);
                }
            }
            black_box(&large_write_values);
        });
    });

    let (write_world, _) = positions(ITERATION_COUNT);
    let mut write_positions = write_world.borrow::<ViewMut<Position>>().unwrap();
    // Mutate a dense Position storage.
    group.bench_function("write_one_dense", |b| {
        b.iter(|| {
            for position in (&mut write_positions).iter() {
                position.0 += 1.0;
                position.1 -= 1.0;
            }
            black_box(&write_positions);
        });
    });

    let (fragmented_world, _) = fragmented_positions(ITERATION_COUNT);
    let fragmented_positions = fragmented_world.borrow::<View<Position>>().unwrap();
    // Read Position values with sparse IDs.
    group.bench_function("read_one_fragmented_entity_ids", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for position in (&fragmented_positions).iter() {
                sum += position.0 + position.1;
            }
            black_box(sum);
        });
    });

    let (large_fragmented_world, _) = fragmented_large_components(ITERATION_COUNT);
    let large_fragmented_values = large_fragmented_world.borrow::<View<Large>>().unwrap();
    group.throughput(Throughput::Elements((ITERATION_COUNT / 2) as u64));
    // Read Large values with sparse IDs.
    group.bench_function("read_one_large_fragmented_entity_ids", |b| {
        b.iter(|| {
            let mut sum = 0_u64;
            for component in (&large_fragmented_values).iter() {
                sum = sum.wrapping_add(component.0[0]);
            }
            black_box(sum);
        });
    });
    group.throughput(Throughput::Elements(ITERATION_COUNT as u64));

    let (dense_world, _) = three_components(ITERATION_COUNT);
    let (dense_positions, dense_velocities, dense_health) = dense_world
        .borrow::<(View<Position>, View<Velocity>, View<Health>)>()
        .unwrap();
    // Join Position and Velocity densely.
    group.bench_function("join_two_dense", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for (position, velocity) in (&dense_positions, &dense_velocities).iter() {
                sum += position.0 + position.1 + velocity.0 + velocity.1;
            }
            black_box(sum);
        });
    });

    // Join three dense storages.
    group.bench_function("join_three_dense", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for (position, velocity, health) in
                (&dense_positions, &dense_velocities, &dense_health).iter()
            {
                sum += position.0 + position.1 + velocity.0 + velocity.1 + health.0 as f32;
            }
            black_box(sum);
        });
    });

    let partial_world = partially_populated_world(ITERATION_COUNT);
    let (partial_positions, partial_velocities, markers) = partial_world
        .borrow::<(View<Position>, View<Velocity>, View<Marker>)>()
        .unwrap();
    group.throughput(Throughput::Elements((ITERATION_COUNT / SPARSE_STEP) as u64));
    // Join storages with ten-percent overlap.
    group.bench_function("join_two_ten_percent_overlap", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for (position, velocity) in (&partial_positions, &partial_velocities).iter() {
                sum += position.0 + position.1 + velocity.0 + velocity.1;
            }
            black_box(sum);
        });
    });

    // Select the ten percent with Marker.
    group.bench_function("with_marker_ten_percent", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for (position, _) in (&partial_positions, &markers).iter() {
                sum += position.0 + position.1;
            }
            black_box(sum);
        });
    });

    group.throughput(Throughput::Elements(ITERATION_COUNT as u64));
    // Exclude the ten-percent Marker set.
    group.bench_function("without_marker_ninety_percent", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for (position, _) in (&partial_positions, !&markers).iter() {
                sum += position.0 + position.1;
            }
            black_box(sum);
        });
    });

    let disjoint_world = disjoint_world(ITERATION_COUNT);
    let (disjoint_positions, disjoint_velocities) = disjoint_world
        .borrow::<(View<Position>, View<Velocity>)>()
        .unwrap();
    group.throughput(Throughput::Elements((ITERATION_COUNT / SPARSE_STEP) as u64));
    // Join two disjoint component sets.
    group.bench_function("join_two_no_overlap", |b| {
        b.iter(|| {
            let mut count = 0;
            for components in (&disjoint_positions, &disjoint_velocities).iter() {
                black_box(components);
                count += 1;
            }
            black_box(count);
        });
    });

    group.throughput(Throughput::Elements(ITERATION_COUNT as u64));
    // Read Position values alongside IDs.
    group.bench_function("read_one_with_entity_id", |b| {
        b.iter(|| {
            let mut sum = 0_u64;
            for (entity, position) in (&read_positions).iter().with_id() {
                black_box(position);
                sum = sum.wrapping_add(entity.inner());
            }
            black_box(sum);
        });
    });

    let (tracked_world, _) = tracked_components(ITERATION_COUNT);
    let tracked = tracked_world.borrow::<View<Tracked, track::All>>().unwrap();
    // Read a tracked component storage.
    group.bench_function("read_one_tracked", |b| {
        b.iter(|| {
            let mut sum = 0_u64;
            for component in (&tracked).iter() {
                sum = sum.wrapping_add(component.0);
            }
            black_box(sum);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    add_entities,
    delete_entities,
    add_components,
    remove_components,
    iterate_entities,
    iterate_components,
    regroup_overlapping_groups,
);
criterion_main!(benches);
