use core::any::type_name;
use shipyard::error;
use shipyard::*;

struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[test]
fn unique_storage() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.add_unique(USIZE(0)).unwrap();

    world
        .run(|mut x: UniqueViewMut<USIZE>| {
            x.0 += 1;
        })
        .unwrap();
    world
        .run(|x: UniqueView<USIZE>| {
            assert_eq!(x.0, 1);
        })
        .unwrap();

    world.remove_unique::<USIZE>().unwrap();

    if let Some(shipyard::error::Run::GetStorage(get_error)) = world
        .run(|mut x: UniqueViewMut<USIZE>| {
            x.0 += 1;
        })
        .err()
    {
        assert_eq!(
            get_error,
            shipyard::error::GetStorage::MissingStorage {
                name: Some(type_name::<Unique<USIZE>>().into()),
                id: StorageId::of::<Unique<USIZE>>(),
            }
        );
    } else {
        panic!()
    }

    world.add_unique(USIZE(0)).unwrap();
}

#[test]
fn not_unique_storage() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    match world.run(|_: UniqueView<USIZE>| {}).err() {
        Some(error::Run::GetStorage(get_storage)) => assert_eq!(
            get_storage,
            shipyard::error::GetStorage::MissingStorage {
                name: Some(type_name::<Unique<USIZE>>().into()),
                id: StorageId::of::<Unique<USIZE>>(),
            }
        ),
        _ => panic!(),
    }

    match world.run(|_: UniqueViewMut<USIZE>| {}).err() {
        Some(error::Run::GetStorage(get_storage)) => assert_eq!(
            get_storage,
            shipyard::error::GetStorage::MissingStorage {
                name: Some(type_name::<Unique<USIZE>>().into()),
                id: StorageId::of::<Unique<USIZE>>(),
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

    let world = World::default();
    world
        .add_unique_non_send(NonSendStruct {
            value: 0,
            _phantom: core::marker::PhantomData,
        })
        .unwrap();

    world
        .run(|mut x: NonSend<UniqueViewMut<NonSendStruct>>| {
            x.value += 1;
        })
        .unwrap();
    world
        .run(|x: NonSend<UniqueView<NonSendStruct>>| {
            assert_eq!(x.value, 1);
        })
        .unwrap();
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

    let world = World::default();
    world
        .add_unique_non_sync(NonSyncStruct {
            value: 0,
            _phantom: core::marker::PhantomData,
        })
        .unwrap();

    world
        .run(|mut x: NonSync<UniqueViewMut<NonSyncStruct>>| {
            x.value += 1;
        })
        .unwrap();
    world
        .run(|x: NonSync<UniqueView<NonSyncStruct>>| {
            assert_eq!(x.value, 1);
        })
        .unwrap();
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

    let world = World::default();
    world
        .add_unique_non_send_sync(NonSendSyncStruct {
            value: 0,
            _phantom: core::marker::PhantomData,
        })
        .unwrap();

    world
        .run(|mut x: NonSendSync<UniqueViewMut<NonSendSyncStruct>>| {
            x.value += 1;
        })
        .unwrap();
    world
        .run(|x: NonSendSync<UniqueView<NonSendSyncStruct>>| {
            assert_eq!(x.value, 1);
        })
        .unwrap();
}

#[test]
#[cfg(feature = "thread_local")]
fn non_send_remove() {
    let world: &'static World = Box::leak(Box::new(World::new_with_custom_lock::<
        parking_lot::RawRwLock,
    >()));

    world.add_unique_non_send(USIZE(0)).unwrap();

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
