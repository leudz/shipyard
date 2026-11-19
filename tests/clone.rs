use shipyard::{
    AllStoragesView, AllStoragesViewMut, Component, Delete, EntityId, IntoIter, Remove, Unique,
    UniqueStorage, UniqueViewMut, View, ViewMut, World, sparse_set::SparseSet, track,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[derive(Clone)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Clone)]
struct B(u32);
impl Component for B {
    type Tracking = track::Untracked;
}

/// Not cloneable.
struct C;
impl Component for C {
    type Tracking = track::Untracked;
}

#[derive(Clone)]
struct Counter;
impl Unique for Counter {}

/// Returns `(entity, a, b)` for every entity owning A and B, sorted, iterating them together.
fn ab_contents(world: &World) -> Vec<(EntityId, u32, u32)> {
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    let mut contents = Vec::new();
    (&a, &b)
        .iter()
        .with_id()
        .for_each(|(entity, (a, b))| contents.push((entity, a.0, b.0)));
    contents.sort_unstable();

    contents
}

#[test]
fn clone_world() {
    let mut world = World::new();

    let eid = world.add_entity(USIZE(1));

    let world2 = world.clone();

    assert!(world2.borrow::<View<USIZE>>().unwrap().is_empty());

    world.register_clone::<SparseSet<USIZE>>();

    let world3 = world.clone();

    world3.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.len(), 1);

        assert_eq!(usizes[eid], USIZE(1));
    });
}

/// Asserts that a clone keeps runtime tracking, treats copied components as newly inserted,
/// and starts its modification, removal, and deletion history fresh.
#[test]
fn clone_tracking() {
    let mut source = World::new();
    source.track_all::<USIZE>();
    source.register_clone::<SparseSet<USIZE>>();

    let removed_in_source = source.add_entity(USIZE(1));
    let deleted_in_source = source.add_entity(USIZE(2));
    let removed_in_clone = source.add_entity(USIZE(3));
    let deleted_in_clone = source.add_entity(USIZE(4));

    {
        let mut values = source.borrow::<ViewMut<'_, USIZE, track::All>>().unwrap();
        values[removed_in_clone] = USIZE(30);
        assert!(values.is_modified(removed_in_clone));
        assert_eq!(values.remove(removed_in_source), Some(USIZE(1)));
        assert!(values.delete(deleted_in_source));
    }

    let clone = source.clone();
    {
        let values = clone.borrow::<View<'_, USIZE, track::All>>().unwrap();
        assert!(values.is_inserted(removed_in_clone));
        assert!(values.is_inserted(deleted_in_clone));
        assert_eq!(values[removed_in_clone], USIZE(30));
    }

    // Copied source events could initially be beyond the clone's tracking window.
    let source_time = source.get_tracking_timestamp();
    while clone.get_tracking_timestamp().is_older_than(source_time) {
        clone.borrow::<()>().unwrap();
    }

    let mut values = clone.borrow::<ViewMut<'_, USIZE, track::All>>().unwrap();
    assert!(!values.is_modified(removed_in_clone));
    assert_eq!(values.removed().next(), None);
    assert_eq!(values.deleted().next(), None);

    values[removed_in_clone] = USIZE(31);
    assert!(values.is_modified(removed_in_clone));
    assert_eq!(values.remove(removed_in_clone), Some(USIZE(31)));
    assert!(values.delete(deleted_in_clone));
    assert_eq!(values.removed().collect::<Vec<_>>(), [removed_in_clone]);
    assert_eq!(
        values.deleted().collect::<Vec<_>>(),
        [(deleted_in_clone, &USIZE(4))]
    );
}

#[test]
fn clone_entity() {
    let mut world = World::new();

    let eid = world.add_entity(USIZE(1));
    world.add_entity(USIZE(2));

    let mut world2 = world.clone();

    assert!(world2.borrow::<View<USIZE>>().unwrap().is_empty());

    world.register_clone::<SparseSet<USIZE>>();

    world.clone_entity_to(&mut world2, eid);

    world2.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.len(), 1);

        assert_eq!(usizes[eid], USIZE(1));
    });
}

