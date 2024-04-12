use core::any::type_name;
use shipyard::error;
use shipyard::*;

struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}
impl Unique for USIZE {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}
impl Unique for U64 {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug)]
struct I32(i32);
impl Component for I32 {
    type Tracking = track::Untracked;
}

#[cfg(feature = "thread_local")]
struct NotSend(*const ());

#[cfg(feature = "thread_local")]
unsafe impl Sync for NotSend {}

#[cfg(feature = "thread_local")]
impl Component for NotSend {
    type Tracking = track::Untracked;
}
#[cfg(feature = "thread_local")]
impl Unique for NotSend {
    type Tracking = track::Untracked;
}

#[cfg(feature = "thread_local")]
struct NotSync(*const ());

#[cfg(feature = "thread_local")]
unsafe impl Send for NotSync {}

#[cfg(feature = "thread_local")]
impl Component for NotSync {
    type Tracking = track::Untracked;
}
#[cfg(feature = "thread_local")]
impl Unique for NotSync {
    type Tracking = track::Untracked;
}

#[cfg(feature = "thread_local")]
struct NotSendSync(*const ());

#[cfg(feature = "thread_local")]
impl Component for NotSendSync {
    type Tracking = track::Untracked;
}
#[cfg(feature = "thread_local")]
impl Unique for NotSendSync {
    type Tracking = track::Untracked;
}

#[test]
fn simple_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let u64s = world.borrow::<View<U64>>().unwrap();
    assert_eq!(u64s.len(), 0);
}

#[test]
fn option_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let u64s = world.borrow::<Option<View<U64>>>().unwrap();

    let u64s = u64s.unwrap();
    assert_eq!(u64s.len(), 0);
    drop(u64s);
    let _i32s = world.borrow::<ViewMut<I32>>().unwrap();
    let other_i32s = world.borrow::<Option<View<I32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
fn all_storages_simple_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let u64s = all_storages.borrow::<View<U64>>().unwrap();
    assert_eq!(u64s.len(), 0);
}

#[test]
fn invalid_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let _u64s = world.borrow::<ViewMut<U64>>().unwrap();
    assert_eq!(
        world.borrow::<ViewMut<U64>>().err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(type_name::<SparseSet<U64>>()),
            id: StorageId::of::<SparseSet<U64>>(),
            borrow: error::Borrow::Unique
        })
    );
}

#[test]
fn all_storages_invalid_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let _u64s = all_storages.borrow::<ViewMut<U64>>().unwrap();
    assert_eq!(
        all_storages.borrow::<ViewMut<U64>>().err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(type_name::<SparseSet<U64>>()),
            id: StorageId::of::<SparseSet<U64>>(),
            borrow: error::Borrow::Unique
        })
    );
}

#[test]
fn double_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let u64s = world.borrow::<ViewMut<U64>>().unwrap();
    drop(u64s);
    world.borrow::<ViewMut<U64>>().unwrap();
}

#[test]
fn all_storages_double_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    let u64s = all_storages.borrow::<ViewMut<U64>>().unwrap();
    drop(u64s);
    all_storages.borrow::<ViewMut<U64>>().unwrap();
}

#[test]
fn all_storages_option_borrow() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

    let u64s = all_storages.borrow::<Option<View<U64>>>().unwrap();

    let u64s = u64s.unwrap();
    assert_eq!(u64s.len(), 0);
    drop(u64s);
    let _i32s = all_storages.borrow::<ViewMut<I32>>().unwrap();
    let other_i32s = all_storages.borrow::<Option<View<I32>>>().unwrap();
    assert!(other_i32s.is_none());
}

#[test]
#[cfg(feature = "thread_local")]
fn non_send_storage_in_other_thread() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
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
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
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
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.add_unique(U64(0));
    let _s = world.borrow::<UniqueView<'_, U64>>().unwrap();
    world.add_unique(USIZE(0));
}

#[test]
fn sparse_set_and_unique() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.add_unique(U64(0));
    world
        .borrow::<(UniqueViewMut<U64>, ViewMut<U64>)>()
        .unwrap();
}

#[test]
#[cfg(feature = "thread_local")]
fn exhaustive_list() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

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
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

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
