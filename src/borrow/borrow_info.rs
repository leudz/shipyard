#[cfg(feature = "thread_local")]
use super::non_send::NonSend;
#[cfg(feature = "thread_local")]
use super::non_send_sync::NonSendSync;
#[cfg(feature = "thread_local")]
use super::non_sync::NonSync;
use super::Mutability;
use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::component::{Component, Unique};
use crate::entities::Entities;
use crate::error;
use crate::scheduler::TypeInfo;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::system::Nothing;
use crate::tracking::Tracking;
use crate::unique::UniqueStorage;
use crate::views::{
    AllStoragesView, AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut,
    View, ViewMut,
};
use alloc::vec::Vec;
use core::any::type_name;

/// Explains to a workload which storage are borrowed by a system.
///
/// # Safety
///
/// Must accurately list everything borrowed.
///
/// ### Example of manual implementation:
/// ```rust
/// use shipyard::{borrow::BorrowInfo, scheduler::info::TypeInfo, track, View, UniqueView};
///
/// # struct Camera {}
/// # impl shipyard::Unique for Camera {}
/// # struct Position {}
/// # impl shipyard::Component for Position {
/// #     type Tracking = track::Untracked;
/// # }
/// #
/// struct CameraView<'v> {
///     camera: UniqueView<'v, Camera>,
///     position: View<'v, Position>,
/// }
///
/// // SAFE: All storages info is recorded.
/// unsafe impl BorrowInfo for CameraView<'_> {
///     fn borrow_info(info: &mut Vec<TypeInfo>) {
///         <UniqueView<'_, Camera>>::borrow_info(info);
///         <View<'_, Position>>::borrow_info(info);
///     }
///     fn enable_tracking(
///         _: &mut Vec<
///             for<'a> fn(&'a shipyard::AllStorages) -> Result<(), shipyard::error::GetStorage>,
///         >,
///     ) {}
/// }
/// ```
pub unsafe trait BorrowInfo {
    /// This information is used during workload creation to determine which systems can run in parallel.
    ///
    /// A borrow error might happen if the information is not correct.
    fn borrow_info(info: &mut Vec<TypeInfo>);
    /// Enable tracking on the `World` where this storage is borrowed.
    #[allow(clippy::type_complexity)]
    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    );
}

// this is needed for downstream crates to impl IntoWorkloadSystem
unsafe impl BorrowInfo for Nothing {
    fn borrow_info(_: &mut Vec<TypeInfo>) {}
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl BorrowInfo for () {
    fn borrow_info(_: &mut Vec<TypeInfo>) {}
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<'a> BorrowInfo for AllStoragesView<'a> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<AllStorages>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<AllStorages>(),
            #[cfg(not(feature = "thread_local"))]
            thread_safe: true,
            #[cfg(feature = "thread_local")]
            thread_safe: false,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<'a> BorrowInfo for AllStoragesViewMut<'a> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<AllStorages>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<AllStorages>(),
            #[cfg(not(feature = "thread_local"))]
            thread_safe: true,
            #[cfg(feature = "thread_local")]
            thread_safe: false,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<'a> BorrowInfo for EntitiesView<'a> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<Entities>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<Entities>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<'a> BorrowInfo for EntitiesViewMut<'a> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<Entities>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<Entities>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<'a, T: Send + Sync + Component, Track> BorrowInfo for View<'a, T, Track>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_mut(SparseSet::<T>::new)?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Sync + Component, Track> BorrowInfo for NonSend<View<'a, T, Track>>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: true,
        });
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_non_send_mut(|| NonSend(SparseSet::<T>::new()))?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Send + Component, Track> BorrowInfo for NonSync<View<'a, T, Track>>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: false,
        });
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_non_sync_mut(|| NonSync(SparseSet::<T>::new()))?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Component, Track> BorrowInfo for NonSendSync<View<'a, T, Track>>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: false,
        });
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_non_send_sync_mut(|| NonSendSync(SparseSet::<T>::new()))?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

unsafe impl<'a, T: Send + Sync + Component, Track> BorrowInfo for ViewMut<'a, T, Track>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_mut(SparseSet::<T>::new)?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Sync + Component, Track> BorrowInfo for NonSend<ViewMut<'a, T, Track>>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: false,
        });
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_non_send_mut(|| NonSend(SparseSet::<T>::new()))?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Send + Component, Track> BorrowInfo for NonSync<ViewMut<'a, T, Track>>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: true,
        });
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_non_sync_mut(|| NonSync(SparseSet::<T>::new()))?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Component, Track> BorrowInfo for NonSendSync<ViewMut<'a, T, Track>>
where
    Track: Tracking,
{
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<SparseSet<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<SparseSet<T>>(),
            thread_safe: false,
        });
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        enable_tracking_fn.push(|all_storages| {
            all_storages
                .custom_storage_or_insert_non_send_sync_mut(|| NonSendSync(SparseSet::<T>::new()))?
                .enable_tracking::<Track>();

            Ok(())
        })
    }
}

unsafe impl<'a, T: Send + Sync + Unique> BorrowInfo for UniqueView<'a, T> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Sync + Unique> BorrowInfo for NonSend<UniqueView<'a, T>> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Send + Unique> BorrowInfo for NonSync<UniqueView<'a, T>> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: false,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Unique> BorrowInfo for NonSendSync<UniqueView<'a, T>> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Shared,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: false,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<'a, T: Send + Sync + Unique> BorrowInfo for UniqueViewMut<'a, T> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Sync + Unique> BorrowInfo for NonSend<UniqueViewMut<'a, T>> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: false,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Send + Unique> BorrowInfo for NonSync<UniqueViewMut<'a, T>> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: true,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

#[cfg(feature = "thread_local")]
unsafe impl<'a, T: Unique> BorrowInfo for NonSendSync<UniqueViewMut<'a, T>> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        info.push(TypeInfo {
            name: type_name::<UniqueStorage<T>>().into(),
            mutability: Mutability::Exclusive,
            storage_id: StorageId::of::<UniqueStorage<T>>(),
            thread_safe: false,
        });
    }
    fn enable_tracking(_: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {}
}

unsafe impl<T: BorrowInfo> BorrowInfo for Option<T> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        T::borrow_info(info);
    }
    fn enable_tracking(
        enable_tracking: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        T::enable_tracking(enable_tracking);
    }
}

macro_rules! impl_borrow_info {
    ($(($type: ident, $index: tt))+) => {
        unsafe impl<$($type: BorrowInfo),+> BorrowInfo for ($($type,)+) {
            fn borrow_info(info: &mut Vec<TypeInfo>) {
                $(
                    $type::borrow_info(info);
                )+
            }
            fn enable_tracking(enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>) {
                $(
                    $type::enable_tracking(enable_tracking_fn);
                )+
            }
        }
    }
}

macro_rules! borrow_info {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_borrow_info![$(($type, $index))*];
        borrow_info![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_borrow_info![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
borrow_info![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
borrow_info![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
