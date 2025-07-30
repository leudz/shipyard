mod borrow_info;
#[cfg(feature = "thread_local")]
mod non_send;
#[cfg(feature = "thread_local")]
mod non_send_sync;
#[cfg(feature = "thread_local")]
mod non_sync;
mod world_borrow;

pub use borrow_info::BorrowInfo;
#[cfg(feature = "thread_local")]
pub use non_send::NonSend;
#[cfg(feature = "thread_local")]
pub use non_send_sync::NonSendSync;
#[cfg(feature = "thread_local")]
pub use non_sync::NonSync;
pub use world_borrow::WorldBorrow;

use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::atomic_refcell::{ARef, ARefMut, SharedBorrow};
use crate::component::{Component, Unique};
use crate::error;
use crate::sparse_set::SparseSet;
#[cfg(feature = "thread_local")]
use crate::storage::StorageId;
use crate::system::Nothing;
use crate::tracking::{Tracking, TrackingTimestamp};
use crate::unique::UniqueStorage;
use crate::views::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
use core::marker::PhantomData;

/// Describes if a storage is borrowed exclusively or not.  
/// It is used to display workloads' borrowing information.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum Mutability {
    #[allow(missing_docs)]
    Shared,
    #[allow(missing_docs)]
    Exclusive,
}

/// Allows a type to be borrowed by [`AllStorages::borrow`], [`AllStorages::run`],
/// [`World::borrow`], [`World::run`] and workloads.
///
/// ### Example of manual implementation:
/// ```rust
/// use shipyard::{AllStorages, borrow::Borrow, atomic_refcell::SharedBorrow, track, tracking::TrackingTimestamp, View, UniqueView};
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
/// impl Borrow for CameraView<'_> {
///     type View<'v> = CameraView<'v>;
///
///     fn borrow<'a>(
///         all_storages: &'a AllStorages,
///         all_borrow: Option<SharedBorrow<'a>>,
///         last_run: Option<TrackingTimestamp>,
///         current: TrackingTimestamp,
///     ) -> Result<Self::View<'a>, shipyard::error::GetStorage> {
///         Ok(CameraView {
///             camera: UniqueView::<Camera>::borrow(all_storages, all_borrow.clone(), last_run, current)?,
///             position: View::<Position>::borrow(all_storages, all_borrow, last_run, current)?,
///         })
///     }
/// }
/// ```
///
/// [`World::borrow`]: crate::World::borrow
/// [`World::run`]: crate::World::run
pub trait Borrow {
    #[allow(missing_docs)]
    type View<'a>;

    /// This function is where the actual borrowing happens.
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage>;
}

// this is needed for downstream crate to impl System
impl Borrow for Nothing {
    type View<'a> = ();

    fn borrow<'a>(
        _all_storages: &'a AllStorages,
        _all_borrow: Option<SharedBorrow<'a>>,
        _last_run: Option<TrackingTimestamp>,
        _current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        Ok(())
    }
}

impl Borrow for () {
    type View<'a> = ();

    #[inline]
    fn borrow<'a>(
        _all_storages: &'a AllStorages,
        _all_borrow: Option<SharedBorrow<'a>>,
        _last_run: Option<TrackingTimestamp>,
        _current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl Borrow for EntitiesView<'_> {
    type View<'a> = EntitiesView<'a>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        _last_run: Option<TrackingTimestamp>,
        _current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { ARef::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow,
        })
    }
}

impl Borrow for EntitiesViewMut<'_> {
    type View<'a> = EntitiesViewMut<'a>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        _last_run: Option<TrackingTimestamp>,
        _current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { ARefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        })
    }
}

