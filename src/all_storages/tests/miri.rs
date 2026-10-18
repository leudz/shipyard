//! Focused Miri scenarios for the storage registry in `AllStorages`.
//!
//! The registry is a hash map of boxed storages. Storages can be added through `&AllStorages`
//! while others are borrowed, so these tests keep borrows alive across insertions and check
//! every erased storage is reached with the right type and dropped exactly once.

use crate::all_storages::CustomStorageAccess;
use crate::storage::{Storage, StorageId};
use crate::{track, Component, Get, Unique, UniqueView, View, World};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Heap-owning value that counts how many times it's dropped.
///
/// Used both as a custom storage and as a unique.
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

struct Heap(Vec<u32>);

impl Component for Heap {
    type Tracking = track::Untracked;
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
