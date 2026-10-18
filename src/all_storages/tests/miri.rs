//! Focused Miri scenarios for the storage registry in `AllStorages`.
//!
//! The registry is a hash map of boxed storages. Storages can be added through `&AllStorages`
//! while others are borrowed, so these tests keep borrows alive across insertions and check
//! every erased storage is reached with the right type and dropped exactly once.

use crate::all_storages::CustomStorageAccess;
use crate::sparse_set::SparseSet;
use crate::storage::{Storage, StorageId};
use crate::{track, Component, EntityId, Get, Unique, UniqueView, View, World};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
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
    let world = World::new();

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
    let world = World::new();
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
    let world = World::new();
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
    let world = World::new();
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

/// Asserts that the wrapper returned by `AllStorages::iter` owns its storage borrow: writes
/// through yielded references are valid while it's alive, the storage can't be borrowed
/// elsewhere until it's dropped, and the writes are visible afterwards.
#[test]
fn iter_borrows() {
    let mut world = World::new();
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
    let mut source = World::new();
    let mut destination = World::new();
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
#[test]
fn clone() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut world = World::new();
    world.register_clone::<SparseSet<Counted>>();
    let entity = world.add_entity(counted(0, &counter));

    let copy = world.clone();
    assert_eq!(copy.get::<&Counted>(entity).unwrap().values, [0]);

    let mut other = World::new();
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
    let mut world = World::new();
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

    // An entity in both groups, then a deletion resetting `Counted` and `Marker` before the
    // next regroup places everything again.
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
#[cfg(feature = "thread_local")]
struct Local {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
    _non_send_sync: core::marker::PhantomData<alloc::rc::Rc<()>>,
}

#[cfg(feature = "thread_local")]
impl Local {
    fn new(drops: &Arc<AtomicUsize>) -> Local {
        Local {
            values: vec![1, 2],
            drops: drops.clone(),
            _non_send_sync: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "thread_local")]
impl Drop for Local {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(feature = "thread_local")]
impl Storage for Local {}

#[cfg(feature = "thread_local")]
impl Unique for Local {}

/// Asserts that a `!Send + !Sync` unique and custom storage are only reachable from the world's
/// thread: another thread is rejected with `WrongThread`, removing the unique included, without
/// changing anything. The world's thread then writes both and removes the unique, which casts
/// its erased box back to the unique storage, and each value is dropped exactly once.
#[cfg(feature = "thread_local")]
#[test]
fn thread_local_storages() {
    use crate::borrow::NonSendSync;
    use crate::error;
    use crate::UniqueViewMut;

    let counter = Arc::new(AtomicUsize::new(0));
    let world = World::new();
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
            assert!(all_storages
                .borrow::<NonSendSync<UniqueViewMut<'_, Local>>>()
                .is_err());
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
