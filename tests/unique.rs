use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[derive(Default, Debug, PartialEq)]
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

    world.run(|mut x: NonSend<UniqueViewMut<NonSendStruct>>| {
        x.value += 1;
    });
    world.run(|x: NonSend<UniqueView<NonSendStruct>>| {
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

    world.run(|mut x: NonSync<UniqueViewMut<NonSyncStruct>>| {
        x.value += 1;
    });
    world.run(|x: NonSync<UniqueView<NonSyncStruct>>| {
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

    world.run(|mut x: NonSendSync<UniqueViewMut<NonSendSyncStruct>>| {
        x.value += 1;
    });
    world.run(|x: NonSendSync<UniqueView<NonSendSyncStruct>>| {
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
