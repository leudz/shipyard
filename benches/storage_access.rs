//! Storage access: the cost of borrowing a storage view from a prebuilt world.
//!
//! - `entities-view`: acquiring and dropping `World::borrow::<EntitiesView>()`. The world holds one entity
//!   owning `Value`, so it has a component storage next to the entities. Each iteration asks the view whether
//!   that entity is alive, a single lookup, so the timing is the borrow and its release, not entity work.
//!
//! The world is built and checked before timing.
//!
//! - `cargo bench --bench storage_access` runs the case.
//! - `cargo bench --bench storage_access -- --test` validates the case and runs its measurement once.

use criterion::{Criterion, criterion_group, criterion_main};
use shipyard::{Component, EntitiesView, EntitiesViewMut, World, track};
use std::hint::black_box;

/// 16-byte component of the probed entity.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Value([u32; 4]);

impl Component for Value {
    type Tracking = track::Untracked;
}

/// Borrowing `EntitiesView` from a world with one entity and one component storage.
///
/// Measured: `World::borrow::<EntitiesView>()`, one `is_alive` call and dropping the view.
fn entities_view(c: &mut Criterion) {
    let mut world = World::new();
    let probe = world.add_entity((Value([0; 4]),));

    // The view must see the entity, and dropping it must release the borrow.
    {
        let entities = world.borrow::<EntitiesView>().unwrap();
        assert!(entities.is_alive(probe));
    }
    assert!(world.borrow::<EntitiesViewMut>().is_ok());

    let mut group = c.benchmark_group("storage-access");
    group.bench_function("entities-view", |b| {
        b.iter(|| {
            let entities = black_box(&world).borrow::<EntitiesView>().unwrap();

            // Returning the answer and not the view drops the view inside the iteration.
            entities.is_alive(black_box(probe))
        });
    });
    group.finish();
}

criterion_group!(storage_access, entities_view);
criterion_main!(storage_access);