impl<T: Send + Sync + Component, Track> Borrow for View<'_, T, Track>
where
    Track: Tracking,
{
    type View<'a> = View<'a, T, Track>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(View::new(sparse_set, borrow, all_borrow, last_run, current))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component, Track> Borrow for NonSend<View<'_, T, Track>>
where
    Track: Tracking,
{
    type View<'a> = NonSend<View<'a, T, Track>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send(|| NonSend(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(NonSend(View::new(
            sparse_set, borrow, all_borrow, last_run, current,
        )))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component, Track> Borrow for NonSync<View<'_, T, Track>>
where
    Track: Tracking,
{
    type View<'a> = NonSync<View<'a, T, Track>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync(|| NonSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(NonSync(View::new(
            sparse_set, borrow, all_borrow, last_run, current,
        )))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component, Track> Borrow for NonSendSync<View<'_, T, Track>>
where
    Track: Tracking,
{
    type View<'a> = NonSendSync<View<'a, T, Track>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages
            .custom_storage_or_insert_non_send_sync(|| NonSendSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARef::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(NonSendSync(View::new(
            sparse_set, borrow, all_borrow, last_run, current,
        )))
    }
}

impl<T: Send + Sync + Component, Track> Borrow for ViewMut<'_, T, Track>
where
    Track: Tracking,
{
    type View<'a> = ViewMut<'a, T, Track>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(ViewMut {
            last_insertion: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(TrackingTimestamp::origin()),
            current,
            sparse_set,
            borrow,
            all_borrow,
            phantom: PhantomData,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component, Track> Borrow for NonSend<ViewMut<'_, T, Track>>
where
    Track: Tracking,
{
    type View<'a> = NonSend<ViewMut<'a, T, Track>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view =
            all_storages.custom_storage_or_insert_non_send_mut(|| NonSend(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(NonSend(ViewMut {
            last_insertion: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(TrackingTimestamp::origin()),
            current,
            sparse_set,
            borrow: borrow,
            all_borrow: all_borrow,
            phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component, Track> Borrow for NonSync<ViewMut<'_, T, Track>>
where
    Track: Tracking,
{
    type View<'a> = NonSync<ViewMut<'a, T, Track>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view =
            all_storages.custom_storage_or_insert_non_sync_mut(|| NonSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(NonSync(ViewMut {
            last_insertion: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(TrackingTimestamp::origin()),
            current,
            sparse_set,
            borrow: borrow,
            all_borrow: all_borrow,
            phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component, Track> Borrow for NonSendSync<ViewMut<'_, T, Track>>
where
    Track: Tracking,
{
    type View<'a> = NonSendSync<ViewMut<'a, T, Track>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages
            .custom_storage_or_insert_non_send_sync_mut(|| NonSendSync(SparseSet::new()))?;

        let (sparse_set, borrow) = unsafe { ARefMut::destructure(view) };

        sparse_set.check_tracking::<Track>()?;

        Ok(NonSendSync(ViewMut {
            last_insertion: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(TrackingTimestamp::origin()),
            current,
            sparse_set,
            borrow: borrow,
            all_borrow: all_borrow,
            phantom: PhantomData,
        }))
    }
}

impl<T: Send + Sync + Unique> Borrow for UniqueView<'_, T> {
    type View<'a> = UniqueView<'a, T>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { ARef::destructure(view) };

        Ok(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Unique> Borrow for NonSend<UniqueView<'_, T>> {
    type View<'a> = NonSend<UniqueView<'a, T>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_by_id(StorageId::of::<UniqueStorage<T>>())?;
        let view = ARef::map(view, |storage| {
            storage
                .as_any()
                .downcast_ref::<NonSend<UniqueStorage<T>>>()
                .unwrap()
        });

        let (unique, borrow) = unsafe { ARef::destructure(view) };

        Ok(NonSend(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Unique> Borrow for NonSync<UniqueView<'_, T>> {
    type View<'a> = NonSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_by_id(StorageId::of::<UniqueStorage<T>>())?;
        let view = ARef::map(view, |storage| {
            storage
                .as_any()
                .downcast_ref::<NonSync<UniqueStorage<T>>>()
                .unwrap()
        });

        let (unique, borrow) = unsafe { ARef::destructure(view) };

        Ok(NonSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique> Borrow for NonSendSync<UniqueView<'_, T>> {
    type View<'a> = NonSendSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_by_id(StorageId::of::<UniqueStorage<T>>())?;
        let view = ARef::map(view, |storage| {
            storage
                .as_any()
                .downcast_ref::<NonSendSync<UniqueStorage<T>>>()
                .unwrap()
        });

        let (unique, borrow) = unsafe { ARef::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

impl<T: Send + Sync + Unique> Borrow for UniqueViewMut<'_, T> {
    type View<'a> = UniqueViewMut<'a, T>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { ARefMut::destructure(view) };

        Ok(UniqueViewMut {
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Unique> Borrow for NonSend<UniqueViewMut<'_, T>> {
    type View<'a> = NonSend<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_mut_by_id(StorageId::of::<UniqueStorage<T>>())?;
        let view = ARefMut::map(view, |storage| {
            storage
                .as_any_mut()
                .downcast_mut::<NonSend<UniqueStorage<T>>>()
                .unwrap()
        });

        let (unique, borrow) = unsafe { ARefMut::destructure(view) };

        Ok(NonSend(UniqueViewMut {
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Unique> Borrow for NonSync<UniqueViewMut<'_, T>> {
    type View<'a> = NonSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_mut_by_id(StorageId::of::<UniqueStorage<T>>())?;
        let view = ARefMut::map(view, |storage| {
            storage
                .as_any_mut()
                .downcast_mut::<NonSync<UniqueStorage<T>>>()
                .unwrap()
        });

        let (unique, borrow) = unsafe { ARefMut::destructure(view) };

        Ok(NonSync(UniqueViewMut {
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique> Borrow for NonSendSync<UniqueViewMut<'_, T>> {
    type View<'a> = NonSendSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        let view = all_storages.custom_storage_mut_by_id(StorageId::of::<UniqueStorage<T>>())?;
        let view = ARefMut::map(view, |storage| {
            storage
                .as_any_mut()
                .downcast_mut::<NonSendSync<UniqueStorage<T>>>()
                .unwrap()
        });

        let (unique, borrow) = unsafe { ARefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            last_insertion: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

impl<T: Borrow> Borrow for Option<T> {
    type View<'a> = Option<T::View<'a>>;

    #[inline]
    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, error::GetStorage> {
        Ok(T::borrow(all_storages, all_borrow, last_run, current).ok())
    }
}

macro_rules! impl_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Borrow),+> Borrow for ($($type,)+) {
            type View<'a> = ($($type::View<'a>,)+);

            #[inline]
            fn borrow<'a>(
                all_storages: &'a AllStorages,
                all_borrow: Option<SharedBorrow<'a>>,
                last_run: Option<TrackingTimestamp>,
                current: TrackingTimestamp
            ) -> Result<Self::View<'a>, error::GetStorage> {
                Ok(($($type::borrow(all_storages, all_borrow.clone(), last_run, current)?,)+))
            }
        }
    }
}

macro_rules! borrow {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_borrow![$(($type, $index))*];
        borrow![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_borrow![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
borrow![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
borrow![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
