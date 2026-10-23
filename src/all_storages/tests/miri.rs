//! Focused Miri scenarios for the storage registry in `AllStorages`.
//!
//! The registry is a hash map of boxed storages. Storages can be added through `&AllStorages`
//! while others are borrowed, so these tests keep borrows alive across insertions and check
//! every erased storage is reached with the right type and dropped exactly once.

use crate::all_storages::CustomStorageAccess;
#[cfg(all(feature = "std", feature = "thread_local"))]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::{Component, Unique};
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::error;
use crate::get::Get;
#[cfg(feature = "std")]
use crate::public_transport::ShipyardRwLock;
use crate::scheduler::Workload;
use crate::sparse_set::SparseSet;
use crate::storage::{SBoxBuilder, Storage, StorageId};
use crate::track;
use crate::tracking::TrackingTimestamp;
use crate::unique::UniqueStorage;
#[cfg(feature = "std")]
use crate::views::UniqueViewMut;
use crate::views::{EntitiesView, EntitiesViewMut, UniqueView, View};
use crate::world::World;
#[cfg(feature = "std")]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use alloc::format;
#[cfg(feature = "std")]
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::any::type_name;
#[cfg(feature = "std")]
use core::panic::AssertUnwindSafe;
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "std")]
use lock_api::RawRwLock as _;

/// Heap-owning value that counts how many times it's dropped, clones included.
///
/// Used as a custom storage, a unique and a component.
#[derive(Clone)]
struct Counted {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
}

