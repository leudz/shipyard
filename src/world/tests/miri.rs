//! Focused Miri scenarios for borrowing through `World`.
//!
//! Views, component references and iterators split their guards into raw references plus the
//! borrows protecting them. These tests use the references while those borrows are alive, then
//! check every borrow (including the one on `AllStorages`) is released when they're dropped.

use crate::get_component::{Ref, RefMut};
use crate::views::UniqueOrInitViewMut;
use crate::{
    track, AllStoragesViewMut, Component, Get, Unique, UniqueView, UniqueViewMut, View, ViewMut,
    World,
};
use alloc::vec;
use alloc::vec::Vec;

struct Heap(Vec<u32>);

impl Component for Heap {
    type Tracking = track::Untracked;
}

struct Other(u32);

impl Component for Other {
    type Tracking = track::Untracked;
}

struct Config(Vec<u32>);

impl Unique for Config {}

/// Counts how many times it's dropped.
#[cfg(feature = "std")]
struct Counter(alloc::sync::Arc<core::sync::atomic::AtomicUsize>);

#[cfg(feature = "std")]
impl Drop for Counter {
    fn drop(&mut self) {
        self.0.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }
}

/// Asserts the `ViewMut::as_view` reborrow through `World`: writes made through the `ViewMut`
/// are read through the `View` it hands out, and the storage stays exclusively borrowed until
/// the `ViewMut` itself is dropped, not just its `View`. The `AllStorages` borrow it carries is
/// released with it.
#[test]
fn as_view() {
    let mut world = World::new_for_test();
    let entity = world.add_entity(Heap(vec![1]));

    let mut heaps = world.borrow::<ViewMut<'_, Heap>>().unwrap();
    (&mut heaps).get(entity).unwrap().0.push(2);
    {
        let view = heaps.as_view();
        assert_eq!((&view).get(entity).unwrap().0, [1, 2]);
        assert!(world.borrow::<View<'_, Heap>>().is_err());
    }
    assert!(world.borrow::<View<'_, Heap>>().is_err());
    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_err());
    drop(heaps);

    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_ok());
    let heaps = world.borrow::<View<'_, Heap>>().unwrap();
    assert_eq!((&heaps).get(entity).unwrap().0, [1, 2]);
}

/// Asserts that `World::get` hands out references that hold both their storage borrow and the
/// `AllStorages` borrow, still after `RefMut::map`/`Ref::map` narrow them to a field, and that
/// both are released once the reference is dropped.
#[test]
fn get_refs() {
    let mut world = World::new_for_test();
    let entity = world.add_entity(Heap(vec![1]));

    let mut values = RefMut::map(world.get::<&mut Heap>(entity).unwrap(), |heap| &mut heap.0);
    values.push(2);
    assert!(world.borrow::<View<'_, Heap>>().is_err());
    drop(values);

    let values = Ref::map(world.get::<&Heap>(entity).unwrap(), |heap| &heap.0);
    assert!(world.borrow::<ViewMut<'_, Heap>>().is_err());
    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_err());
    assert_eq!(**values, [1, 2]);
    drop(values);

    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_ok());
}

/// Asserts that a `World::get` that fails because the component is missing releases the storage
/// and `AllStorages` borrows it took before finding out.
#[test]
fn get_missing() {
    let mut world = World::new_for_test();
    let entity = world.add_entity(Other(0));

    assert!(world.get::<&mut Heap>(entity).is_err());

    assert!(world.borrow::<ViewMut<'_, Heap>>().is_ok());
    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_ok());
}

/// Asserts that a tuple borrow failing partway releases the views it already acquired:
/// `View<Other>` succeeds, then `ViewMut<Heap>` fails because it's already borrowed, and
/// afterwards `Other` can be borrowed exclusively.
#[test]
fn tuple_partial() {
    let world = World::new_for_test();
    let heaps = world.borrow::<ViewMut<'_, Heap>>().unwrap();

    assert!(world
        .borrow::<(View<'_, Other>, ViewMut<'_, Heap>)>()
        .is_err());
    assert!(world.borrow::<ViewMut<'_, Other>>().is_ok());

    drop(heaps);
    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_ok());
}

