mod all_storages;
mod borrow_info;
#[cfg(feature = "thread_local")]
mod non_send;
#[cfg(feature = "thread_local")]
mod non_send_sync;
#[cfg(feature = "thread_local")]
mod non_sync;

use crate::all_storages::CustomStorageAccess;
use crate::atomic_refcell::{Ref, RefMut};
use crate::component::{Component, Unique};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unique::UniqueStorage;
use crate::view::{
    AllStoragesView, AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut,
    View, ViewMut,
};
use crate::world::World;
pub use all_storages::AllStoragesBorrow;
pub use borrow_info::BorrowInfo;
#[cfg(feature = "thread_local")]
pub use non_send::NonSend;
#[cfg(feature = "thread_local")]
pub use non_send_sync::NonSendSync;
#[cfg(feature = "thread_local")]
pub use non_sync::NonSync;

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

/// Allows a type to be borrowed by [`World::borrow`], [`World::run`] and workloads.
///
/// ### Example of manual implementation:
/// ```rust
/// use shipyard::{Borrow, View, UniqueView, World};
///
/// # struct Camera {}
/// # impl shipyard::Unique for Camera {}
/// # struct Position {}
/// # impl shipyard::Component for Position {}
/// #
/// struct CameraView<'v> {
///     camera: UniqueView<'v, Camera>,
///     position: View<'v, Position>,
/// }
///
/// impl Borrow for CameraView<'_> {
///     type View<'v> = CameraView<'v>;
///
///     fn borrow(
///         world: &World,
///         last_run: Option<u32>,
///         current: u32,
///     ) -> Result<Self::View<'_>, shipyard::error::GetStorage> {
///         Ok(CameraView {
///             camera: UniqueView::<Camera>::borrow(world, last_run, current)?,
///             position: View::<Position>::borrow(world, last_run, current)?,
///         })
///     }
/// }
/// ```
pub trait Borrow {
    #[allow(missing_docs)]
    type View<'a>;

    /// This function is where the actual borrowing happens.
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage>;
}

impl Borrow for AllStoragesView<'_> {
    type View<'a> = AllStoragesView<'a>;

    fn borrow(
        world: &World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        world
            .all_storages
            .borrow()
            .map(AllStoragesView)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }
}

impl Borrow for AllStoragesViewMut<'_> {
    type View<'a> = AllStoragesViewMut<'a>;

    #[inline]
    fn borrow(
        world: &World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        world
            .all_storages
            .borrow_mut()
            .map(AllStoragesViewMut)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }
}

impl Borrow for () {
    type View<'a> = ();

    #[inline]
    fn borrow(
        _: &World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl Borrow for EntitiesView<'_> {
    type View<'a> = EntitiesView<'a>;

    #[inline]
    fn borrow(
        world: &World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { Ref::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        })
    }
}

impl Borrow for EntitiesViewMut<'_> {
    type View<'a> = EntitiesViewMut<'a>;

    #[inline]
    fn borrow(
        world: &World,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { RefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        })
    }
}

impl<T: Send + Sync + Component, const TRACK: u32> Borrow for View<'_, T, TRACK> {
    type View<'a> = View<'a, T, TRACK>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run
                .unwrap_or_else(|| current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component, const TRACK: u32> Borrow for NonSend<View<'_, T, TRACK>> {
    type View<'a> = NonSend<View<'a, T, TRACK>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(NonSend(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component, const TRACK: u32> Borrow for NonSync<View<'_, T, TRACK>> {
    type View<'a> = NonSync<View<'a, T, TRACK>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(NonSync(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component, const TRACK: u32> Borrow for NonSendSync<View<'_, T, TRACK>> {
    type View<'a> = NonSendSync<View<'a, T, TRACK>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(NonSendSync(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
        }))
    }
}

impl<T: Send + Sync + Component, const TRACK: u32> Borrow for ViewMut<'_, T, TRACK> {
    type View<'a> = ViewMut<'a, T, TRACK>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run
                .unwrap_or_else(|| current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Component, const TRACK: u32> Borrow for NonSend<ViewMut<'_, T, TRACK>> {
    type View<'a> = NonSend<ViewMut<'a, T, TRACK>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(NonSend(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Component, const TRACK: u32> Borrow for NonSync<ViewMut<'_, T, TRACK>> {
    type View<'a> = NonSync<ViewMut<'a, T, TRACK>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(NonSync(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Component, const TRACK: u32> Borrow for NonSendSync<ViewMut<'_, T, TRACK>> {
    type View<'a> = NonSendSync<ViewMut<'a, T, TRACK>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_or_insert_non_send_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        sparse_set.check_tracking::<TRACK>()?;

        Ok(NonSendSync(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

impl<T: Send + Sync + Unique> Borrow for UniqueView<'_, T> {
    type View<'a> = UniqueView<'a, T>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Unique> Borrow for NonSend<UniqueView<'_, T>> {
    type View<'a> = NonSend<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Unique> Borrow for NonSync<UniqueView<'_, T>> {
    type View<'a> = NonSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique> Borrow for NonSendSync<UniqueView<'_, T>> {
    type View<'a> = NonSendSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: Some(all_borrow),
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

impl<T: Send + Sync + Unique> Borrow for UniqueViewMut<'_, T> {
    type View<'a> = UniqueViewMut<'a, T>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        })
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync + Unique> Borrow for NonSend<UniqueViewMut<'_, T>> {
    type View<'a> = NonSend<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send + Unique> Borrow for NonSync<UniqueViewMut<'_, T>> {
    type View<'a> = NonSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique> Borrow for NonSendSync<UniqueViewMut<'_, T>> {
    type View<'a> = NonSendSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: Some(all_borrow),
        }))
    }
}

impl<T: Borrow> Borrow for Option<T> {
    type View<'a> = Option<T::View<'a>>;

    #[inline]
    fn borrow(
        world: &World,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        Ok(T::borrow(world, last_run, current).ok())
    }
}

macro_rules! impl_world_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Borrow),+> Borrow for ($($type,)+) {
            type View<'a> = ($($type::View<'a>,)+);

            #[inline]
            fn borrow(world: &World, last_run: Option<u32>, current: u32) -> Result<Self::View<'_>, error::GetStorage> {
                Ok(($($type::borrow(world, last_run, current)?,)+))
            }
        }
    }
}

macro_rules! world_borrow {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_world_borrow![$(($type, $index))*];
        world_borrow![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_world_borrow![$(($type, $index))*];
    }
}

world_borrow![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