impl Counted {
    fn new(drops: &Arc<AtomicUsize>) -> Counted {
        Counted {
            values: vec![1, 2],
            drops: drops.clone(),
        }
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

impl Storage for Counted {}

impl Unique for Counted {}

impl Component for Counted {
    type Tracking = track::Untracked;
}

struct Heap(Vec<u32>);

impl Component for Heap {
    type Tracking = track::Untracked;
}

/// Component aligned more strictly than any of its fields need.
#[repr(align(64))]
struct Aligned(u64);

impl Component for Aligned {
    type Tracking = track::Untracked;
}

struct Marker;

impl Component for Marker {
    type Tracking = track::Untracked;
}

fn drops(counter: &Arc<AtomicUsize>) -> usize {
    counter.load(Ordering::Relaxed)
}

fn counted(value: u32, drops: &Arc<AtomicUsize>) -> Counted {
    Counted {
        values: vec![value],
        drops: drops.clone(),
    }
}

/// Asserts that a custom storage borrowed through the shared registry API stays valid while
/// inserting more storages makes the registry reallocate. The registry only holds pointers to
/// boxed storages, so the borrowed one mustn't move. Every storage is dropped exactly once with
/// the world.
#[test]
fn registry_growth() {
    let drops = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();

    {
        let all_storages = world.all_storages().unwrap();
        let first = all_storages
            .custom_storage_or_insert_by_id(StorageId::Custom(0), || Counted::new(&drops))
            .unwrap();
        let capacity = all_storages
            .storages
            .read()
            .components_and_custom
            .capacity();

        for id in 1..=8 {
            let _ = all_storages
                .custom_storage_or_insert_by_id(StorageId::Custom(id), || Counted::new(&drops))
                .unwrap();
        }
        assert!(
            all_storages
                .storages
                .read()
                .components_and_custom
                .capacity()
                > capacity
        );

        assert_eq!(first.values, [1, 2]);
    }

    drop(world);
    assert_eq!(drops.load(Ordering::Relaxed), 9);
}

/// A unique type per `N`, so a world can hold many uniques.
struct Numbered<const N: usize>(Counted);

impl<const N: usize> Unique for Numbered<N> {}

/// Asserts that a unique borrowed through its view stays valid while adding more uniques makes
/// the unique map reallocate. Every unique is dropped exactly once with the world.
#[test]
fn unique_registry_growth() {
    let drops = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();

    {
        let all_storages = world.all_storages().unwrap();
        all_storages.add_unique(Numbered::<0>(Counted::new(&drops)));
        let first = all_storages
            .borrow::<UniqueView<'_, Numbered<0>>>()
            .unwrap();
        let capacity = all_storages.storages.read().uniques.capacity();

        all_storages.add_unique(Numbered::<1>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<2>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<3>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<4>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<5>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<6>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<7>(Counted::new(&drops)));
        all_storages.add_unique(Numbered::<8>(Counted::new(&drops)));
        assert!(all_storages.storages.read().uniques.capacity() > capacity);

        assert_eq!(first.0.values, [1, 2]);
    }

    drop(world);
    assert_eq!(drops.load(Ordering::Relaxed), 9);
}

/// Asserts typed and by-ID custom storage access: an exclusive typed borrow writes and keeps
/// every other borrow of the storage out, shared borrows by type and by ID coexist and read the
/// write once downcast to the right type, and an exclusive by-ID borrow writes through the
/// erased storage.
#[test]
fn custom_access() {
    let drops = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<Counted>();
    world.add_custom_storage(id, Counted::new(&drops)).unwrap();
    let all_storages = world.all_storages().unwrap();

    let mut typed = all_storages.custom_storage_mut::<Counted>().unwrap();
    typed.values.push(3);
    assert!(all_storages.custom_storage::<Counted>().is_err());
    assert!(all_storages.custom_storage_by_id(id).is_err());
    drop(typed);

    let erased = all_storages.custom_storage_by_id(id).unwrap();
    let typed = all_storages.custom_storage::<Counted>().unwrap();
    assert!(erased.any().downcast_ref::<Vec<u32>>().is_none());
    let counted = erased.any().downcast_ref::<Counted>().unwrap();
    assert_eq!(counted.values, [1, 2, 3]);
    assert_eq!(typed.values.len(), 3);
    assert!(all_storages.custom_storage_mut_by_id(id).is_err());
    drop((erased, typed));

    let mut erased = all_storages.custom_storage_mut_by_id(id).unwrap();
    let counted = erased.any_mut().downcast_mut::<Counted>().unwrap();
    counted.values.clear();
    drop(erased);

    let typed = all_storages.custom_storage::<Counted>().unwrap();
    assert!(typed.values.is_empty());
    drop(typed);

    drop(all_storages);
    drop(world);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Asserts that the guards returned by `iter_storages` keep each erased storage borrowed while
/// we read through them, and that `iter_storages_mut` gives write access that's visible once its
/// guards are dropped.
#[test]
fn iter_storages() {
    let drops = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world
        .add_custom_storage(StorageId::of::<Counted>(), Counted::new(&drops))
        .unwrap();
    let all_storages = world.all_storages().unwrap();

    let storages = all_storages.iter_storages();
    let counted = storages
        .iter()
        .find_map(|storage| storage.any().downcast_ref::<Counted>())
        .unwrap();
    assert!(all_storages.custom_storage_mut::<Counted>().is_err());
    assert_eq!(counted.values, [1, 2]);
    drop(storages);

    let mut storages = all_storages.iter_storages_mut();
    let counted = storages
        .iter_mut()
        .find_map(|storage| storage.any_mut().downcast_mut::<Counted>())
        .unwrap();
    assert!(all_storages.custom_storage::<Counted>().is_err());
    counted.values.push(3);
    drop(storages);

    let typed = all_storages.custom_storage::<Counted>().unwrap();
    assert_eq!(typed.values, [1, 2, 3]);
}

/// Asserts that `remove_unique` refuses to remove a unique that's borrowed, leaving it in place.
/// Otherwise it casts the erased box back to the concrete unique storage and hands the value
/// over: the world no longer drops it, we do, exactly once.
#[test]
fn remove_unique() {
    let drops = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique(Counted::new(&drops));

    let view = all_storages.borrow::<UniqueView<'_, Counted>>().unwrap();
    assert!(all_storages.remove_unique::<Counted>().is_err());
    assert_eq!(view.values, [1, 2]);
    drop(view);

    let unique = all_storages.remove_unique::<Counted>().unwrap();
    drop(all_storages);
    drop(world);
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    assert_eq!(unique.values, [1, 2]);

    drop(unique);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Asserts that replacing a unique drops the displaced value exactly once, right away, and keeps
/// the new value in the same storage until the world drops it.
#[test]
fn replace_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world.add_unique(counted(0, &counter));

    world.add_unique(counted(1, &counter));
    assert_eq!(drops(&counter), 1);
    assert_eq!(
        world.borrow::<UniqueView<'_, Counted>>().unwrap().values,
        [1]
    );

    drop(world);
    assert_eq!(drops(&counter), 2);
}

/// Calls `f` and returns the message it panicked with, if it did.
#[cfg(feature = "std")]
fn catch_panic(f: impl FnOnce()) -> Result<(), String> {
    std::panic::catch_unwind(AssertUnwindSafe(f))
        .map_err(|payload| *payload.downcast::<String>().unwrap())
}

/// Returns the message `add_unique` panics with when `Counted`'s unique storage is borrowed.
#[cfg(feature = "std")]
fn borrowed_panic() -> String {
    format!(
        "{:?}",
        error::GetStorage::StorageBorrow {
            name: Some(type_name::<UniqueStorage<Counted>>()),
            id: StorageId::of::<UniqueStorage<Counted>>(),
            borrow: error::Borrow::Unique,
        }
    )
}

/// Asserts that `add_unique` panics instead of replacing a unique while a `UniqueView` of it is
/// alive. The view keeps reading the original value and only the rejected value is dropped. Once
/// the view is gone a replacement goes through, so the registry lock wasn't poisoned and no
/// borrow was leaked.
#[cfg(feature = "std")]
#[test]
fn replace_shared_view() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique(counted(0, &counter));

    let view = all_storages.borrow::<UniqueView<'_, Counted>>().unwrap();
    let result = catch_panic(|| all_storages.add_unique(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    all_storages.add_unique(counted(2, &counter));
    assert_eq!(drops(&counter), 2);
    assert_eq!(
        all_storages
            .borrow::<UniqueView<'_, Counted>>()
            .unwrap()
            .values,
        [2]
    );

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique` panics instead of replacing a unique while a `UniqueViewMut` of it is
/// alive. The view can still write to the original value and only the rejected value is dropped.
/// Once the view is gone a replacement goes through, so the registry lock wasn't poisoned and no
/// borrow was leaked.
#[cfg(feature = "std")]
#[test]
fn replace_exclusive_view() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique(counted(0, &counter));

    let mut view = all_storages.borrow::<UniqueViewMut<'_, Counted>>().unwrap();
    let result = catch_panic(|| all_storages.add_unique(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    view.values.push(10);
    assert_eq!(view.values, [0, 10]);
    drop(view);

    all_storages.add_unique(counted(2, &counter));
    assert_eq!(drops(&counter), 2);
    assert_eq!(
        all_storages
            .borrow::<UniqueView<'_, Counted>>()
            .unwrap()
            .values,
        [2]
    );

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique` panics instead of replacing a unique while its storage is borrowed
/// directly with `custom_storage`. The guard keeps reading the original value and only the
/// rejected value is dropped. Once the guard is gone a replacement goes through.
#[cfg(feature = "std")]
#[test]
fn replace_shared_custom() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique(counted(0, &counter));

    let storage = all_storages
        .custom_storage::<UniqueStorage<Counted>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(storage.value.values, [0]);
    drop(storage);

    all_storages.add_unique(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique` panics instead of replacing a unique while its storage is borrowed
/// directly with `custom_storage_mut`. The guard can still write to the original value and only
/// the rejected value is dropped. Once the guard is gone a replacement goes through.
#[cfg(feature = "std")]
#[test]
fn replace_exclusive_custom() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique(counted(0, &counter));

    let mut storage = all_storages
        .custom_storage_mut::<UniqueStorage<Counted>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    storage.value.values.push(10);
    assert_eq!(storage.value.values, [0, 10]);
    drop(storage);

    all_storages.add_unique(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique` replaces a custom storage registered under the unique's storage id
/// that isn't a unique storage. The custom storage is dropped exactly once and a fresh unique
/// storage, flagged as inserted, takes its place.
#[test]
fn replace_foreign() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world
        .add_custom_storage(
            StorageId::of::<UniqueStorage<Counted>>(),
            counted(0, &counter),
        )
        .unwrap();

    world.add_unique(counted(1, &counter));
    assert_eq!(drops(&counter), 1);
    let view = world.borrow::<UniqueView<'_, Counted>>().unwrap();
    assert_eq!(view.values, [1]);
    assert!(view.is_inserted());
    drop(view);

    drop(world);
    assert_eq!(drops(&counter), 2);
}

/// Asserts that `add_unique` panics instead of replacing a custom storage registered under the
/// unique's storage id while it's borrowed. The guard keeps reading the custom storage and only
/// the rejected value is dropped. Once the guard is gone the custom storage is replaced.
#[cfg(feature = "std")]
#[test]
fn replace_borrowed_foreign() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    world.add_custom_storage(id, counted(0, &counter)).unwrap();
    let all_storages = world.all_storages().unwrap();

    let storage = all_storages.custom_storage_by_id(id).unwrap();
    let result = catch_panic(|| all_storages.add_unique(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    let foreign = storage.any().downcast_ref::<Counted>().unwrap();
    assert_eq!(foreign.values, [0]);
    drop(storage);

    all_storages.add_unique(counted(2, &counter));
    assert_eq!(drops(&counter), 2);
    assert_eq!(
        all_storages
            .borrow::<UniqueView<'_, Counted>>()
            .unwrap()
            .values,
        [2]
    );

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that the wrapper returned by `AllStorages::iter` owns its storage borrow: writes
/// through yielded references are valid while it's alive, the storage can't be borrowed
/// elsewhere until it's dropped, and the writes are visible afterwards.
#[test]
fn iter_borrows() {
    let mut world = World::new_for_test();
    let entity0 = world.add_entity(Heap(vec![0]));
    let entity1 = world.add_entity(Heap(vec![1]));
    let all_storages = world.all_storages().unwrap();

    let mut iter = all_storages.iter::<&mut Heap>();
    for heap in &mut iter {
        heap.0.push(10);
    }
    assert!(all_storages.borrow::<View<'_, Heap>>().is_err());
    drop(iter);

    let heaps = all_storages.borrow::<View<'_, Heap>>().unwrap();
    assert_eq!((&heaps).get(entity0).unwrap().0, [0, 10]);
    assert_eq!((&heaps).get(entity1).unwrap().0, [1, 10]);
}

/// Asserts that moving an entity to another world hands its component over: the source no
/// longer has it and still reaches the component swapped into its slot, the destination reads it,
/// and each component is dropped exactly once, by the world holding it.
#[test]
fn move_entity() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut source = World::new_for_test();
    let mut destination = World::new_for_test();
    let moved = source.add_entity(counted(0, &counter));
    let stays = source.add_entity(counted(1, &counter));

    source.move_entity(&mut destination, moved);

    assert!(source.get::<&Counted>(moved).is_err());
    assert_eq!(source.get::<&Counted>(stays).unwrap().values, [1]);
    assert_eq!(destination.get::<&Counted>(moved).unwrap().values, [0]);

    drop(source);
    assert_eq!(drops(&counter), 1);
    drop(destination);
    assert_eq!(drops(&counter), 2);
}

/// Asserts that cloning a registered storage into a new world, and a single entity into another
/// world while a view of the source storage is alive, builds independent copies: each original
/// and each copy is dropped exactly once, by its own world.
#[cfg(feature = "std")]
#[test]
fn clone() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::new_for_test();
    world.register_clone::<SparseSet<Counted>>();
    let entity = world.add_entity(counted(0, &counter));

    let copy = world.clone();
    assert_eq!(copy.get::<&Counted>(entity).unwrap().values, [0]);

    let mut other = World::new_for_test();
    let view = world.borrow::<View<'_, Counted>>().unwrap();
    world.clone_entity_to(&mut other, entity);
    assert_eq!(view.get(entity).unwrap().values, [0]);
    drop(view);
    assert_eq!(other.get::<&Counted>(entity).unwrap().values, [0]);

    drop(world);
    assert_eq!(drops(&counter), 1);
    drop(copy);
    assert_eq!(drops(&counter), 2);
    drop(other);
    assert_eq!(drops(&counter), 3);
}

/// Returns `true` if one of the ranges in `T`'s storage wraps around its physical end.
fn has_wrapped_range<T: Component + Send + Sync>(world: &World) -> bool {
    let view = world.borrow::<View<'_, T>>().unwrap();

    view.groups
        .ranges
        .iter()
        .any(|range| range.ring().wrap_offset() != 0)
}

/// Returns the values of `entity`'s `Counted` component.
fn values_of(world: &World, entity: EntityId) -> Vec<u32> {
    world.get::<&Counted>(entity).unwrap().values.clone()
}

/// Asserts that regrouping overlapping groups physically moves rows of heap-owning,
/// over-aligned and zero-sized components, including through a range that wraps around its
/// physical end. Every component stays reachable with its value, writes through a grouped
/// iteration land on the right components, and each component is dropped exactly once.
#[test]
fn regroup_moves() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::builder_for_test()
        .with_group::<(Counted, Aligned)>()
        .with_group::<(Counted, Marker)>()
        .build();

    // The (Counted, Marker) range is filled first. Appending to the (Counted, Aligned) range,
    // which comes before it in `Counted`, then rotates it.
    let cm0 = world.add_entity((counted(0, &counter), Marker));
    let cm1 = world.add_entity((counted(1, &counter), Marker));
    world.regroup();
    let ca2 = world.add_entity((counted(2, &counter), Aligned(2)));
    world.regroup();
    assert!(has_wrapped_range::<Counted>(&world));

    // An entity in both groups, then a deletion damaging the (Counted, Marker) range in both
    // members before the next regroup places everything again.
    let both3 = world.add_entity((counted(3, &counter), Aligned(3), Marker));
    world.regroup();
    world.delete_entity(cm0);
    world.regroup();
    assert_eq!(drops(&counter), 1);

    assert_eq!(values_of(&world, cm1), [1]);
    assert_eq!(values_of(&world, ca2), [2]);
    assert_eq!(values_of(&world, both3), [3]);
    assert_eq!(world.get::<&Aligned>(both3).unwrap().0, 3);

    for (counted, _) in &mut world.iter::<(&mut Counted, &Marker)>() {
        counted.values.push(10);
    }
    assert_eq!(values_of(&world, cm1), [1, 10]);
    assert_eq!(values_of(&world, ca2), [2]);
    assert_eq!(values_of(&world, both3), [3, 10]);

    drop(world);
    assert_eq!(drops(&counter), 4);
}

/// Returns the `(Counted, Aligned)` values of every entity owning both, sorted, iterating them
/// together.
#[cfg(feature = "std")]
fn grouped_values(world: &World) -> Vec<(u32, u64)> {
    use crate::iter::IntoIter;

    let (counted, aligned) = world
        .borrow::<(View<'_, Counted>, View<'_, Aligned>)>()
        .unwrap();
    let mut values = Vec::new();
    (&counted, &aligned)
        .iter()
        .for_each(|(counted, aligned)| values.push((counted.values[0], aligned.0)));
    values.sort_unstable();

    values
}

/// Asserts that replacing a grouped member with a clone keeps the destination's groups and
/// union working: a grouped iteration right after the replacement, and after the next regroup,
/// reaches every copied component. Each original and each copy is dropped exactly once.
#[cfg(feature = "std")]
#[test]
fn regroup_clone() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut destination = World::builder_for_test()
        .with_group::<(Counted, Aligned)>()
        .with_group::<(Counted, Marker)>()
        .build();
    let mut source = World::builder_for_test().build();
    source.register_clone::<SparseSet<Counted>>();

    // The destination's entities own all three components, so the regroup attaches the union.
    // The source's entities get the same ids.
    for value in 0..3 {
        destination.add_entity((counted(value, &counter), Aligned(value.into()), Marker));
        source.add_entity(counted(10 + value, &counter));
    }
    destination.regroup();

    source
        .all_storages()
        .unwrap()
        .clone_storages_to(&mut destination.all_storages_mut().unwrap());
    assert_eq!(drops(&counter), 3);
    assert_eq!(grouped_values(&destination), [(10, 0), (11, 1), (12, 2)]);

    destination.regroup();
    assert_eq!(grouped_values(&destination), [(10, 0), (11, 1), (12, 2)]);

    drop(destination);
    assert_eq!(drops(&counter), 6);
    drop(source);
    assert_eq!(drops(&counter), 9);
}

/// Asserts that regrouping one connected set through borrow guards moves heap-owning,
/// over-aligned and zero-sized components and attaches their union, while a storage outside the
/// set stays borrowed exclusively and is written through. A held borrow of a member then fails
/// the next regroup before anything moves, and the guards it took are released.
/// Each component is dropped exactly once.
#[test]
fn regroup_guarded() {
    use crate::views::ViewMut;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::builder_for_test()
        .with_group::<(Counted, Aligned)>()
        .with_group::<(Counted, Marker)>()
        .build();
    let both = world.add_entity((counted(0, &counter), Aligned(0), Marker));
    let ca = world.add_entity((counted(1, &counter), Aligned(1)));
    let cm = world.add_entity((counted(2, &counter), Marker));
    let heap = world.add_entity(Heap(vec![7]));
    let counted_id = StorageId::of::<SparseSet<Counted>>();
    let set = world
        .all_storages()
        .unwrap()
        .grouping
        .set_of(counted_id)
        .unwrap()
        .clone();

    let mut held = world.borrow::<ViewMut<'_, Heap>>().unwrap();
    world
        .all_storages()
        .unwrap()
        .regroup_connected_set(&set)
        .unwrap();
    (&mut held).get(heap).unwrap().0.push(8);
    drop(held);

    let counted_view = world.borrow::<View<'_, Counted>>().unwrap();
    // The two declared groups and the union of `both`.
    assert_eq!(counted_view.groups.ranges.len(), 3);
    assert_eq!(counted_view.groups.unevaluated_start, counted_view.len());
    drop(counted_view);
    assert_eq!(values_of(&world, both), [0]);
    assert_eq!(values_of(&world, ca), [1]);
    assert_eq!(values_of(&world, cm), [2]);
    assert_eq!(world.get::<&Heap>(heap).unwrap().0, [7, 8]);

    let late = world.add_entity((counted(3, &counter), Marker));
    let held = world.borrow::<View<'_, Marker>>().unwrap();
    assert!(
        world
            .all_storages()
            .unwrap()
            .regroup_connected_set(&set)
            .is_err()
    );
    drop(held);
    // `Counted` and `Aligned` were borrowed before `Marker` failed, they're free again.
    drop(
        world
            .borrow::<(ViewMut<'_, Counted>, ViewMut<'_, Aligned>)>()
            .unwrap(),
    );
    world
        .all_storages()
        .unwrap()
        .regroup_connected_set(&set)
        .unwrap();
    assert_eq!(values_of(&world, late), [3]);
    assert_eq!(drops(&counter), 0);

    drop(world);
    assert_eq!(drops(&counter), 4);
}

/// Heap-owning component that counts how many times it's dropped, tracking everything.
struct TrackedCounted {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
}

impl Drop for TrackedCounted {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

impl Component for TrackedCounted {
    type Tracking = track::All;
}

fn tracked_counted(value: u32, drops: &Arc<AtomicUsize>) -> TrackedCounted {
    TrackedCounted {
        values: vec![value],
        drops: drops.clone(),
    }
}

/// Asserts that regroup moves the timestamps of tracked, heap-owning components with them.
/// After a modification, damage in both members, a deletion and a new pair, the grouped,
/// inserted and modified queries and every value are the same once regrouped. A mutable traversal
/// then reaches every pair once, and each component is dropped exactly once.
#[test]
fn regroup_tracked() {
    use crate::iter::IntoIter;
    use crate::views::ViewMut;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::builder_for_test()
        .with_group::<(TrackedCounted, Aligned)>()
        .build();
    let e: Vec<EntityId> = (0..8)
        .map(|i| world.add_entity((tracked_counted(i, &counter), Aligned(u64::from(i)))))
        .collect();
    world.regroup();
    world.run(|tracked: ViewMut<'_, TrackedCounted, track::All>| {
        tracked.clear_all_inserted_and_modified();
    });

    world.run(|mut tracked: ViewMut<'_, TrackedCounted, track::All>| {
        (&mut tracked).get(e[5]).unwrap().values.push(50);
    });
    world.remove::<(Aligned,)>(e[1]);
    let (component,) = world.remove::<(TrackedCounted,)>(e[6]);
    world.add_component(e[6], component.unwrap());
    world.delete_entity(e[3]);
    let new = world.add_entity((tracked_counted(8, &counter), Aligned(8)));
    // Deletion tracking keeps e3's component until the world drops it.
    assert_eq!(drops(&counter), 0);

    let results = |world: &World| {
        let (tracked, aligned) = world
            .borrow::<(View<'_, TrackedCounted, track::All>, View<'_, Aligned>)>()
            .unwrap();
        let mut grouped: Vec<(EntityId, Vec<u32>, u64)> = (&tracked, &aligned)
            .iter()
            .with_id()
            .map(|(entity, (tracked, aligned))| (entity, tracked.values.clone(), aligned.0))
            .collect();
        grouped.sort_unstable();
        let mut inserted: Vec<EntityId> = (tracked.inserted(), &aligned)
            .iter()
            .with_id()
            .map(|(entity, _)| entity)
            .collect();
        inserted.sort_unstable();
        let mut modified: Vec<EntityId> = (tracked.modified(), &aligned)
            .iter()
            .with_id()
            .map(|(entity, _)| entity)
            .collect();
        modified.sort_unstable();

        (grouped, inserted, modified)
    };
    let before = results(&world);
    let mut expected_inserted = vec![e[6], new];
    expected_inserted.sort_unstable();
    assert_eq!(before.1, expected_inserted);
    assert_eq!(before.2, [e[5]]);

    world.regroup();

    assert_eq!(results(&world), before);
    for (mut tracked, _) in &mut world.iter::<(&mut TrackedCounted, &Aligned)>() {
        tracked.values.push(10);
    }
    let (tracked, aligned) = world
        .borrow::<(View<'_, TrackedCounted, track::All>, View<'_, Aligned>)>()
        .unwrap();
    for (entity, values, _) in &before.0 {
        let mut expected = values.clone();
        expected.push(10);
        assert_eq!(tracked.get(*entity).unwrap().values, expected);
    }
    assert_eq!(tracked.get(e[1]).unwrap().values, [1]);
    assert!(aligned.get(e[1]).is_err());
    drop((tracked, aligned));

    drop(world);
    assert_eq!(drops(&counter), 9);
}

/// Asserts that a component left by a deleted entity, at the index a new entity reuses,
/// isn't grouped with the new entity's component and keeps its value.
/// Each component is dropped exactly once.
#[test]
fn regroup_generation_reuse() {
    use crate::AddComponent;
    use crate::iter::IntoIter;
    use crate::views::ViewMut;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::builder_for_test()
        .with_group::<(Counted, Aligned)>()
        .build();
    let old = world.add_entity((counted(0, &counter), Aligned(0)));
    let other = world.add_entity((counted(1, &counter), Aligned(1)));
    world.regroup();
    world.delete_entity(old);
    let new = world.add_entity((counted(2, &counter),));
    assert_eq!(new.index(), old.index());
    // The deleted entity's id gets an `Aligned` again, it stays a component of a dead entity.
    world
        .borrow::<ViewMut<'_, Aligned>>()
        .unwrap()
        .add_component_unchecked(old, Aligned(9));

    world.regroup();

    let (counted_view, aligned) = world
        .borrow::<(View<'_, Counted>, View<'_, Aligned>)>()
        .unwrap();
    let grouped: Vec<EntityId> = (&counted_view, &aligned)
        .iter()
        .with_id()
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(grouped, [other]);
    assert_eq!(aligned.get(old).unwrap().0, 9);
    assert_eq!(counted_view.get(new).unwrap().values, [2]);
    drop((counted_view, aligned));

    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that a regroup after scattered re-additions, removals and a deletion in both members
/// of a group of heap-owning components moves every row through its payload cycles without
/// losing, duplicating or dropping one: values stay with their entities, each dropped once.
#[test]
fn regroup_churn() {
    use crate::iter::IntoIter;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::builder_for_test()
        .with_group::<(Counted, Aligned)>()
        .build();
    let e: Vec<EntityId> = (0..24)
        .map(|i| world.add_entity((counted(i, &counter), Aligned(u64::from(i)))))
        .collect();
    world.regroup();

    for i in (1..24).step_by(4) {
        let (component,) = world.remove::<(Counted,)>(e[i]);
        world.add_component(e[i], component.unwrap());
    }
    for i in (2..24).step_by(5) {
        world.remove::<(Aligned,)>(e[i]);
    }
    world.delete_entity(e[9]);
    assert_eq!(drops(&counter), 1);

    world.regroup();

    let (counted_view, aligned) = world
        .borrow::<(View<'_, Counted>, View<'_, Aligned>)>()
        .unwrap();
    let mut grouped: Vec<(EntityId, Vec<u32>, u64)> = (&counted_view, &aligned)
        .iter()
        .with_id()
        .map(|(entity, (counted, aligned))| (entity, counted.values.clone(), aligned.0))
        .collect();
    grouped.sort_unstable();
    let mut expected: Vec<(EntityId, Vec<u32>, u64)> = (0..24u32)
        .filter(|&i| i != 9 && i % 5 != 2)
        .map(|i| (e[i as usize], vec![i], u64::from(i)))
        .collect();
    expected.sort_unstable();
    assert_eq!(grouped, expected);
    for i in (2..24).step_by(5) {
        assert_eq!(counted_view.get(e[i]).unwrap().values, [i as u32]);
    }
    drop((counted_view, aligned));

    drop(world);
    assert_eq!(drops(&counter), 24);
}

/// Defines a test grouping `Aligned` with heap-owning components behind `$wrapper`.
/// After a deletion, a re-addition and a new pair, regroup keeps every value with its entity,
/// and each component is dropped exactly once.
#[cfg(all(feature = "std", feature = "thread_local"))]
macro_rules! regroup_wrapped {
    ($name: ident, $wrapper: ident) => {
        #[test]
        fn $name() {
            use crate::AddComponent;
            use crate::Remove;
            use crate::borrow::$wrapper;
            use crate::iter::IntoIter;
            use crate::views::ViewMut;

            let counter = Arc::new(AtomicUsize::new(0));
            let mut world = World::builder_for_test()
                .with_group::<(Aligned, $wrapper<Counted>)>()
                .build();
            let add = |world: &mut World, i: u32| {
                let entity = world.add_entity((Aligned(u64::from(i)),));
                world
                    .borrow::<$wrapper<ViewMut<'_, Counted>>>()
                    .unwrap()
                    .add_component_unchecked(entity, counted(i, &counter));
                entity
            };
            let e: Vec<EntityId> = (0..6).map(|i| add(&mut world, i)).collect();
            world.regroup();

            world.delete_entity(e[1]);
            {
                let mut wrapped = world.borrow::<$wrapper<ViewMut<'_, Counted>>>().unwrap();
                // From the middle of the range, its last component takes the slot.
                let component = wrapped.remove(e[2]).unwrap();
                wrapped.add_component_unchecked(e[2], component);
            }
            let new = add(&mut world, 6);

            world.regroup();

            let (aligned, wrapped) = world
                .borrow::<(View<'_, Aligned>, $wrapper<View<'_, Counted>>)>()
                .unwrap();
            let mut grouped: Vec<(EntityId, Vec<u32>, u64)> = (&aligned, &*wrapped)
                .iter()
                .with_id()
                .map(|(entity, (aligned, counted))| (entity, counted.values.clone(), aligned.0))
                .collect();
            grouped.sort_unstable();
            let mut expected: Vec<(EntityId, Vec<u32>, u64)> = [0, 2, 3, 4, 5]
                .into_iter()
                .map(|i: u32| (e[i as usize], vec![i], u64::from(i)))
                .chain([(new, vec![6], 6)])
                .collect();
            expected.sort_unstable();
            assert_eq!(grouped, expected);
            drop((aligned, wrapped));

            assert_eq!(drops(&counter), 1);
            drop(world);
            assert_eq!(drops(&counter), 7);
        }
    };
}

#[cfg(all(feature = "std", feature = "thread_local"))]
regroup_wrapped!(regroup_non_send, NonSend);
#[cfg(all(feature = "std", feature = "thread_local"))]
regroup_wrapped!(regroup_non_sync, NonSync);
#[cfg(all(feature = "std", feature = "thread_local"))]
regroup_wrapped!(regroup_non_send_sync, NonSendSync);

/// Heap-owning value that is neither `Send` nor `Sync` and counts how many times it's dropped.
///
/// Used as a custom storage and as a unique.
#[cfg(all(feature = "std", feature = "thread_local"))]
struct Local {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
    _non_send_sync: core::marker::PhantomData<alloc::rc::Rc<()>>,
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Local {
    fn new(drops: &Arc<AtomicUsize>) -> Local {
        Local {
            values: vec![1, 2],
            drops: drops.clone(),
            _non_send_sync: core::marker::PhantomData,
        }
    }
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Drop for Local {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Storage for Local {}

#[cfg(all(feature = "std", feature = "thread_local"))]
impl Unique for Local {}

/// Asserts that a `!Send + !Sync` unique and custom storage are only reachable from the world's
/// thread: another thread is rejected with `WrongThread`, removing the unique included, without
/// changing anything. The world's thread then writes both and removes the unique, which casts
/// its erased box back to the unique storage, and each value is dropped exactly once.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn thread_local_storages() {
    use crate::UniqueViewMut;
    use crate::borrow::NonSendSync;
    use crate::error;

    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world.add_unique_non_send_sync(Local::new(&counter));
    let _ = world
        .all_storages()
        .unwrap()
        .custom_storage_or_insert_non_send_sync(|| Local::new(&counter))
        .unwrap();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let all_storages = world.all_storages().unwrap();

            let custom = all_storages.custom_storage::<Local>();
            assert!(matches!(
                custom,
                Err(error::GetStorage::StorageBorrow {
                    borrow: error::Borrow::WrongThread,
                    ..
                })
            ));
            assert!(
                all_storages
                    .borrow::<NonSendSync<UniqueViewMut<'_, Local>>>()
                    .is_err()
            );
            assert!(matches!(
                all_storages.remove_unique::<Local>(),
                Err(error::UniqueRemove::StorageBorrow((
                    _,
                    error::Borrow::WrongThread
                )))
            ));
        });
    });
    assert_eq!(drops(&counter), 0);

    {
        let all_storages = world.all_storages().unwrap();
        all_storages
            .custom_storage_mut::<Local>()
            .unwrap()
            .values
            .push(3);
        let mut unique = all_storages
            .borrow::<NonSendSync<UniqueViewMut<'_, Local>>>()
            .unwrap();
        unique.values.push(4);
    }

    let unique = world.remove_unique::<Local>().unwrap();
    assert_eq!(unique.values, [1, 2, 4]);
    assert_eq!(
        world
            .all_storages()
            .unwrap()
            .custom_storage::<Local>()
            .unwrap()
            .values,
        [1, 2, 3]
    );

    drop(world);
    assert_eq!(drops(&counter), 1);
    drop(unique);
    assert_eq!(drops(&counter), 2);
}

/// Unique that is `Send` but not `Sync`.
#[cfg(feature = "thread_local")]
struct SendOnly(Counted, core::marker::PhantomData<core::cell::Cell<()>>);

#[cfg(feature = "thread_local")]
impl Unique for SendOnly {}

/// Asserts that `remove_unique` takes uniques added with `add_unique_non_send` and
/// `add_unique_non_sync` out as the wrapped types they were stored as: each value is handed back
/// intact and dropped exactly once, by us.
#[cfg(feature = "thread_local")]
#[test]
fn remove_wrapped_uniques() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world.add_unique_non_send(counted(1, &counter));
    world.add_unique_non_sync(SendOnly(counted(2, &counter), core::marker::PhantomData));

    let non_send = world.remove_unique::<Counted>().unwrap();
    let non_sync = world.remove_unique::<SendOnly>().unwrap();
    drop(world);
    assert_eq!(drops(&counter), 0);
    assert_eq!(non_send.values, [1]);
    assert_eq!(non_sync.0.values, [2]);

    drop((non_send, non_sync));
    assert_eq!(drops(&counter), 2);
}

#[cfg(all(feature = "std", feature = "thread_local"))]
fn local(value: u32, drops: &Arc<AtomicUsize>) -> Local {
    Local {
        values: vec![value],
        drops: drops.clone(),
        _non_send_sync: core::marker::PhantomData,
    }
}

#[cfg(feature = "thread_local")]
fn send_only(value: u32, drops: &Arc<AtomicUsize>) -> SendOnly {
    SendOnly(counted(value, drops), core::marker::PhantomData)
}

/// Asserts that replacing uniques added with each thread-local method drops each displaced value
/// exactly once, right away, and keeps the new values in their wrapped storages until the world
/// drops them.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_wrapped_uniques() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world.add_unique_non_send(counted(0, &counter));
    world.add_unique_non_sync(send_only(0, &counter));
    world.add_unique_non_send_sync(local(0, &counter));