/// Asserts that a unique's exclusive view writes through to the stored value and keeps other
/// views out, and that the views `get_unique` returns hold their borrow the same way.
#[test]
fn unique_views() {
    let world = World::new_for_test();
    world.add_unique(Config(vec![1]));

    let mut config = world.borrow::<UniqueViewMut<'_, Config>>().unwrap();
    config.0.push(2);
    assert!(world.borrow::<UniqueView<'_, Config>>().is_err());
    drop(config);

    let config = world.get_unique::<&Config>().unwrap();
    assert!(world.get_unique::<&mut Config>().is_err());
    assert_eq!(config.0, [1, 2]);
    drop(config);

    world.get_unique::<&mut Config>().unwrap().0.push(3);
    let config = world.borrow::<UniqueView<'_, Config>>().unwrap();
    assert_eq!(config.0, [1, 2, 3]);
}

/// Asserts that `UniqueOrInitViewMut` lazily inserts a missing unique while it holds the
/// `AllStorages` borrow, then hands out an exclusive view of it that keeps other views out until
/// the lazy view is dropped.
#[test]
fn unique_or_init() {
    let world = World::new_for_test();

    let mut lazy = world.borrow::<UniqueOrInitViewMut<'_, Config>>().unwrap();
    assert!(lazy.get().is_none());
    lazy.get_or_init(|| Config(vec![1])).unwrap();
    lazy.get_mut().unwrap().0.push(2);
    assert!(world.borrow::<UniqueView<'_, Config>>().is_err());
    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_err());
    drop(lazy);

    assert_eq!(world.borrow::<UniqueView<'_, Config>>().unwrap().0, [1, 2]);
}

/// Asserts that `World::iter`'s wrapper owns its storage borrows across several traversals: each
/// `&mut iter` pass starts over from the first component and can write, other borrows are
/// rejected while the wrapper is alive, and the writes are visible after it's dropped.
#[test]
fn world_iter() {
    let mut world = World::new_for_test();
    let entity0 = world.add_entity((Heap(vec![0]), Other(10)));
    let entity1 = world.add_entity((Heap(vec![1]), Other(11)));

    let mut iter = world.iter::<(&mut Heap, &Other)>();
    for (heap, other) in &mut iter {
        heap.0.push(other.0);
    }
    for (heap, _) in &mut iter {
        heap.0.push(20);
    }
    assert!(world.borrow::<View<'_, Heap>>().is_err());
    assert!(world.borrow::<ViewMut<'_, Other>>().is_err());
    drop(iter);

    let heaps = world.borrow::<View<'_, Heap>>().unwrap();
    assert_eq!((&heaps).get(entity0).unwrap().0, [0, 10, 20]);
    assert_eq!((&heaps).get(entity1).unwrap().0, [1, 11, 20]);
}

/// Asserts that a system unwinding while it holds views releases them: a drop-counted value it
/// owns is dropped exactly once, the writes it made before panicking are kept, and the storages
/// it borrowed (and `AllStorages`) can be borrowed exclusively again.
#[cfg(feature = "std")]
#[test]
fn system_unwind() {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    let mut world = World::new_for_test();
    let entity = world.add_entity(Heap(vec![1]));
    world.add_unique(Config(vec![]));
    let drops = Arc::new(AtomicUsize::new(0));
    let counter = Counter(drops.clone());

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        world.run(
            move |mut heaps: ViewMut<'_, Heap>, mut config: UniqueViewMut<'_, Config>| {
                let _counter = counter;
                (&mut heaps).get(entity).unwrap().0.push(2);
                config.0.push(3);
                panic!("system panicked while holding views");
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::Relaxed), 1);

    let (heaps, config) = world
        .borrow::<(ViewMut<'_, Heap>, UniqueViewMut<'_, Config>)>()
        .unwrap();
    assert_eq!((&heaps).get(entity).unwrap().0, [1, 2]);
    assert_eq!(config.0, [3]);
    drop((heaps, config));

    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_ok());
}

/// Heap-owning component that is `Sync` but not `Send`, like a type holding a `MutexGuard`.
#[cfg(feature = "thread_local")]
struct SyncOnly(
    Vec<u32>,
    core::marker::PhantomData<std::sync::MutexGuard<'static, ()>>,
);

#[cfg(feature = "thread_local")]
impl Component for SyncOnly {
    type Tracking = track::Untracked;
}

/// Asserts the default thread id path through public world operations. From another thread,
/// `AllStorages` can be shared but not borrowed exclusively, `Send + Sync` components and shared
/// views of `!Send` ones stay readable, and exclusive views of `!Send` ones are rejected. The
/// world's thread then writes everything.
#[cfg(feature = "thread_local")]
#[test]
fn other_thread() {
    use crate::borrow::NonSend;
    use crate::EntitiesView;

    let mut world = World::new_for_test();
    let entity = world.add_entity(Heap(vec![1]));
    world.run(
        |entities: EntitiesView<'_>, mut sync_only: NonSend<ViewMut<'_, SyncOnly>>| {
            let component = SyncOnly(vec![2], core::marker::PhantomData);
            entities.add_component(entity, &mut *sync_only, component);
        },
    );

    std::thread::scope(|scope| {
        scope.spawn(|| {
            assert!(world.borrow::<AllStoragesViewMut<'_>>().is_err());

            let heaps = world.borrow::<View<'_, Heap>>().unwrap();
            assert_eq!((&heaps).get(entity).unwrap().0, [1]);

            let sync_only = world.borrow::<NonSend<View<'_, SyncOnly>>>().unwrap();
            assert_eq!((&*sync_only).get(entity).unwrap().0, [2]);
            assert!(world.borrow::<NonSend<ViewMut<'_, SyncOnly>>>().is_err());
        });
    });

    let mut sync_only = world.borrow::<NonSend<ViewMut<'_, SyncOnly>>>().unwrap();
    (&mut *sync_only).get(entity).unwrap().0.push(3);
    assert_eq!((&*sync_only).get(entity).unwrap().0, [2, 3]);
    drop(sync_only);

    assert!(world.borrow::<AllStoragesViewMut<'_>>().is_ok());
}
