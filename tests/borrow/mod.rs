#[cfg(feature = "non_send")]
use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[cfg(feature = "non_send")]
struct NotSend(*const ());

#[cfg(feature = "non_send")]
unsafe impl Sync for NotSend {}

#[cfg(feature = "non_sync")]
struct NotSync(*const ());

#[cfg(feature = "non_sync")]
unsafe impl Send for NotSync {}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
struct NotSendSync(*const ());

#[test]
fn simple_borrow() {
    let world = World::new();

    let u32s = world.try_borrow::<View<u32>>().unwrap();
    assert_eq!(u32s.len(), 0);
}

#[test]
fn option_borrow() {
    let world = World::new();

    let u32s = world.try_borrow::<Option<View<u32>>>().unwrap();

    let u32s = u32s.unwrap();
    assert_eq!(u32s.len(), 0);
    drop(u32s);
    let _i32s = world.try_borrow::<ViewMut<i32>>().unwrap();
    let other_i32s = world.try_borrow::<Option<View<i32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
fn all_storages_simple_borrow() {
    let world = World::new();

    let all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    let u32s = all_storages.try_borrow::<View<u32>>().unwrap();
    assert_eq!(u32s.len(), 0);
}

#[test]
fn invalid_borrow() {
    let world = World::new();

    let _u32s = world.try_borrow::<ViewMut<u32>>().unwrap();
    assert_eq!(
        world.try_borrow::<ViewMut<u32>>().err(),
        Some(error::GetStorage::StorageBorrow((
            core::any::type_name::<u32>(),
            error::Borrow::Unique
        )))
    );
}

#[test]
fn all_storages_invalid_borrow() {
    let world = World::new();

    let all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    let _u32s = all_storages.try_borrow::<ViewMut<u32>>().unwrap();
    assert_eq!(
        all_storages.try_borrow::<ViewMut<u32>>().err(),
        Some(error::GetStorage::StorageBorrow((
            core::any::type_name::<u32>(),
            error::Borrow::Unique
        )))
    );
}

#[test]
fn double_borrow() {
    let world = World::new();

    let u32s = world.try_borrow::<ViewMut<u32>>().unwrap();
    drop(u32s);
    world.try_borrow::<ViewMut<u32>>().unwrap();
}

#[test]
fn all_storages_double_borrow() {
    let world = World::new();

    let all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    let u32s = all_storages.try_borrow::<ViewMut<u32>>().unwrap();
    drop(u32s);
    all_storages.try_borrow::<ViewMut<u32>>().unwrap();
}

#[test]
fn all_storages_option_borrow() {
    let world = World::new();
    let all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();

    let u32s = all_storages.try_borrow::<Option<View<u32>>>().unwrap();

    let u32s = u32s.unwrap();
    assert_eq!(u32s.len(), 0);
    drop(u32s);
    let _i32s = all_storages.try_borrow::<ViewMut<i32>>().unwrap();
    let other_i32s = all_storages.try_borrow::<Option<View<i32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
#[cfg(feature = "non_send")]
fn non_send_storage_in_other_thread() {
    let world = World::new();
    rayon::join(
        || {
            assert_eq!(
                world.try_borrow::<NonSend<ViewMut<NotSend>>>().err(),
                Some(error::GetStorage::StorageBorrow((
                    type_name::<NotSend>(),
                    error::Borrow::WrongThread
                )))
            )
        },
        || {},
    );
}

#[test]
#[cfg(all(feature = "non_send", feature = "non_sync"))]
fn non_send_sync_storage_in_other_thread() {
    let world = World::new();
    rayon::join(
        || {
            assert_eq!(
                world.try_borrow::<NonSendSync<View<NotSendSync>>>().err(),
                Some(error::GetStorage::StorageBorrow((
                    type_name::<NotSendSync>(),
                    error::Borrow::WrongThread
                )))
            )
        },
        || {},
    );
}

#[test]
fn add_unique_while_borrowing() {
    let world = World::new();
    world.try_add_unique(0u32).unwrap();
    let _s = world.try_borrow::<UniqueView<'_, u32>>().unwrap();
    world.try_add_unique(0usize).unwrap();
}

#[test]
fn sparse_set_and_unique() {
    let world = World::new();
    world.try_add_unique(0u32).unwrap();
    world
        .try_borrow::<(UniqueViewMut<u32>, ViewMut<u32>)>()
        .unwrap();
}

#[test]
#[cfg(all(feature = "non_send", feature = "non_sync"))]
fn exhaustive_list() {
    let world = World::new();

    let _ = world.try_borrow::<(
        NonSend<View<NotSend>>,
        NonSync<View<NotSync>>,
        NonSendSync<View<NotSendSync>>,
        NonSend<ViewMut<NotSend>>,
        NonSync<ViewMut<NotSync>>,
        NonSendSync<ViewMut<NotSendSync>>,
    )>();

    world.run(|all_storages: AllStoragesViewMut| {
        let _ = all_storages.try_borrow::<(
            NonSend<View<NotSend>>,
            NonSync<View<NotSync>>,
            NonSendSync<View<NotSendSync>>,
            NonSend<ViewMut<NotSend>>,
            NonSync<ViewMut<NotSync>>,
            NonSendSync<ViewMut<NotSendSync>>,
        )>();
    });
}

#[test]
#[cfg(all(feature = "non_send", feature = "non_sync"))]
fn unique_exhaustive_list() {
    let world = World::new();

    world.add_unique_non_send(NotSend(&()));
    world.add_unique_non_sync(NotSync(&()));
    world.add_unique_non_send_sync(NotSendSync(&()));

    let _ = world.try_borrow::<(
        NonSend<UniqueView<NotSend>>,
        NonSync<UniqueView<NotSync>>,
        NonSendSync<UniqueView<NotSendSync>>,
        NonSend<UniqueViewMut<NotSend>>,
        NonSync<UniqueViewMut<NotSync>>,
        NonSendSync<UniqueViewMut<NotSendSync>>,
    )>();

    world.run(|all_storages: AllStoragesViewMut| {
        let _ = all_storages.try_borrow::<(
            NonSend<UniqueView<NotSend>>,
            NonSync<UniqueView<NotSync>>,
            NonSendSync<UniqueView<NotSendSync>>,
            NonSend<UniqueViewMut<NotSend>>,
            NonSync<UniqueViewMut<NotSync>>,
            NonSendSync<UniqueViewMut<NotSendSync>>,
        )>();
    });
}