/// Asserts that a world's clone keeps its overlapping groups, a member without a registered
/// clone function included, and iterates the copied components correctly before and after
/// a regroup.
#[test]
fn clone_grouped_world() {
    let mut world = World::builder()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let first = world.add_entity((A(0), B(10), C));
    let second = world.add_entity((A(1), B(11)));
    world.add_entity((A(2), C));
    world.regroup();
    world.register_clone::<SparseSet<A>>();
    world.register_clone::<SparseSet<B>>();

    let clone = world.clone();

    let expected = vec![(first, 0, 10), (second, 1, 11)];
    assert_eq!(ab_contents(&clone), expected);
    assert!(clone.borrow::<View<C>>().unwrap().is_empty());
    // C isn't cloned, it's still part of the clone's groups.
    clone.regroup_set::<C>().unwrap();
    assert_eq!(ab_contents(&clone), expected);
}

/// Asserts that a world's clone has the same alive entities and reuses deleted ids like the
/// original does.
#[test]
fn clone_entities() {
    let mut world = World::new();
    let deleted = world.add_entity(());
    let alive = world.add_entity(());
    world.delete_entity(deleted);

    let mut clone = world.clone();

    assert!(!clone.is_entity_alive(deleted));
    assert!(clone.is_entity_alive(alive));
    assert_eq!(clone.add_entity(()), world.add_entity(()));
}

