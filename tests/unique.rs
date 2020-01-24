use shipyard::prelude::*;

#[test]
fn unique_storage() {
    let world = World::default();
    world.add_unique(0usize);

    world.run::<Unique<&mut usize>, _, _>(|mut x| {
        *x += 1;
    });
    world.run::<Unique<&usize>, _, _>(|x| {
        assert_eq!(*x, 1);
    });
}

#[test]
fn not_unique_storage() {
    match std::panic::catch_unwind(|| {
        let world = World::new();

        world.run::<Unique<&usize>, _, _>(|x| {
            assert_eq!(*x, 1);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            format!("{}", err.downcast::<String>().unwrap()),
            "called `Result::unwrap()` on an `Err` value: usize's storage isn't unique.\n\
            You might have forgotten to declare it, replace world.register::<usize>() by world.register_unique(/* your_storage */).\n\
            If it isn't supposed to be a unique storage, replace Unique<&usize> by &usize."
        ),
    }

    match std::panic::catch_unwind(|| {
        let world = World::new();

        world.run::<Unique<&mut usize>, _, _>(|x| {
            assert_eq!(*x, 1);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            format!("{}", err.downcast::<String>().unwrap()),
            "called `Result::unwrap()` on an `Err` value: usize's storage isn't unique.\n\
            You might have forgotten to declare it, replace world.register::<usize>() by world.register_unique(/* your_storage */).\n\
            If it isn't supposed to be a unique storage, replace Unique<&mut usize> by &mut usize."
        ),
    }
}

#[cfg(feature = "non_send")]
#[test]
fn non_send() {
    struct NonSendStruct {
        value: usize,
        _phantom: core::marker::PhantomData<*const ()>,
    }
    unsafe impl Sync for NonSendStruct {}

    let world = World::default();
    world.add_unique_non_send(NonSendStruct {
        value: 0,
        _phantom: core::marker::PhantomData,
    });

    world.run::<Unique<NonSend<&mut NonSendStruct>>, _, _>(|mut x| {
        x.value += 1;
    });
    world.run::<Unique<NonSend<&NonSendStruct>>, _, _>(|x| {
        assert_eq!(x.value, 1);
    });
}

#[cfg(feature = "non_sync")]
#[test]
fn non_sync() {
    struct NonSyncStruct {
        value: usize,
        _phantom: core::marker::PhantomData<*const ()>,
    }
    unsafe impl Send for NonSyncStruct {}

    let world = World::default();
    world.add_unique_non_sync(NonSyncStruct {
        value: 0,
        _phantom: core::marker::PhantomData,
    });

    world.run::<Unique<NonSync<&mut NonSyncStruct>>, _, _>(|mut x| {
        x.value += 1;
    });
    world.run::<Unique<NonSync<&NonSyncStruct>>, _, _>(|x| {
        assert_eq!(x.value, 1);
    });
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
#[test]
fn non_send_sync() {
    struct NonSendSyncStruct {
        value: usize,
        _phantom: core::marker::PhantomData<*const ()>,
    }

    let world = World::default();
    world.add_unique_non_send_sync(NonSendSyncStruct {
        value: 0,
        _phantom: core::marker::PhantomData,
    });

    world.run::<Unique<NonSendSync<&mut NonSendSyncStruct>>, _, _>(|mut x| {
        x.value += 1;
    });
    world.run::<Unique<NonSendSync<&NonSendSyncStruct>>, _, _>(|x| {
        assert_eq!(x.value, 1);
    });
}
