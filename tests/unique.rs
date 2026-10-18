use core::any::type_name;
use shipyard::advanced::StorageId;
use shipyard::error;
use shipyard::views::{
    UniqueOrDefaultView, UniqueOrDefaultViewMut, UniqueOrInitView, UniqueOrInitViewMut,
};
use shipyard::*;

#[derive(Clone, Default, Debug, PartialEq)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}
impl Unique for USIZE {}

#[test]
fn unique_storage() {
    let world = World::new();
    world.add_unique(USIZE(0));

    world.run(|mut x: UniqueViewMut<USIZE>| {
        x.0 += 1;
    });
    world.run(|x: UniqueView<USIZE>| {
        assert_eq!(x.0, 1);
    });

    world.remove_unique::<USIZE>().unwrap();

    if let Some(get_error) = world.borrow::<UniqueViewMut<USIZE>>().err() {
        assert_eq!(
            get_error,
            shipyard::error::GetStorage::MissingStorage {
                name: Some(type_name::<UniqueStorage<USIZE>>()),
                id: StorageId::of::<UniqueStorage<USIZE>>(),
            }
        );
    } else {
        panic!()
    }
}

#[test]
fn not_unique_storage() {
    let world = World::new();

    match world.borrow::<UniqueView<USIZE>>().err() {
        Some(get_storage) => assert_eq!(
            get_storage,
            shipyard::error::GetStorage::MissingStorage {
                name: Some(type_name::<UniqueStorage<USIZE>>()),
                id: StorageId::of::<UniqueStorage<USIZE>>(),
            }
        ),
        _ => panic!(),
    }

    match world.borrow::<UniqueViewMut<USIZE>>().err() {
        Some(get_storage) => assert_eq!(
            get_storage,
            shipyard::error::GetStorage::MissingStorage {
                name: Some(type_name::<UniqueStorage<USIZE>>()),
                id: StorageId::of::<UniqueStorage<USIZE>>(),
            }
        ),
        _ => panic!(),
    }

    match world.remove_unique::<USIZE>().err() {
        Some(error::UniqueRemove::MissingUnique(name)) => assert_eq!(name, type_name::<USIZE>()),
        _ => panic!(),
    }
}

#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    struct NonSendStruct {
        value: usize,
        _phantom: core::marker::PhantomData<*const ()>,
    }
    unsafe impl Sync for NonSendStruct {}
    impl Component for NonSendStruct {
        type Tracking = track::Untracked;
    }
    impl Unique for NonSendStruct {}

    let world = World::default();
    world.add_unique_non_send(NonSendStruct {
        value: 0,
        _phantom: core::marker::PhantomData,
    });

    world.run(|mut x: borrow::NonSend<UniqueViewMut<NonSendStruct>>| {
        x.value += 1;
    });
    world.run(|x: borrow::NonSend<UniqueView<NonSendStruct>>| {
        assert_eq!(x.value, 1);
    });
}

#[cfg(feature = "thread_local")]
#[test]
fn non_sync() {
    struct NonSyncStruct {
        value: usize,
        _phantom: core::marker::PhantomData<*const ()>,
    }
    unsafe impl Send for NonSyncStruct {}
    impl Component for NonSyncStruct {
        type Tracking = track::Untracked;
    }
    impl Unique for NonSyncStruct {}

    let world = World::default();
    world.add_unique_non_sync(NonSyncStruct {
        value: 0,
        _phantom: core::marker::PhantomData,
    });

    world.run(|mut x: borrow::NonSync<UniqueViewMut<NonSyncStruct>>| {
        x.value += 1;
    });
    world.run(|x: borrow::NonSync<UniqueView<NonSyncStruct>>| {
        assert_eq!(x.value, 1);
    });
}

#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync() {
    struct NonSendSyncStruct {
        value: usize,
        _phantom: core::marker::PhantomData<*const ()>,
    }
    impl Component for NonSendSyncStruct {
        type Tracking = track::Untracked;
    }
    impl Unique for NonSendSyncStruct {}

    let world = World::default();
    world.add_unique_non_send_sync(NonSendSyncStruct {
        value: 0,
        _phantom: core::marker::PhantomData,
    });

    world.run(
        |mut x: borrow::NonSendSync<UniqueViewMut<NonSendSyncStruct>>| {
            x.value += 1;
        },
    );
    world.run(|x: borrow::NonSendSync<UniqueView<NonSendSyncStruct>>| {
        assert_eq!(x.value, 1);
    });
}

