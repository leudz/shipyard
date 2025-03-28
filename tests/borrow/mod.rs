use core::any::type_name;
use shipyard::error;
use shipyard::sparse_set::SparseSet;
use shipyard::*;

#[allow(unused)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}
impl Unique for USIZE {}

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}
impl Unique for U32 {}

#[derive(PartialEq, Eq, Debug)]
struct I32(i32);
impl Component for I32 {
    type Tracking = track::Untracked;
}

#[allow(unused)]
#[cfg(feature = "thread_local")]
struct NotSend(*const ());

#[cfg(feature = "thread_local")]
unsafe impl Sync for NotSend {}

#[cfg(feature = "thread_local")]
impl Component for NotSend {
    type Tracking = track::Untracked;
}
#[cfg(feature = "thread_local")]
impl Unique for NotSend {}

#[allow(unused)]
#[cfg(feature = "thread_local")]
struct NotSync(*const ());

#[cfg(feature = "thread_local")]
unsafe impl Send for NotSync {}

#[cfg(feature = "thread_local")]
impl Component for NotSync {
    type Tracking = track::Untracked;
}
#[cfg(feature = "thread_local")]
impl Unique for NotSync {}

#[allow(unused)]
#[cfg(feature = "thread_local")]
struct NotSendSync(*const ());

#[cfg(feature = "thread_local")]
impl Component for NotSendSync {
    type Tracking = track::Untracked;
}
#[cfg(feature = "thread_local")]
impl Unique for NotSendSync {}

#[test]
fn simple_borrow() {
    let world = World::new();

    let u32s = world.borrow::<View<U32>>().unwrap();
    assert_eq!(u32s.len(), 0);
}

#[test]
fn option_borrow() {
    let world = World::new();

    let u32s = world.borrow::<Option<View<U32>>>().unwrap();

    let u32s = u32s.unwrap();
    assert_eq!(u32s.len(), 0);
    drop(u32s);
    let _i32s = world.borrow::<ViewMut<I32>>().unwrap();
    let other_i32s = world.borrow::<Option<View<I32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
fn all_storages_simple_borrow() {
    let world = World::new();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let u32s = all_storages.borrow::<View<U32>>().unwrap();
    assert_eq!(u32s.len(), 0);
}

#[test]
fn invalid_borrow() {
    let world = World::new();

    let _u32s = world.borrow::<ViewMut<U32>>().unwrap();
    assert_eq!(
        world.borrow::<ViewMut<U32>>().err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(type_name::<SparseSet<U32>>()),
            id: StorageId::of::<SparseSet<U32>>(),
            borrow: error::Borrow::Unique
        })
    );
}

#[test]
fn all_storages_invalid_borrow() {
    let world = World::new();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let _u32s = all_storages.borrow::<ViewMut<U32>>().unwrap();
    assert_eq!(
        all_storages.borrow::<ViewMut<U32>>().err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(type_name::<SparseSet<U32>>()),
            id: StorageId::of::<SparseSet<U32>>(),
            borrow: error::Borrow::Unique
        })
    );
}

#[test]
fn double_borrow() {
    let world = World::new();

    let u32s = world.borrow::<ViewMut<U32>>().unwrap();
    drop(u32s);
    world.borrow::<ViewMut<U32>>().unwrap();
}

#[test]
fn all_storages_double_borrow() {
    let world = World::new();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let u32s = all_storages.borrow::<ViewMut<U32>>().unwrap();
    drop(u32s);
    all_storages.borrow::<ViewMut<U32>>().unwrap();
}

#[test]
fn all_storages_option_borrow() {
    let world = World::new();
    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

    let u32s = all_storages.borrow::<Option<View<U32>>>().unwrap();

    let u32s = u32s.unwrap();
    assert_eq!(u32s.len(), 0);
    drop(u32s);
    let _i32s = all_storages.borrow::<ViewMut<I32>>().unwrap();
    let other_i32s = all_storages.borrow::<Option<View<I32>>>().unwrap();
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
                    name: Some(type_name::<NonSend<SparseSet<NotSend>>>()),
                    id: StorageId::of::<NonSend<SparseSet<NotSend>>>(),
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
                    name: Some(type_name::<NonSendSync<SparseSet<NotSendSync>>>()),
                    id: StorageId::of::<NonSendSync<SparseSet<NotSendSync>>>(),
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
    world.add_unique(U32(0));
    let _s = world.borrow::<UniqueView<'_, U32>>().unwrap();
    world.add_unique(USIZE(0));
}

#[test]
fn sparse_set_and_unique() {
    let world = World::new();
    world.add_unique(U32(0));
    world
        .borrow::<(UniqueViewMut<U32>, ViewMut<U32>)>()
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

    world.run(|all_storages: AllStoragesViewMut| {
        let _ = all_storages.borrow::<(
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
#[cfg(feature = "thread_local")]
fn unique_exhaustive_list() {
    let world = World::new();

    world.add_unique_non_send(NotSend(&()));
    world.add_unique_non_sync(NotSync(&()));
    world.add_unique_non_send_sync(NotSendSync(&()));

    let _ = world.borrow::<(
        NonSend<UniqueView<NotSend>>,
        NonSync<UniqueView<NotSync>>,
        NonSendSync<UniqueView<NotSendSync>>,
        NonSend<UniqueViewMut<NotSend>>,
        NonSync<UniqueViewMut<NotSync>>,
        NonSendSync<UniqueViewMut<NotSendSync>>,
    )>();

    world.run(|all_storages: AllStoragesViewMut| {
        let _ = all_storages.borrow::<(
            NonSend<UniqueView<NotSend>>,
            NonSync<UniqueView<NotSync>>,
            NonSendSync<UniqueView<NotSendSync>>,
            NonSend<UniqueViewMut<NotSend>>,
            NonSync<UniqueViewMut<NotSync>>,
            NonSendSync<UniqueViewMut<NotSendSync>>,
        )>();
    });
}
