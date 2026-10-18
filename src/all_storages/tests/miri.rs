//! Focused Miri scenarios for the storage registry in `AllStorages`.
//!
//! The registry is a hash map of boxed storages. Storages can be added through `&AllStorages`
//! while others are borrowed, so these tests keep borrows alive across insertions and check
//! every erased storage is reached with the right type and dropped exactly once.

use crate::all_storages::CustomStorageAccess;
#[cfg(all(feature = "std", feature = "thread_local"))]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::{Component, Unique};
use crate::entity_id::EntityId;
#[cfg(feature = "std")]
use crate::error;
use crate::get::Get;
#[cfg(feature = "std")]
use crate::sparse_set::SparseSet;
use crate::storage::{Storage, StorageId};
use crate::track;
use crate::unique::UniqueStorage;
#[cfg(feature = "std")]
use crate::views::UniqueViewMut;
use crate::views::{UniqueView, View};
use crate::world::World;
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
        let capacity = all_storages.storages.read().capacity();

        for id in 1..=8 {
            let _ = all_storages
                .custom_storage_or_insert_by_id(StorageId::Custom(id), || Counted::new(&drops))
                .unwrap();
        }
        assert!(all_storages.storages.read().capacity() > capacity);

        assert_eq!(first.values, [1, 2]);
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
    let mut world = World::new_for_test();
    world.group::<(Counted, Aligned)>();
    world.group::<(Counted, Marker)>();

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