#[test]
#[cfg(feature = "thread_local")]
fn non_send_remove() {
    let world: &'static World = Box::leak(Box::new(World::new()));

    world.add_unique_non_send(USIZE(0));

    std::thread::spawn(move || {
        if let Some(shipyard::error::UniqueRemove::StorageBorrow(infos)) =
            world.remove_unique::<USIZE>().err()
        {
            assert_eq!(
                infos,
                (type_name::<USIZE>(), shipyard::error::Borrow::WrongThread)
            );
        } else {
            panic!()
        }
    })
    .join()
    .unwrap();
}

/// Returns the message of the panic caught in `result`.
#[cfg(feature = "thread_local")]
fn panic_message(result: std::thread::Result<()>) -> String {
    *result.unwrap_err().downcast::<String>().unwrap()
}

/// Returns the message `add_unique*` panics with when it can't borrow `T`'s existing storage.
#[cfg(feature = "thread_local")]
fn borrow_panic<T: Unique>(borrow: error::Borrow) -> String {
    format!(
        "{:?}",
        error::GetStorage::StorageBorrow {
            name: Some(type_name::<UniqueStorage<T>>()),
            id: StorageId::of::<UniqueStorage<T>>(),
            borrow,
        }
    )
}

/// Asserts that a newly added unique is flagged as inserted and not as modified.
#[test]
fn insert_tracking() {
    let world = World::new();
    world.add_unique(USIZE(0));

    let view = world.borrow::<UniqueViewMut<USIZE>>().unwrap();
    assert_eq!(*view, USIZE(0));
    assert!(view.is_inserted());
    assert!(!view.is_modified());
}

/// Asserts that `World::add_unique` replaces the value of a unique that already exists.
#[test]
fn replace() {
    let world = World::new();
    world.add_unique(USIZE(0));
    world.add_unique(USIZE(1));

    assert_eq!(*world.borrow::<UniqueView<USIZE>>().unwrap(), USIZE(1));
}

/// Asserts that `AllStorages::add_unique` replaces the value of a unique that already exists.
#[test]
fn replace_all_storages() {
    let world = World::new();
    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.add_unique(USIZE(0));
    all_storages.add_unique(USIZE(1));

    assert_eq!(
        *all_storages.borrow::<UniqueView<USIZE>>().unwrap(),
        USIZE(1)
    );
}

/// Asserts that replacing a unique whose tracking was cleared flags it as modified, but doesn't
/// flag it as inserted again.
#[test]
fn replace_tracking() {
    let world = World::new();
    world.add_unique(USIZE(0));
    world
        .borrow::<UniqueViewMut<USIZE>>()
        .unwrap()
        .clear_inserted_and_modified();

    world.add_unique(USIZE(1));

    let view = world.borrow::<UniqueViewMut<USIZE>>().unwrap();
    assert!(!view.is_inserted());
    assert!(view.is_modified());
}

/// Asserts that replacing a unique keeps its registered clone function, so cloning the world
/// copies the new value.
#[test]
fn replace_clone() {
    let mut world = World::new();
    world.add_unique(USIZE(0));
    world.register_clone::<UniqueStorage<USIZE>>();

    world.add_unique(USIZE(1));

    let clone = world.clone();
    assert_eq!(*clone.borrow::<UniqueView<USIZE>>().unwrap(), USIZE(1));
}

/// Asserts that replacing a unique first added with `add_unique_non_send` keeps it `NonSend`: the
/// new value is read through `NonSend<UniqueView>`, and another thread still can't replace it.
#[cfg(feature = "thread_local")]
#[test]
fn replace_non_send() {
    let world = World::new();
    world.add_unique_non_send(USIZE(0));
    world.add_unique(USIZE(1));

    let result = std::thread::scope(|scope| scope.spawn(|| world.add_unique(USIZE(2))).join());

    assert_eq!(
        panic_message(result),
        borrow_panic::<USIZE>(error::Borrow::WrongThread)
    );
    assert_eq!(
        **world
            .borrow::<borrow::NonSend<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );
}

