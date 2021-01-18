use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn unique_storage() {
    let world = World::default();
    world.add_unique(0usize).unwrap();

    world
        .run(|mut x: UniqueViewMut<usize>| {
            *x += 1;
        })
        .unwrap();
    world
        .run(|x: UniqueView<usize>| {
            assert_eq!(*x, 1);
        })
        .unwrap();

    world.remove_unique::<usize>().unwrap();

    if let Some(shipyard::error::Run::GetStorage(get_error)) = world
        .run(|mut x: UniqueViewMut<usize>| {
            *x += 1;
        })
        .err()
    {
        assert_eq!(
            get_error,
            shipyard::error::GetStorage::MissingStorage(
                core::any::type_name::<Unique<usize>>().into()
            )
        );
    } else {
        panic!()
    }

    world.add_unique(0usize).unwrap();
}

#[test]
fn not_unique_storage() {
    let world = World::new();

    match world.run(|_: UniqueView<usize>| {}).err() {
        Some(error::Run::GetStorage(get_storage)) => assert_eq!(
            get_storage,
            error::GetStorage::MissingStorage(type_name::<Unique<usize>>().into())
        ),
        _ => panic!(),
    }

    match world.run(|_: UniqueViewMut<usize>| {}).err() {
        Some(error::Run::GetStorage(get_storage)) => assert_eq!(
            get_storage,
            error::GetStorage::MissingStorage(type_name::<Unique<usize>>().into())
        ),
        _ => panic!(),
    }

    match world.remove_unique::<usize>().err() {
        Some(error::UniqueRemove::MissingUnique(name)) => assert_eq!(name, type_name::<usize>()),
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
    let world: &'static World = Box::leak(Box::new(World::new()));

    world.add_unique_non_send(0usize).unwrap();

    std::thread::spawn(move || {
        if let Some(shipyard::error::UniqueRemove::StorageBorrow(infos)) =
            world.remove_unique::<usize>().err()
        {
            assert_eq!(
                infos,
                (type_name::<usize>(), shipyard::error::Borrow::WrongThread)
            );
        } else {
            panic!()
        }
    })
    .join()
    .unwrap();
}
