use crate::error;
use crate::world::World;
use crate::AllStorages;

// Register multiple storages at once
pub trait Register {
    fn try_register(world: &World) -> Result<(), error::Register>;
}

pub trait RegisterNonSend {
    fn try_register(world: &World) -> Result<(), error::Register>;
}

pub trait RegisterNonSync {
    fn try_register(world: &World) -> Result<(), error::Register>;
}

pub trait RegisterNonSendNonSync {
    fn try_register(world: &World) -> Result<(), error::Register>;
}

macro_rules! impl_register {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static + Send + Sync),+> Register for ($($type,)+) {
            fn try_register(world: &World) -> Result<(), error::Register> {
                world.try_run::<AllStorages, _, _>(|mut all_storages| {
                    $({
                        all_storages.register::<$type>();
                    })+
                }).map_err(|err| match err {
                    error::GetStorage::AllStoragesBorrow(borrow) => borrow.into(),
                    _ => unreachable!()
                })
            }
        }

        impl<$($type: 'static + Sync),+> RegisterNonSend for ($($type,)+) {
            fn try_register(world: &World) -> Result<(), error::Register> {
                if world.thread_id != std::thread::current().id() {
                    Err(error::Register::WrongThread)
                } else {
                    world.try_run::<AllStorages, _, _>(|mut all_storages| {
                        $({
                            all_storages.register_non_send::<$type>();
                        })+
                    }).map_err(|err| match err {
                        error::GetStorage::AllStoragesBorrow(borrow) => borrow.into(),
                        _ => unreachable!()
                    })
                }
            }
        }

        impl<$($type: 'static + Send),+> RegisterNonSync for ($($type,)+) {
            fn try_register(world: &World) -> Result<(), error::Register> {
                world.try_run::<AllStorages, _, _>(|mut all_storages| {
                    $({
                        all_storages.register_non_sync::<$type>();
                    })+
                }).map_err(|err| match err {
                    error::GetStorage::AllStoragesBorrow(borrow) => borrow.into(),
                    _ => unreachable!()
                })
            }
        }

        impl<$($type: 'static),+> RegisterNonSendNonSync for ($($type,)+) {
            fn try_register(world: &World) -> Result<(), error::Register> {
                if world.thread_id != std::thread::current().id() {
                    Err(error::Register::WrongThread)
                } else {
                    world.try_run::<AllStorages, _, _>(|mut all_storages| {
                        $({
                            all_storages.register_non_send_non_sync::<$type>();
                        })+
                    }).map_err(|err| match err {
                        error::GetStorage::AllStoragesBorrow(borrow) => borrow.into(),
                        _ => unreachable!()
                    })
                }
            }
        }
    }
}

macro_rules! register {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_register![$(($type, $index))*];
        register![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_register![$(($type, $index))*];
    }
}

register![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