/// Asserts that replacing a unique first added with `add_unique_non_sync` keeps it `NonSync`: the
/// new value is read through `NonSync<UniqueView>`. Another thread can still replace it since a
/// `!Sync` storage can be borrowed exclusively from any thread.
#[cfg(feature = "thread_local")]
#[test]
fn replace_non_sync() {
    let world = World::new();
    world.add_unique_non_sync(USIZE(0));
    world.add_unique(USIZE(1));

    assert_eq!(
        **world
            .borrow::<borrow::NonSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );

    std::thread::scope(|scope| {
        scope.spawn(|| world.add_unique(USIZE(2)));
    });

    assert_eq!(
        **world
            .borrow::<borrow::NonSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(2)
    );
}

/// Asserts that replacing a unique first added with `add_unique_non_send_sync` keeps it
/// `NonSendSync`: the new value is read through `NonSendSync<UniqueView>`, and another thread
/// still can't replace it.
#[cfg(feature = "thread_local")]
#[test]
fn replace_non_send_sync() {
    let world = World::new();
    world.add_unique_non_send_sync(USIZE(0));
    world.add_unique(USIZE(1));

    let result = std::thread::scope(|scope| scope.spawn(|| world.add_unique(USIZE(2))).join());

    assert_eq!(
        panic_message(result),
        borrow_panic::<USIZE>(error::Borrow::WrongThread)
    );
    assert_eq!(
        **world
            .borrow::<borrow::NonSendSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );
}

/// Unique that is `Sync` but not `Send`.
#[cfg(feature = "thread_local")]
struct NotSend(usize, core::marker::PhantomData<*const ()>);
#[cfg(feature = "thread_local")]
unsafe impl Sync for NotSend {}
#[cfg(feature = "thread_local")]
impl Unique for NotSend {}

/// Unique that is `Send` but not `Sync`.
#[cfg(feature = "thread_local")]
struct NotSync(usize, core::marker::PhantomData<*const ()>);
#[cfg(feature = "thread_local")]
unsafe impl Send for NotSync {}
#[cfg(feature = "thread_local")]
impl Unique for NotSync {}

/// Unique that is neither `Send` nor `Sync`.
#[cfg(feature = "thread_local")]
struct NotSendSync(usize, core::marker::PhantomData<*const ()>);
#[cfg(feature = "thread_local")]
impl Unique for NotSendSync {}

/// Returns the message `add_unique_non_send*` panics with when called from another thread.
#[cfg(feature = "thread_local")]
fn wrong_thread_panic<T>() -> String {
    format!(
        "Cannot add {} unique from other thread than the World's because it's !Send.",
        type_name::<T>()
    )
}

