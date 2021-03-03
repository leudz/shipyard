use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[cfg(feature = "thread_local")]
struct NotSend(*const ());

#[cfg(feature = "thread_local")]
unsafe impl Sync for NotSend {}

#[cfg(feature = "thread_local")]
struct NotSync(*const ());

#[cfg(feature = "thread_local")]
unsafe impl Send for NotSync {}

#[cfg(feature = "thread_local")]
struct NotSendSync(*const ());

#[test]
fn simple_borrow() {
    let world = World::new();

    let u32s = world.borrow::<View<u32>>().unwrap();
    assert_eq!(u32s.len(), 0);
}

#[test]
fn option_borrow() {
    let world = World::new();

    let u32s = world.borrow::<Option<View<u32>>>().unwrap();

    let u32s = u32s.unwrap();
    assert_eq!(u32s.len(), 0);
    drop(u32s);
    let _i32s = world.borrow::<ViewMut<i32>>().unwrap();
    let other_i32s = world.borrow::<Option<View<i32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
fn all_storages_simple_borrow() {
    let world = World::new();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let u32s = all_storages.borrow::<View<u32>>().unwrap();
    assert_eq!(u32s.len(), 0);
}

#[test]
fn invalid_borrow() {
    let world = World::new();

    let _u32s = world.borrow::<ViewMut<u32>>().unwrap();
    assert_eq!(
        world.borrow::<ViewMut<u32>>().err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(type_name::<SparseSet<u32>>()),
            id: StorageId::of::<SparseSet<u32>>(),
            borrow: error::Borrow::Unique
        })
    );
}

#[test]
fn all_storages_invalid_borrow() {
    let world = World::new();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let _u32s = all_storages.borrow::<ViewMut<u32>>().unwrap();
    assert_eq!(
        all_storages.borrow::<ViewMut<u32>>().err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(type_name::<SparseSet<u32>>()),
            id: StorageId::of::<SparseSet<u32>>(),
            borrow: error::Borrow::Unique
        })
    );
}

#[test]
fn double_borrow() {
    let world = World::new();

    let u32s = world.borrow::<ViewMut<u32>>().unwrap();
    drop(u32s);
    world.borrow::<ViewMut<u32>>().unwrap();
}

#[test]
fn all_storages_double_borrow() {
    let world = World::new();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let u32s = all_storages.borrow::<ViewMut<u32>>().unwrap();
    drop(u32s);
    all_storages.borrow::<ViewMut<u32>>().unwrap();
}

#[test]
fn all_storages_option_borrow() {
    let world = World::new();
    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

    let u32s = all_storages.borrow::<Option<View<u32>>>().unwrap();

    let u32s = u32s.unwrap();
    assert_eq!(u32s.len(), 0);
    drop(u32s);
    let _i32s = all_storages.borrow::<ViewMut<i32>>().unwrap();
    let other_i32s = all_storages.borrow::<Option<View<i32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
#[cfg(feature = "thread_local")]
fn non_send_storage_in_other_thread() {
    let world = World::new();
    rayon::join(
        || {
            assert_eq!(
                world.borrow::<NonSend<ViewMut<NotSend>>>().err(),
                Some(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<SparseSet<NotSend>>()),
                    id: StorageId::of::<SparseSet<NotSend>>(),
                    borrow: error::Borrow::WrongThread
                })
            )
        },
        || {},
    );
}

#[test]
#[cfg(feature = "thread_local")]
fn non_send_sync_storage_in_other_thread() {
    let world = World::new();
    rayon::join(
        || {
            assert_eq!(
                world.borrow::<NonSendSync<View<NotSendSync>>>().err(),
                Some(error::GetStorage::StorageBorrow {
                    name: Some(type_name::<SparseSet<NotSendSync>>()),
                    id: StorageId::of::<SparseSet<NotSendSync>>(),
                    borrow: error::Borrow::WrongThread
                })
            )
        },
        || {},
    );
}

#[test]
fn add_unique_while_borrowing() {
    let world = World::new();
    world.add_unique(0u32).unwrap();
    let _s = world.borrow::<UniqueView<'_, u32>>().unwrap();
    world.add_unique(0usize).unwrap();
}

#[test]
fn sparse_set_and_unique() {
    let world = World::new();
    world.add_unique(0u32).unwrap();
    world
        .borrow::<(UniqueViewMut<u32>, ViewMut<u32>)>()
        .unwrap();
}

#[test]
#[cfg(feature = "thread_local")]
fn exhaustive_list() {
    let world = World::new();

    let _ = world.borrow::<(
        NonSend<View<NotSend>>,
        NonSync<View<NotSync>>,
        NonSendSync<View<NotSendSync>>,
        NonSend<ViewMut<NotSend>>,
        NonSync<ViewMut<NotSync>>,
        NonSendSync<ViewMut<NotSendSync>>,
    )>();

    world
        .run(|all_storages: AllStoragesViewMut| {
            let _ = all_storages.borrow::<(
                NonSend<View<NotSend>>,
                NonSync<View<NotSync>>,
                NonSendSync<View<NotSendSync>>,
                NonSend<ViewMut<NotSend>>,
                NonSync<ViewMut<NotSync>>,
                NonSendSync<ViewMut<NotSendSync>>,
            )>();
        })
        .unwrap();
}

#[test]
#[cfg(feature = "thread_local")]
fn unique_exhaustive_list() {
    let world = World::new();

    world.add_unique_non_send(NotSend(&())).unwrap();
    world.add_unique_non_sync(NotSync(&())).unwrap();
    world.add_unique_non_send_sync(NotSendSync(&())).unwrap();

    let _ = world.borrow::<(
        NonSend<UniqueView<NotSend>>,
        NonSync<UniqueView<NotSync>>,
        NonSendSync<UniqueView<NotSendSync>>,
        NonSend<UniqueViewMut<NotSend>>,
        NonSync<UniqueViewMut<NotSync>>,
        NonSendSync<UniqueViewMut<NotSendSync>>,
    )>();

    world
        .run(|all_storages: AllStoragesViewMut| {
            let _ = all_storages.borrow::<(
                NonSend<UniqueView<NotSend>>,
                NonSync<UniqueView<NotSync>>,
                NonSendSync<UniqueView<NotSendSync>>,
                NonSend<UniqueViewMut<NotSend>>,
                NonSync<UniqueViewMut<NotSync>>,
                NonSendSync<UniqueViewMut<NotSendSync>>,
            )>();
        })
        .unwrap();
}