/// Asserts that a world's clone doesn't get the original's deletion callback.
#[test]
fn clone_without_on_deletion() {
    let mut world = World::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    world.on_deletion(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    let entity = world.add_entity(());

    let mut clone = world.clone();
    clone.delete_entity(entity);
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    world.delete_entity(entity);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

/// Asserts that cloning storages into an existing world replaces its entities, deletion callback
/// included.
#[test]
fn clone_storages_replace_entities() {
    let mut source = World::new();
    let entity = source.add_entity(());
    let mut destination = World::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    destination.on_deletion(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    });

    let source_storages = source.borrow::<AllStoragesView>().unwrap();
    let mut destination_storages = destination.borrow::<AllStoragesViewMut>().unwrap();
    source_storages.clone_storages_to(&mut destination_storages);
    drop((source_storages, destination_storages));

    assert!(destination.is_entity_alive(entity));
    destination.delete_entity(entity);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

/// Asserts that cloning an entity to another world works while a unique is borrowed exclusively,
/// uniques don't hold components.
#[test]
fn clone_entity_borrowed_unique() {
    let mut world = World::new();
    world.add_unique(Counter);
    world.register_clone::<SparseSet<USIZE>>();
    let entity = world.add_entity(USIZE(1));
    let other = World::new();

    let all_storages = world.borrow::<AllStoragesView>().unwrap();
    let unique = all_storages.borrow::<UniqueViewMut<Counter>>().unwrap();
    let mut other_storages = other.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clone_entity_to(&mut other_storages, entity);
    drop(other_storages);
    drop(unique);
    drop(all_storages);

    assert_eq!(other.borrow::<View<USIZE>>().unwrap()[entity], USIZE(1));
}

/// Asserts that cloning components to another world's entity works while a unique is borrowed
/// exclusively, uniques don't hold components.
#[test]
fn clone_components_borrowed_unique() {
    let mut world = World::new();
    world.add_unique(Counter);
    world.register_clone::<SparseSet<USIZE>>();
    let from = world.add_entity(USIZE(1));
    let mut other = World::new();
    let to = other.add_entity(());

    let all_storages = world.borrow::<AllStoragesView>().unwrap();
    let unique = all_storages.borrow::<UniqueViewMut<Counter>>().unwrap();
    let mut other_storages = other.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clone_components_to(&mut other_storages, from, to);
    drop(other_storages);
    drop(unique);
    drop(all_storages);

    assert_eq!(other.borrow::<View<USIZE>>().unwrap()[to], USIZE(1));
}

/// Asserts that cloning all storages still fails while a unique is borrowed exclusively, the
/// uniques are cloned too.
#[test]
#[should_panic]
fn clone_storages_borrowed_unique() {
    let mut world = World::new();
    world.add_unique(Counter);
    world.register_clone::<UniqueStorage<Counter>>();
    let other = World::new();

    let all_storages = world.borrow::<AllStoragesView>().unwrap();
    let _unique = all_storages.borrow::<UniqueViewMut<Counter>>().unwrap();
    let mut other_storages = other.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clone_storages_to(&mut other_storages);
}

#[cfg(feature = "thread_local")]
mod thread_local {
    use shipyard::{
        Component, EntitiesViewMut, View, ViewMut, World,
        borrow::{NonSend, NonSendSync, NonSync},
        error::{Borrow, GetStorage},
        sparse_set::SparseSet,
        track,
    };
    use std::{cell::Cell, marker::PhantomData, rc::Rc, sync::MutexGuard, thread};

    /// Asserts that registering a `!Send` component before or after insertion preserves shared
    /// access from other threads and keeps exclusive access on the clone's thread.
    #[test]
    fn non_send() {
        #[derive(Clone)]
        struct SyncOnly(u32, PhantomData<MutexGuard<'static, ()>>);

        impl Component for SyncOnly {
            type Tracking = track::Untracked;
        }

        for register_first in [true, false] {
            let mut world = World::new();
            if register_first {
                world.register_clone::<NonSend<SparseSet<SyncOnly>>>();
            }
            let entity = world.run(
                |mut entities: EntitiesViewMut, mut values: NonSend<ViewMut<SyncOnly>>| {
                    entities.add_entity(&mut *values, SyncOnly(1, PhantomData))
                },
            );
            if !register_first {
                world.register_clone::<NonSend<SparseSet<SyncOnly>>>();
            }

            let clone = world.clone();
            thread::scope(|scope| {
                scope.spawn(|| {
                    let values = clone.borrow::<NonSend<View<SyncOnly>>>().unwrap();
                    assert_eq!(values[entity].0, 1);
                    drop(values);
                    assert!(matches!(
                        clone.borrow::<NonSend<ViewMut<SyncOnly>>>(),
                        Err(GetStorage::StorageBorrow {
                            borrow: Borrow::WrongThread,
                            ..
                        })
                    ));
                });
            });
            assert!(clone.borrow::<NonSend<ViewMut<SyncOnly>>>().is_ok());
        }
    }

    /// Asserts that registering a `!Sync` component before or after insertion lets another
    /// thread borrow the clone once the current shared borrow is released.
    #[test]
    fn non_sync() {
        #[derive(Clone)]
        struct SendOnly(Cell<u32>);

        impl Component for SendOnly {
            type Tracking = track::Untracked;
        }

        for register_first in [true, false] {
            let mut world = World::new();
            if register_first {
                world.register_clone::<NonSync<SparseSet<SendOnly>>>();
            }
            let entity = world.run(
                |mut entities: EntitiesViewMut, mut values: NonSync<ViewMut<SendOnly>>| {
                    entities.add_entity(&mut *values, SendOnly(Cell::new(2)))
                },
            );
            if !register_first {
                world.register_clone::<NonSync<SparseSet<SendOnly>>>();
            }

            let clone = world.clone();
            thread::scope(|scope| {
                let values = clone.borrow::<NonSync<View<SendOnly>>>().unwrap();
                scope
                    .spawn(|| {
                        assert!(matches!(
                            clone.borrow::<NonSync<View<SendOnly>>>(),
                            Err(GetStorage::StorageBorrow {
                                borrow: Borrow::Shared,
                                ..
                            })
                        ));
                    })
                    .join()
                    .unwrap();
                drop(values);

                scope.spawn(|| {
                    let values = clone.borrow::<NonSync<ViewMut<SendOnly>>>().unwrap();
                    assert_eq!(values[entity].0.get(), 2);
                });
            });
        }
    }

    /// Asserts that registering a `!Send + !Sync` component before or after insertion keeps
    /// both shared and exclusive access to the clone on its thread.
    #[test]
    fn non_send_sync() {
        #[derive(Clone)]
        struct Local(Rc<u32>);

        impl Component for Local {
            type Tracking = track::Untracked;
        }

        for register_first in [true, false] {
            let mut world = World::new();
            if register_first {
                world.register_clone::<NonSendSync<SparseSet<Local>>>();
            }
            let entity = world.run(
                |mut entities: EntitiesViewMut, mut values: NonSendSync<ViewMut<Local>>| {
                    entities.add_entity(&mut *values, Local(Rc::new(3)))
                },
            );
            if !register_first {
                world.register_clone::<NonSendSync<SparseSet<Local>>>();
            }

            let clone = world.clone();
            thread::scope(|scope| {
                scope.spawn(|| {
                    assert!(matches!(
                        clone.borrow::<NonSendSync<View<Local>>>(),
                        Err(GetStorage::StorageBorrow {
                            borrow: Borrow::WrongThread,
                            ..
                        })
                    ));
                    assert!(matches!(
                        clone.borrow::<NonSendSync<ViewMut<Local>>>(),
                        Err(GetStorage::StorageBorrow {
                            borrow: Borrow::WrongThread,
                            ..
                        })
                    ));
                });
            });
            let values = clone.borrow::<NonSendSync<View<Local>>>().unwrap();
            assert_eq!(*values[entity].0, 3);
            drop(values);
            assert!(clone.borrow::<NonSendSync<ViewMut<Local>>>().is_ok());
        }
    }
}