/// Asserts that `add_unique_non_send` replaces the value of a `!Send` unique it added. The
/// replacement is flagged as modified but not as inserted again once the tracking was cleared.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_replace() {
    let world = World::new();
    world.add_unique_non_send(NotSend(0, core::marker::PhantomData));
    world
        .borrow::<borrow::NonSend<UniqueViewMut<NotSend>>>()
        .unwrap()
        .clear_inserted_and_modified();

    world.add_unique_non_send(NotSend(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSend<UniqueViewMut<NotSend>>>()
        .unwrap();
    assert_eq!(view.0, 1);
    assert!(!view.is_inserted());
    assert!(view.is_modified());
}

/// Asserts that `add_unique_non_sync` replaces the value of a `!Sync` unique it added. The
/// replacement is flagged as modified but not as inserted again once the tracking was cleared.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_replace() {
    let world = World::new();
    world.add_unique_non_sync(NotSync(0, core::marker::PhantomData));
    world
        .borrow::<borrow::NonSync<UniqueViewMut<NotSync>>>()
        .unwrap()
        .clear_inserted_and_modified();

    world.add_unique_non_sync(NotSync(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSync<UniqueViewMut<NotSync>>>()
        .unwrap();
    assert_eq!(view.0, 1);
    assert!(!view.is_inserted());
    assert!(view.is_modified());
}

/// Asserts that `add_unique_non_send_sync` replaces the value of a `!Send + !Sync` unique it
/// added. The replacement is flagged as modified but not as inserted again once the tracking was
/// cleared.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_replace() {
    let world = World::new();
    world.add_unique_non_send_sync(NotSendSync(0, core::marker::PhantomData));
    world
        .borrow::<borrow::NonSendSync<UniqueViewMut<NotSendSync>>>()
        .unwrap()
        .clear_inserted_and_modified();

    world.add_unique_non_send_sync(NotSendSync(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSendSync<UniqueViewMut<NotSendSync>>>()
        .unwrap();
    assert_eq!(view.0, 1);
    assert!(!view.is_inserted());
    assert!(view.is_modified());
}

/// Asserts that replacing a `NonSend` unique keeps its registered clone function, so cloning the
/// world copies the new value.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_replace_clone() {
    let mut world = World::new();
    world.add_unique_non_send(USIZE(0));
    world.register_clone::<borrow::NonSend<UniqueStorage<USIZE>>>();

    world.add_unique_non_send(USIZE(1));

    let clone = world.clone();
    assert_eq!(
        **clone
            .borrow::<borrow::NonSend<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );
}

/// Asserts that replacing a `NonSync` unique keeps its registered clone function, so cloning the
/// world copies the new value.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_replace_clone() {
    let mut world = World::new();
    world.add_unique_non_sync(USIZE(0));
    world.register_clone::<borrow::NonSync<UniqueStorage<USIZE>>>();

    world.add_unique_non_sync(USIZE(1));

    let clone = world.clone();
    assert_eq!(
        **clone
            .borrow::<borrow::NonSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );
}

/// Asserts that replacing a `NonSendSync` unique keeps its registered clone function, so cloning
/// the world copies the new value.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_replace_clone() {
    let mut world = World::new();
    world.add_unique_non_send_sync(USIZE(0));
    world.register_clone::<borrow::NonSendSync<UniqueStorage<USIZE>>>();

    world.add_unique_non_send_sync(USIZE(1));

    let clone = world.clone();
    assert_eq!(
        **clone
            .borrow::<borrow::NonSendSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );
}

/// Asserts that every `add_unique*` method replaces the value of a unique first added with
/// `add_unique`, and that it's still borrowed without a wrapper.
#[cfg(feature = "thread_local")]
#[test]
fn unique_replace_any() {
    let world = World::new();
    let value = || world.borrow::<UniqueView<USIZE>>().unwrap().0;
    world.add_unique(USIZE(0));

    world.add_unique(USIZE(1));
    assert_eq!(value(), 1);
    world.add_unique_non_send(USIZE(2));
    assert_eq!(value(), 2);
    world.add_unique_non_sync(USIZE(3));
    assert_eq!(value(), 3);
    world.add_unique_non_send_sync(USIZE(4));
    assert_eq!(value(), 4);
}

/// Asserts that every `add_unique*` method replaces the value of a unique first added with
/// `add_unique_non_send`, and that it's still borrowed with `NonSend`.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_replace_any() {
    let world = World::new();
    let value = || {
        world
            .borrow::<borrow::NonSend<UniqueView<USIZE>>>()
            .unwrap()
            .0
    };
    world.add_unique_non_send(USIZE(0));

    world.add_unique(USIZE(1));
    assert_eq!(value(), 1);
    world.add_unique_non_send(USIZE(2));
    assert_eq!(value(), 2);
    world.add_unique_non_sync(USIZE(3));
    assert_eq!(value(), 3);
    world.add_unique_non_send_sync(USIZE(4));
    assert_eq!(value(), 4);
}

/// Asserts that every `add_unique*` method replaces the value of a unique first added with
/// `add_unique_non_sync`, and that it's still borrowed with `NonSync`.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_replace_any() {
    let world = World::new();
    let value = || {
        world
            .borrow::<borrow::NonSync<UniqueView<USIZE>>>()
            .unwrap()
            .0
    };
    world.add_unique_non_sync(USIZE(0));

    world.add_unique(USIZE(1));
    assert_eq!(value(), 1);
    world.add_unique_non_send(USIZE(2));
    assert_eq!(value(), 2);
    world.add_unique_non_sync(USIZE(3));
    assert_eq!(value(), 3);
    world.add_unique_non_send_sync(USIZE(4));
    assert_eq!(value(), 4);
}

