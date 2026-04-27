use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use shipyard::{
    Component, EntitiesView, EntitiesViewMut, EntityId, IntoIter, Remove, View, ViewMut, World,
};
use std::hint::black_box;

const ENTITY_COUNT: usize = 1_000_000;
const PARTIAL_VELOCITY_COUNT: usize = 100_000;
const DEAD_COUNT: usize = 100_000;
const CHURN_COUNT: usize = 100_000;

#[derive(Component, Clone, Copy)]
struct Position(f32, f32);

#[derive(Component, Clone, Copy)]
struct Velocity(f32, f32);

#[derive(Component, Clone, Copy)]
struct Dead;

fn dense_hot_world() -> World {
    let world = World::new();

    world.run(
        |(mut entities, mut positions, mut velocities): (
            EntitiesViewMut,
            ViewMut<Position>,
            ViewMut<Velocity>,
        )| {
            for i in 0..ENTITY_COUNT {
                entities.add_entity(
                    (&mut positions, &mut velocities),
                    (Position(i as f32, i as f32), Velocity(1.0, -1.0)),
                );
            }
        },
    );

    world
}

fn partial_match_world() -> World {
    let world = World::new();
    let velocity_step = ENTITY_COUNT / PARTIAL_VELOCITY_COUNT;

    world.run(
        |(mut entities, mut positions, mut velocities): (
            EntitiesViewMut,
            ViewMut<Position>,
            ViewMut<Velocity>,
        )| {
            for i in 0..ENTITY_COUNT {
                let position = Position(i as f32, i as f32);

                if i % velocity_step == 0 {
                    entities.add_entity(
                        (&mut positions, &mut velocities),
                        (position, Velocity(1.0, -1.0)),
                    );
                } else {
                    entities.add_entity(&mut positions, position);
                }
            }
        },
    );

    world
}

fn filtered_hot_world() -> World {
    let world = World::new();
    let dead_step = ENTITY_COUNT / DEAD_COUNT;

    world.run(
        |(mut entities, mut positions, mut velocities, mut dead): (
            EntitiesViewMut,
            ViewMut<Position>,
            ViewMut<Velocity>,
            ViewMut<Dead>,
        )| {
            for i in 0..ENTITY_COUNT {
                let position = Position(i as f32, i as f32);
                let velocity = Velocity(1.0, -1.0);

                if i % dead_step == 0 {
                    entities.add_entity(
                        (&mut positions, &mut velocities, &mut dead),
                        (position, velocity, Dead),
                    );
                } else {
                    entities.add_entity((&mut positions, &mut velocities), (position, velocity));
                }
            }
        },
    );

    world
}

fn churn_world() -> (World, Vec<EntityId>) {
    let world = dense_world();

    let churn_entities = world
        .iter::<&Velocity>()
        .iter()
        .ids()
        .take(CHURN_COUNT)
        .collect::<Vec<_>>();

    (world, churn_entities)
}

fn hot_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_iteration");

    let dense_hot = dense_hot_world();
    group.bench_with_input(
        BenchmarkId::new("dense_join_pos_vel_yield", "1m/1m/1m"),
        &dense_hot,
        |b, world| {
            let (mut positions, velocities) = world
                .borrow::<(ViewMut<Position>, View<Velocity>)>()
                .unwrap();

            b.iter(|| {
                let mut sum = 0.0;

                for (position, velocity) in (&mut positions, &velocities).iter() {
                    position.0 += velocity.0;
                    position.1 += velocity.1;
                    sum += position.0 + position.1;
                }

                black_box(sum);
            });
        },
    );

    let partial_match = partial_match_world();
    group.bench_with_input(
        BenchmarkId::new("partial_join_pos_vel_yield", "1m/100k/100k"),
        &partial_match,
        |b, world| {
            let (mut positions, velocities) = world
                .borrow::<(ViewMut<Position>, View<Velocity>)>()
                .unwrap();

            b.iter(|| {
                let mut sum = 0.0;

                for (position, velocity) in (&mut positions, &velocities).iter() {
                    position.0 += velocity.0;
                    position.1 += velocity.1;
                    sum += position.0 + position.1;
                }

                black_box(sum);
            });
        },
    );

    let filtered_hot = filtered_hot_world();
    group.bench_with_input(
        BenchmarkId::new("not_filter_pos_vel_dead_yield", "1m/1m/100k/900k"),
        &filtered_hot,
        |b, world| {
            let (mut positions, velocities, dead) = world
                .borrow::<(ViewMut<Position>, View<Velocity>, View<Dead>)>()
                .unwrap();

            b.iter(|| {
                let mut sum = 0.0;

                for (position, velocity, _) in (&mut positions, &velocities, !&dead).iter() {
                    position.0 += velocity.0;
                    position.1 += velocity.1;
                    sum += position.0 + position.1;
                }

                black_box(sum);
            });
        },
    );

    group.finish();
}

fn churn_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("churn_cost");
    let (world, churn_entities) = churn_world();

    group.bench_with_input(
        BenchmarkId::new("remove_add_velocity", "100k"),
        &churn_entities,
        |b, churn_entities| {
            let (entities, mut velocities) =
                world.borrow::<(EntitiesView, ViewMut<Velocity>)>().unwrap();

            b.iter(|| {
                for &entity in churn_entities {
                    black_box(velocities.remove(entity));
                }

                for &entity in churn_entities {
                    entities.add_component(entity, &mut velocities, Velocity(1.0, -1.0));
                }
            });
        },
    );

    group.finish();
}

criterion_group!(benches, hot_iteration, churn_cost);
criterion_main!(benches);