    world.add_unique_non_send(counted(1, &counter));
    world.add_unique_non_sync(send_only(1, &counter));
    world.add_unique_non_send_sync(local(1, &counter));
    assert_eq!(drops(&counter), 3);

    let non_send = world.borrow::<NonSend<UniqueView<'_, Counted>>>().unwrap();
    let non_sync = world.borrow::<NonSync<UniqueView<'_, SendOnly>>>().unwrap();
    let non_send_sync = world
        .borrow::<NonSendSync<UniqueView<'_, Local>>>()
        .unwrap();
    assert_eq!(non_send.values, [1]);
    assert_eq!((**non_sync).0.values, [1]);
    assert_eq!(non_send_sync.values, [1]);
    drop((non_send, non_sync, non_send_sync));

    drop(world);
    assert_eq!(drops(&counter), 6);
}

/// Asserts that `add_unique_non_send` panics instead of replacing a unique while a shared view of
/// it is alive. The view keeps reading the original value, the tracking is left untouched and
/// only the rejected value is dropped. Once the view is gone a replacement goes through.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_non_send_shared() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique_non_send(counted(0, &counter));

    let view = all_storages
        .borrow::<NonSend<UniqueView<'_, Counted>>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique_non_send(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    let view = all_storages
        .borrow::<NonSend<UniqueView<'_, Counted>>>()
        .unwrap();
    assert!(view.is_inserted());
    assert!(!view.is_modified());
    drop(view);

    all_storages.add_unique_non_send(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique_non_send` panics instead of replacing a unique while an exclusive view
/// of it is alive. The view keeps reading the original value, the tracking is left untouched and
/// only the rejected value is dropped. Once the view is gone a replacement goes through.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_non_send_exclusive() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique_non_send(counted(0, &counter));

    let view = all_storages
        .borrow::<NonSend<UniqueViewMut<'_, Counted>>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique_non_send(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    let view = all_storages
        .borrow::<NonSend<UniqueView<'_, Counted>>>()
        .unwrap();
    assert!(view.is_inserted());
    assert!(!view.is_modified());
    drop(view);

    all_storages.add_unique_non_send(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique_non_sync` panics instead of replacing a unique while a shared view of
/// it is alive. The view keeps reading the original value, the tracking is left untouched and
/// only the rejected value is dropped. Once the view is gone a replacement goes through.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_non_sync_shared() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique_non_sync(counted(0, &counter));

    let view = all_storages
        .borrow::<NonSync<UniqueView<'_, Counted>>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique_non_sync(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    let view = all_storages
        .borrow::<NonSync<UniqueView<'_, Counted>>>()
        .unwrap();
    assert!(view.is_inserted());
    assert!(!view.is_modified());
    drop(view);

    all_storages.add_unique_non_sync(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique_non_sync` panics instead of replacing a unique while an exclusive view
/// of it is alive. The view keeps reading the original value, the tracking is left untouched and
/// only the rejected value is dropped. Once the view is gone a replacement goes through.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_non_sync_exclusive() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique_non_sync(counted(0, &counter));

    let view = all_storages
        .borrow::<NonSync<UniqueViewMut<'_, Counted>>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique_non_sync(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    let view = all_storages
        .borrow::<NonSync<UniqueView<'_, Counted>>>()
        .unwrap();
    assert!(view.is_inserted());
    assert!(!view.is_modified());
    drop(view);

    all_storages.add_unique_non_sync(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique_non_send_sync` panics instead of replacing a unique while a shared
/// view of it is alive. The view keeps reading the original value, the tracking is left untouched
/// and only the rejected value is dropped. Once the view is gone a replacement goes through.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_non_send_sync_shared() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique_non_send_sync(counted(0, &counter));

    let view = all_storages
        .borrow::<NonSendSync<UniqueView<'_, Counted>>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique_non_send_sync(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    let view = all_storages
        .borrow::<NonSendSync<UniqueView<'_, Counted>>>()
        .unwrap();
    assert!(view.is_inserted());
    assert!(!view.is_modified());
    drop(view);

    all_storages.add_unique_non_send_sync(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `add_unique_non_send_sync` panics instead of replacing a unique while an exclusive
/// view of it is alive. The view keeps reading the original value, the tracking is left untouched
/// and only the rejected value is dropped. Once the view is gone a replacement goes through.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_non_send_sync_exclusive() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    all_storages.add_unique_non_send_sync(counted(0, &counter));

    let view = all_storages
        .borrow::<NonSendSync<UniqueViewMut<'_, Counted>>>()
        .unwrap();
    let result = catch_panic(|| all_storages.add_unique_non_send_sync(counted(1, &counter)));
    assert_eq!(result, Err(borrowed_panic()));
    assert_eq!(drops(&counter), 1);
    assert_eq!(view.values, [0]);
    drop(view);

    let view = all_storages
        .borrow::<NonSendSync<UniqueView<'_, Counted>>>()
        .unwrap();
    assert!(view.is_inserted());
    assert!(!view.is_modified());
    drop(view);

    all_storages.add_unique_non_send_sync(counted(2, &counter));
    assert_eq!(drops(&counter), 2);

    drop(all_storages);
    drop(world);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that each thread-local method replaces a custom storage registered under the unique's
/// storage id that isn't a unique storage with a fresh storage using that method's wrapper. Each
/// custom storage is dropped exactly once.
#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn replace_foreign_wrapped() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let ids = [
        StorageId::of::<UniqueStorage<Counted>>(),
        StorageId::of::<UniqueStorage<SendOnly>>(),
        StorageId::of::<UniqueStorage<Local>>(),
    ];
    for id in ids {
        world.add_custom_storage(id, counted(0, &counter)).unwrap();
    }

    world.add_unique_non_send(counted(1, &counter));
    world.add_unique_non_sync(send_only(1, &counter));
    world.add_unique_non_send_sync(local(1, &counter));
    assert_eq!(drops(&counter), 3);

    let non_send = world.borrow::<NonSend<UniqueView<'_, Counted>>>().unwrap();
    let non_sync = world.borrow::<NonSync<UniqueView<'_, SendOnly>>>().unwrap();
    let non_send_sync = world
        .borrow::<NonSendSync<UniqueView<'_, Local>>>()
        .unwrap();
    assert_eq!(non_send.values, [1]);
    assert_eq!((**non_sync).0.values, [1]);
    assert_eq!(non_send_sync.values, [1]);
    drop((non_send, non_sync, non_send_sync));

    drop(world);
    assert_eq!(drops(&counter), 6);
}

#[cfg(feature = "std")]
std::thread_local! {
    /// How many times this thread acquired a `CountingLock`, in either mode.
    static ACQUISITIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// A parking_lot lock that counts how many times each thread acquires it.
///
/// Every test runs on its own thread, so a test only sees the acquisitions of its own world.
#[cfg(feature = "std")]
struct CountingLock(parking_lot::RawRwLock);

#[cfg(feature = "std")]
impl ShipyardRwLock for CountingLock {
    fn new() -> Box<dyn ShipyardRwLock + Send + Sync> {
        Box::new(CountingLock(parking_lot::RawRwLock::INIT))
    }

    fn lock_shared(&self) {
        ACQUISITIONS.with(|acquisitions| acquisitions.set(acquisitions.get() + 1));
        ShipyardRwLock::lock_shared(&self.0);
    }

    unsafe fn unlock_shared(&self) {
        // SAFE: we forward to the inner lock our caller locked through `lock_shared`, so the
        // shared lock it holds on us is a shared lock on the inner lock.
        unsafe { ShipyardRwLock::unlock_shared(&self.0) }
    }

    fn lock_exclusive(&self) {
        ACQUISITIONS.with(|acquisitions| acquisitions.set(acquisitions.get() + 1));
        ShipyardRwLock::lock_exclusive(&self.0);
    }

    unsafe fn unlock_exclusive(&self) {
        // SAFE: we forward to the inner lock our caller locked through `lock_exclusive`, so the
        // exclusive lock it holds on us is an exclusive lock on the inner lock.
        unsafe { ShipyardRwLock::unlock_exclusive(&self.0) }
    }
}

/// Returns a world whose storage registry is guarded by a `CountingLock`.
#[cfg(feature = "std")]
fn counting_world() -> World {
    World::builder().with_custom_lock::<CountingLock>().build()
}

/// Returns how many times `f` acquired a `CountingLock` from this thread.
#[cfg(feature = "std")]
fn registry_acquisitions(f: impl FnOnce()) -> usize {
    let before = ACQUISITIONS.with(|acquisitions| acquisitions.get());
    f();

    ACQUISITIONS.with(|acquisitions| acquisitions.get()) - before
}

/// Asserts that `CountingLock` counts registry accesses, so a test seeing no acquisition means
/// there wasn't any.
#[cfg(feature = "std")]
#[test]
fn counting_lock() {
    let world = counting_world();

    let acquisitions = registry_acquisitions(|| drop(world.borrow::<View<'_, Heap>>().unwrap()));

    assert!(acquisitions > 0);
}

/// Asserts that borrowing the entities through `EntitiesView` and `EntitiesViewMut` never takes
/// the registry lock.
#[cfg(feature = "std")]
#[test]
fn entities_views_skip_registry() {
    let world = counting_world();

    let acquisitions = registry_acquisitions(|| {
        drop(world.borrow::<EntitiesView<'_>>().unwrap());
        drop(world.borrow::<EntitiesViewMut<'_>>().unwrap());
    });

    assert_eq!(acquisitions, 0);
}

/// Asserts that the generic API reaches the entities under their canonical id without taking the
/// registry lock, by type and by id, shared and exclusive.
#[cfg(feature = "std")]
#[test]
fn generic_entities_skip_registry() {
    let world = counting_world();
    let all_storages = world.all_storages().unwrap();
    let id = StorageId::of::<Entities>();

    let acquisitions = registry_acquisitions(|| {
        drop(all_storages.custom_storage::<Entities>().unwrap());
        drop(all_storages.custom_storage_mut::<Entities>().unwrap());
        drop(all_storages.custom_storage_by_id(id).unwrap());
        drop(all_storages.custom_storage_mut_by_id(id).unwrap());
    });

    assert_eq!(acquisitions, 0);
}

/// Asserts that inserting through the generic API under the canonical entities id also skips the
/// registry lock, the entities always exist.
#[cfg(feature = "std")]
#[test]
fn generic_insert_entities_skips_registry() {
    let world = counting_world();
    let all_storages = world.all_storages().unwrap();
    let id = StorageId::of::<Entities>();

    let acquisitions = registry_acquisitions(|| {
        drop(
            all_storages
                .custom_storage_or_insert_by_id(id, Entities::new)
                .unwrap(),
        );
        drop(
            all_storages
                .custom_storage_or_insert_mut_by_id(id, Entities::new)
                .unwrap(),
        );
    });

    assert_eq!(acquisitions, 0);
}

/// Asserts that a generic exclusive borrow of the entities blocks `EntitiesView`: both use the
/// same borrow state. The view reports its usual entities error.
#[test]
fn generic_entities_block_view() {
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();

    let generic = all_storages
        .custom_storage_mut_by_id(StorageId::of::<Entities>())
        .unwrap();
    assert!(matches!(
        all_storages.borrow::<EntitiesView<'_>>(),
        Err(error::GetStorage::Entities(error::Borrow::Shared))
    ));

    drop(generic);
    assert!(all_storages.borrow::<EntitiesView<'_>>().is_ok());
}

/// Asserts that `EntitiesViewMut` blocks a generic borrow of the entities, which reports the
/// generic error for its id.
#[test]
fn view_blocks_generic_entities() {
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();
    let id = StorageId::of::<Entities>();

    let view = all_storages.borrow::<EntitiesViewMut<'_>>().unwrap();
    assert_eq!(
        all_storages.custom_storage_by_id(id).err(),
        Some(error::GetStorage::StorageBorrow {
            name: None,
            id,
            borrow: error::Borrow::Shared,
        })
    );

    drop(view);
    assert!(all_storages.custom_storage_by_id(id).is_ok());
}

/// Asserts that inserting through the generic API under the canonical entities id returns the
/// world's entities without running the factory, and leaves no entities entry in the registry.
#[test]
fn generic_insert_entities() {
    let mut world = World::new_for_test();
    let entity = world.add_entity(());
    let all_storages = world.all_storages().unwrap();

    let entities = all_storages
        .custom_storage_or_insert_by_id(StorageId::of::<Entities>(), || -> Entities {
            panic!("the entities already exist")
        })
        .unwrap();
    assert!(entities.is_alive(entity));
    drop(entities);

    let registry = all_storages.storages.read();
    assert!(registry.any(StorageId::of::<Entities>()).is_none());
}

/// Asserts that `iter_storages` and `iter_storages_mut` yield the entities exactly once, next to
/// the registered storages.
#[test]
fn iter_storages_entities() {
    let mut world = World::new_for_test();
    world.add_entity(Heap(vec![0]));
    let all_storages = world.all_storages().unwrap();

    let storages = all_storages.iter_storages();
    assert_eq!(storages.len(), 2);
    assert_eq!(
        storages
            .iter()
            .filter(|storage| storage.any().is::<Entities>())
            .count(),
        1
    );
    drop(storages);

    let storages = all_storages.iter_storages_mut();
    assert_eq!(storages.len(), 2);
    assert_eq!(
        storages
            .iter()
            .filter(|storage| storage.any().is::<Entities>())
            .count(),
        1
    );
}

/// Asserts that `iter_storages` skips the entities while they're borrowed exclusively, like any
/// storage it can't borrow, and still yields the others.
#[test]
fn iter_storages_borrowed_entities() {
    let mut world = World::new_for_test();
    world.add_entity(Heap(vec![0]));
    let all_storages = world.all_storages().unwrap();

    let view = all_storages.borrow::<EntitiesViewMut<'_>>().unwrap();
    let storages = all_storages.iter_storages();

    assert_eq!(storages.len(), 1);
    assert!(storages[0].any().is::<SparseSet<Heap>>());
    drop((storages, view));
}

/// Asserts that `AllStorages`' debug output counts the entities as a storage and lists them.
#[cfg(feature = "std")]
#[test]
fn debug_entities() {
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();

    let debug = format!("{:?}", *all_storages);

    assert!(debug.contains("storage_count: 1"), "{debug}");
    assert!(debug.contains(type_name::<Entities>()), "{debug}");
}

/// Asserts that the memory usage report includes the entities, and counts them as a storage it
/// couldn't borrow while they're borrowed exclusively.
#[cfg(feature = "std")]
#[test]
fn memory_usage_entities() {
    let world = World::new_for_test();
    let all_storages = world.all_storages().unwrap();

    let report = format!("{:?}", all_storages.memory_usage());
    assert!(report.contains(type_name::<Entities>()), "{report}");

    let view = all_storages.borrow::<EntitiesViewMut<'_>>().unwrap();
    let report = format!("{:?}", all_storages.memory_usage());
    assert!(
        report.contains("1 storages could not be borrored"),
        "{report}"
    );
    drop(view);
}

/// Returns a unique storage holding `counted(value)`, to register through a custom path.
fn custom_path_unique(value: u32, counter: &Arc<AtomicUsize>) -> UniqueStorage<Counted> {
    UniqueStorage::new(counted(value, counter), TrackingTimestamp::origin())
}

/// Asserts that `add_unique` registers a new unique in the unique map only.
#[test]
fn add_unique_map() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();

    world.add_unique(counted(0, &counter));

    let all_storages = world.all_storages().unwrap();
    let registry = all_storages.storages.read();
    assert!(registry.uniques.contains_key(&id));
    assert!(!registry.components_and_custom.contains_key(&id));
}

/// Asserts that a unique registered through a custom path stays in the component and custom map,
/// and unique views still find it.
#[test]
fn custom_path_unique_view() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();

    world
        .add_custom_storage(id, custom_path_unique(0, &counter))
        .unwrap();

    assert_eq!(
        world.borrow::<UniqueView<'_, Counted>>().unwrap().values,
        [0]
    );
    let all_storages = world.all_storages().unwrap();
    let registry = all_storages.storages.read();
    assert!(registry.components_and_custom.contains_key(&id));
    assert!(!registry.uniques.contains_key(&id));
}

/// Asserts that `add_unique` replaces the value of a unique registered through a custom path in
/// place: the storage stays in the component and custom map and the old value is dropped once.
#[test]
fn replace_custom_path_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    world
        .add_custom_storage(id, custom_path_unique(0, &counter))
        .unwrap();

    world.add_unique(counted(1, &counter));

    assert_eq!(drops(&counter), 1);
    let view = world.borrow::<UniqueView<'_, Counted>>().unwrap();
    assert_eq!(view.values, [1]);
    assert!(view.is_modified());
    drop(view);
    let all_storages = world.all_storages().unwrap();
    let registry = all_storages.storages.read();
    assert!(registry.components_and_custom.contains_key(&id));
    assert!(!registry.uniques.contains_key(&id));
}

/// Asserts that `remove_unique` removes a unique registered through a custom path and hands its
/// value over, leaving no entry for its id.
#[test]
fn remove_custom_path_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    world
        .add_custom_storage(id, custom_path_unique(0, &counter))
        .unwrap();
    let all_storages = world.all_storages().unwrap();

    let unique = all_storages.remove_unique::<Counted>().unwrap();

    assert_eq!(unique.values, [0]);
    assert_eq!(drops(&counter), 0);
    assert!(all_storages.storages.read().any(id).is_none());
}

/// Asserts that replacing a foreign custom storage registered under a unique's id moves the id
/// to the unique map: the new unique is there and the custom map no longer has the id.
#[test]
fn replace_foreign_map() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    world.add_custom_storage(id, counted(0, &counter)).unwrap();

    world.add_unique(counted(1, &counter));

    assert_eq!(drops(&counter), 1);
    let all_storages = world.all_storages().unwrap();
    let registry = all_storages.storages.read();
    assert!(registry.uniques.contains_key(&id));
    assert!(!registry.components_and_custom.contains_key(&id));
}

/// Asserts that generic insertion under a unique's id returns the unique without running the
/// factory, and doesn't add a second entry for the id.
#[test]
fn generic_insert_existing_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    world.add_unique(counted(0, &counter));
    let all_storages = world.all_storages().unwrap();

    let storage = all_storages
        .custom_storage_or_insert_by_id(id, || -> UniqueStorage<Counted> {
            panic!("the unique already exists")
        })
        .unwrap();
    assert_eq!(storage.value.values, [0]);
    drop(storage);

    let registry = all_storages.storages.read();
    assert!(registry.uniques.contains_key(&id));
    assert!(!registry.components_and_custom.contains_key(&id));
}

/// Asserts that generic insertion under a component's id returns the component storage without
/// running the factory.
#[test]
fn generic_insert_existing_component() {
    let mut world = World::new_for_test();
    let entity = world.add_entity(Heap(vec![0]));
    let all_storages = world.all_storages().unwrap();

    let storage = all_storages
        .custom_storage_or_insert(|| -> SparseSet<Heap> { panic!("the storage already exists") })
        .unwrap();

    assert_eq!(storage.private_get(entity).unwrap().0, [0]);
}

/// Asserts that `iter_storages` skips a unique borrowed exclusively and still yields the
/// entities and the component storages.
#[test]
fn iter_storages_borrowed_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::new_for_test();
    world.add_entity(Heap(vec![0]));
    world.add_unique(counted(0, &counter));
    let all_storages = world.all_storages().unwrap();

    let unique = all_storages.borrow::<UniqueViewMut<'_, Counted>>().unwrap();
    let storages = all_storages.iter_storages();

    assert_eq!(storages.len(), 2);
    assert!(
        storages
            .iter()
            .all(|storage| !storage.any().is::<UniqueStorage<Counted>>())
    );
    drop((storages, unique));
}