/// Asserts that every `add_unique*` method replaces the value of a unique first added with
/// `add_unique_non_send_sync`, and that it's still borrowed with `NonSendSync`.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_replace_any() {
    let world = World::new();
    let value = || {
        world
            .borrow::<borrow::NonSendSync<UniqueView<USIZE>>>()
            .unwrap()
            .0
    };
    world.add_unique_non_send_sync(USIZE(0));

    world.add_unique(USIZE(1));
    assert_eq!(value(), 1);
    world.add_unique_non_send(USIZE(2));
    assert_eq!(value(), 2);
    world.add_unique_non_sync(USIZE(3));
    assert_eq!(value(), 3);
    world.add_unique_non_send_sync(USIZE(4));
    assert_eq!(value(), 4);
}

/// Asserts that `add_unique_non_send_sync` replaces the value of a `!Send` unique first added
/// with `add_unique_non_send`, and that it's still borrowed with `NonSend`.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_then_non_send_sync() {
    let world = World::new();
    world.add_unique_non_send(NotSend(0, core::marker::PhantomData));

    world.add_unique_non_send_sync(NotSend(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSend<UniqueView<NotSend>>>()
        .unwrap();
    assert_eq!(view.0, 1);
}

/// Asserts that `add_unique_non_send` replaces the value of a `!Send` unique first added with
/// `add_unique_non_send_sync`, and that it's still borrowed with `NonSendSync`.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_then_non_send() {
    let world = World::new();
    world.add_unique_non_send_sync(NotSend(0, core::marker::PhantomData));

    world.add_unique_non_send(NotSend(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSendSync<UniqueView<NotSend>>>()
        .unwrap();
    assert_eq!(view.0, 1);
}

/// Asserts that `add_unique_non_send_sync` replaces the value of a `!Sync` unique first added
/// with `add_unique_non_sync`, and that it's still borrowed with `NonSync`.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_then_non_send_sync() {
    let world = World::new();
    world.add_unique_non_sync(NotSync(0, core::marker::PhantomData));

    world.add_unique_non_send_sync(NotSync(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSync<UniqueView<NotSync>>>()
        .unwrap();
    assert_eq!(view.0, 1);
}

/// Asserts that `add_unique_non_sync` replaces the value of a `!Sync` unique first added with
/// `add_unique_non_send_sync`, and that it's still borrowed with `NonSendSync`.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_then_non_sync() {
    let world = World::new();
    world.add_unique_non_send_sync(NotSync(0, core::marker::PhantomData));

    world.add_unique_non_sync(NotSync(1, core::marker::PhantomData));

    let view = world
        .borrow::<borrow::NonSendSync<UniqueView<NotSync>>>()
        .unwrap();
    assert_eq!(view.0, 1);
}

/// Asserts that `add_unique_non_send` panics when called from another thread than the world's,
/// without adding the unique.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_wrong_thread_vacant() {
    let world = World::new();

    let result =
        std::thread::scope(|scope| scope.spawn(|| world.add_unique_non_send(USIZE(0))).join());

    assert_eq!(panic_message(result), wrong_thread_panic::<USIZE>());
    assert!(
        world
            .borrow::<borrow::NonSend<UniqueView<USIZE>>>()
            .is_err()
    );
}

/// Asserts that `add_unique_non_send` panics when called from another thread than the world's,
/// leaving the existing value in place.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_wrong_thread_existing() {
    let world = World::new();
    world.add_unique_non_send(USIZE(0));

    let result =
        std::thread::scope(|scope| scope.spawn(|| world.add_unique_non_send(USIZE(1))).join());

    assert_eq!(panic_message(result), wrong_thread_panic::<USIZE>());
    assert_eq!(
        **world
            .borrow::<borrow::NonSend<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(0)
    );
}

