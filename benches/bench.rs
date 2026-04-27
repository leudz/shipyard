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

struct WorldBuilder {
    total_entities: usize,
    velocity_count: usize,
    dead_count: usize,
}

impl WorldBuilder {
    fn new() -> WorldBuilder {
        WorldBuilder {
            total_entities: 0,
            velocity_count: 0,
            dead_count: 0,
        }
    }

    fn total_entities(mut self, total_entities: usize) -> Self {
        self.total_entities = total_entities;

        self
    }

    fn velocity_count(mut self, velocity_count: usize) -> Self {
        self.velocity_count = velocity_count;

        self
    }

    fn dead_count(mut self, dead_count: usize) -> Self {
        self.dead_count = dead_count;

        self
    }

    fn build(self) -> World {
        let world = World::new();

        let velocity_step = if self.velocity_count == 0 {
            0
        } else {
            self.total_entities / self.velocity_count
        };
        let dead_step = if self.dead_count == 0 {
            0
        } else {
            self.total_entities / self.dead_count
        };

        world.run(
            |mut entities: EntitiesViewMut,
             mut positions: ViewMut<Position>,
             mut velocities: ViewMut<Velocity>,
             mut dead: ViewMut<Dead>| {
                for i in 0..self.total_entities {
                    let position = Position(i as f32, i as f32);
                    let velocity = Velocity(1.0, -1.0);

                    let eid = entities.add_entity(&mut positions, position);

                    if velocity_step != 0 && i % velocity_step == 0 {
                        entities.add_component(eid, &mut velocities, velocity);
                    }

                    if dead_step != 0 && i % dead_step == 0 {
                        entities.add_component(eid, &mut dead, Dead);
                    }
                }
            },
        );

        assert_eq!(
            world.borrow::<View<Position>>().unwrap().len(),
            self.total_entities
        );
        assert_eq!(
            world.borrow::<View<Velocity>>().unwrap().len(),
            self.velocity_count
        );
        assert_eq!(world.borrow::<View<Dead>>().unwrap().len(), self.dead_count);

        world
    }
}

/// `World` with entities that all have `Position` and `Velocity` components.
fn dense_world() -> World {
    WorldBuilder::new()
        .total_entities(ENTITY_COUNT)
        .velocity_count(ENTITY_COUNT)
        .build()
}

/// `World` with entities that all have a `Position` component but only some that have `Velocity`.
fn sparse_world() -> World {
    WorldBuilder::new()
        .total_entities(ENTITY_COUNT)
        .velocity_count(PARTIAL_VELOCITY_COUNT)
        .build()
}

/// `World` with entities that all have `Position` and `Velocity` components, some of which are `Dead`.
fn partially_dead_world() -> World {
    WorldBuilder::new()
        .total_entities(ENTITY_COUNT)
        .velocity_count(ENTITY_COUNT)
        .dead_count(DEAD_COUNT)
        .build()
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

fn iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("iteration");

    let dense_hot = dense_world();
    group.bench_with_input(
        BenchmarkId::new("dense_pos_vel_yield", "1m-1m-1m"),
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

    let sparse = sparse_world();
    group.bench_with_input(
        BenchmarkId::new("sparse_pos_vel_yield", "1m-100k-100k"),
        &sparse,
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

    let partially_dead = partially_dead_world();
    group.bench_with_input(
        BenchmarkId::new("not_filter_pos_vel_dead_yield", "1m-1m-100k-900k"),
        &partially_dead,
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

fn cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("cycle");
    let (world, churn_entities) = churn_world();

    // Repeatedly remove and add all `Velocity` components
    group.bench_with_input(
        BenchmarkId::new("add_remove_component", "100k"),
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

criterion_group!(benches, iteration, cycle);
criterion_main!(benches);