/// Custom storage that copies itself into the worlds its world is cloned to.
#[derive(Clone)]
struct Cloneable(Counted);

impl Storage for Cloneable {
    fn try_clone(&self, _other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        Some(SBoxBuilder::new(self.clone()))
    }
}

/// Asserts that cloning a unique into a world holding a custom storage under the same id
/// replaces it: the id ends up in the unique map only and the custom storage is dropped once.
#[test]
fn clone_unique_over_custom() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut source = World::new_for_test();
    source.add_unique(counted(0, &counter));
    source.register_clone::<UniqueStorage<Counted>>();
    let mut destination = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    destination
        .add_custom_storage(id, Cloneable(counted(1, &counter)))
        .unwrap();

    let source_storages = source.all_storages().unwrap();
    source_storages.clone_storages_to(destination.all_storages.get_mut());
    drop(source_storages);

    assert_eq!(drops(&counter), 1);
    assert_eq!(
        destination
            .borrow::<UniqueView<'_, Counted>>()
            .unwrap()
            .values,
        [0]
    );
    let registry = destination.all_storages.get_mut().storages.get_mut();
    assert!(registry.uniques.contains_key(&id));
    assert!(!registry.components_and_custom.contains_key(&id));
}

/// Asserts that cloning a custom storage into a world holding a unique under the same id
/// replaces it: the id ends up in the component and custom map only and the unique is dropped
/// once.
#[test]
fn clone_custom_over_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let source = World::new_for_test();
    let id = StorageId::of::<UniqueStorage<Counted>>();
    source
        .add_custom_storage(id, Cloneable(counted(0, &counter)))
        .unwrap();
    let mut destination = World::new_for_test();
    destination.add_unique(counted(1, &counter));

    let source_storages = source.all_storages().unwrap();
    source_storages.clone_storages_to(destination.all_storages.get_mut());
    drop(source_storages);

    assert_eq!(drops(&counter), 1);
    let all_storages = destination.all_storages().unwrap();
    let clone = all_storages.custom_storage_by_id(id).unwrap();
    assert_eq!(
        clone.any().downcast_ref::<Cloneable>().unwrap().0.values,
        [0]
    );
    drop(clone);
    let registry = all_storages.storages.read();
    assert!(registry.components_and_custom.contains_key(&id));
    assert!(!registry.uniques.contains_key(&id));
}