/// Asserts that `add_unique_non_send_sync` panics when called from another thread than the
/// world's, without adding the unique.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_wrong_thread_vacant() {
    let world = World::new();

    let result = std::thread::scope(|scope| {
        scope
            .spawn(|| world.add_unique_non_send_sync(USIZE(0)))
            .join()
    });

    assert_eq!(panic_message(result), wrong_thread_panic::<USIZE>());
    assert!(
        world
            .borrow::<borrow::NonSendSync<UniqueView<USIZE>>>()
            .is_err()
    );
}

/// Asserts that `add_unique_non_send_sync` panics when called from another thread than the
/// world's, leaving the existing value in place.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync_wrong_thread_existing() {
    let world = World::new();
    world.add_unique_non_send_sync(USIZE(0));

    let result = std::thread::scope(|scope| {
        scope
            .spawn(|| world.add_unique_non_send_sync(USIZE(1)))
            .join()
    });

    assert_eq!(panic_message(result), wrong_thread_panic::<USIZE>());
    assert_eq!(
        **world
            .borrow::<borrow::NonSendSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(0)
    );
}

/// Asserts that `add_unique_non_sync` can't replace a `NonSend` unique from another thread than
/// the world's: it panics on the storage's thread restriction and the value stays in place.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_wrong_thread_non_send() {
    let world = World::new();
    world.add_unique_non_send(USIZE(0));

    let result =
        std::thread::scope(|scope| scope.spawn(|| world.add_unique_non_sync(USIZE(1))).join());

    assert_eq!(
        panic_message(result),
        borrow_panic::<USIZE>(error::Borrow::WrongThread)
    );
    assert_eq!(
        **world
            .borrow::<borrow::NonSend<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(0)
    );
}

/// Asserts that `add_unique_non_sync` can't replace a `NonSendSync` unique from another thread
/// than the world's: it panics on the storage's thread restriction and the value stays in place.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_wrong_thread_non_send_sync() {
    let world = World::new();
    world.add_unique_non_send_sync(USIZE(0));

    let result =
        std::thread::scope(|scope| scope.spawn(|| world.add_unique_non_sync(USIZE(1))).join());

    assert_eq!(
        panic_message(result),
        borrow_panic::<USIZE>(error::Borrow::WrongThread)
    );
    assert_eq!(
        **world
            .borrow::<borrow::NonSendSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(0)
    );
}

/// Asserts that `add_unique_non_sync` isn't restricted to the world's thread: another thread can
/// add the unique, and yet another one can replace its value.
#[cfg(feature = "thread_local")]
#[test]
fn non_sync_other_threads() {
    let world = World::new();

    std::thread::scope(|scope| {
        scope.spawn(|| world.add_unique_non_sync(USIZE(0)));
    });
    std::thread::scope(|scope| {
        scope.spawn(|| world.add_unique_non_sync(USIZE(1)));
    });

    assert_eq!(
        **world
            .borrow::<borrow::NonSync<UniqueView<USIZE>>>()
            .unwrap(),
        USIZE(1)
    );
}

#[test]
fn unique_or_default() {
    let world = World::new();

    world.run(|u: UniqueOrDefaultView<USIZE>| assert_eq!(*u, USIZE(0)));
}

#[test]
fn unique_or_default_mut() {
    let world = World::new();

    world.run(|mut u: UniqueOrDefaultViewMut<USIZE>| {
        u.0 += 1;
        assert_eq!(*u, USIZE(1))
    });
}

#[test]
fn unique_or_init() {
    let world = World::new();

    world.run(|u: UniqueOrInitView<USIZE>| {
        assert_eq!(**u.get_or_init(|| USIZE(10)).unwrap(), USIZE(10))
    });

    world.run(|u: UniqueOrInitView<USIZE>| assert_eq!(**u.get().unwrap(), USIZE(10)));
}

#[test]
fn unique_or_init_mut() {
    let world = World::new();

    world.run(|u: UniqueOrInitViewMut<USIZE>| {
        assert_eq!(**u.get_or_init(|| USIZE(10)).unwrap(), USIZE(10))
    });

    world.run(|mut u: UniqueOrInitViewMut<USIZE>| u.get_mut().unwrap().0 += 1);

    world.run(|u: UniqueOrInitViewMut<USIZE>| assert_eq!(**u.get().unwrap(), USIZE(11)));
}