/// Asserts that a workload using a unique registered through a custom path finds it present.
#[test]
fn workload_custom_path_unique() {
    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world
        .add_custom_storage(
            StorageId::of::<UniqueStorage<Counted>>(),
            custom_path_unique(0, &counter),
        )
        .unwrap();

    let presence = Workload::new("")
        .with_system(|_: UniqueView<'_, Counted>| {})
        .are_all_uniques_present_in_world(&world);

    assert!(presence.is_ok());
}

/// Asserts that two threads inserting the same missing storage at the same time insert it once:
/// the factory runs once and both threads borrow the inserted storage.
#[cfg(feature = "std")]
#[test]
fn concurrent_insertion() {
    let counter = Arc::new(AtomicUsize::new(0));
    let calls = AtomicUsize::new(0);
    let world = World::new_for_test();
    let barrier = std::sync::Barrier::new(2);

    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                let all_storages = world.all_storages().unwrap();
                barrier.wait();

                let storage = all_storages
                    .custom_storage_or_insert_by_id(StorageId::Custom(0), || {
                        calls.fetch_add(1, Ordering::Relaxed);
                        Counted::new(&counter)
                    })
                    .unwrap();

                assert_eq!(storage.values, [1, 2]);
            });
        }
    });

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let all_storages = world.all_storages().unwrap();
    assert_eq!(all_storages.storages.read().len(), 1);
}
