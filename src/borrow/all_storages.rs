#[cfg(feature = "thread_local")]
use super::NonSend;
#[cfg(feature = "thread_local")]
use super::NonSendSync;
#[cfg(feature = "thread_local")]
use super::NonSync;
use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::atomic_refcell::{Ref, RefMut};
use crate::component::Component;
use crate::component::Unique;
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unique::UniqueStorage;
use crate::view::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
use crate::Borrow;

/// Allows a type to be borrowed by [`AllStorages::borrow`] and [`AllStorages::run`].
pub trait AllStoragesBorrow: Borrow {
    /// This function is where the actual borrowing happens.
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage>;
}

impl AllStoragesBorrow for () {
    #[inline]
    fn all_borrow(
        _: &AllStorages,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl AllStoragesBorrow for EntitiesView<'_> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { Ref::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow: None,
        })
    }
}

impl AllStoragesBorrow for EntitiesViewMut<'_> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        _last_run: Option<u32>,
        _current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { RefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: None,
        })
    }
}

impl<T: Send + Sync + Component> AllStoragesBorrow for View<'_, T>
where
    T::Tracking: Send + Sync,
{
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run
                .unwrap_or_else(|| current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> AllStoragesBorrow for NonSend<View<'_, T>>
where
    T::Tracking: Sync,
{
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> AllStoragesBorrow for NonSync<View<'_, T>>
where
    T::Tracking: Send,
{
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> AllStoragesBorrow for NonSendSync<View<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(View {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        }))
    }
}

impl<T: Send + Sync + Component> AllStoragesBorrow for ViewMut<'_, T>
where
    T::Tracking: Send + Sync,
{
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run
                .unwrap_or_else(|| current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> AllStoragesBorrow for NonSend<ViewMut<'_, T>>
where
    T::Tracking: Sync,
{
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> AllStoragesBorrow for NonSync<ViewMut<'_, T>>
where
    T::Tracking: Send,
{
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> AllStoragesBorrow for NonSendSync<ViewMut<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(ViewMut {
            last_insert: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modification),
            last_removal_or_deletion: last_run.unwrap_or(current.wrapping_sub(u32::MAX / 2)),
            current,
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

impl<T: Send + Sync + Unique> AllStoragesBorrow for UniqueView<'_, T> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Unique> AllStoragesBorrow for NonSend<UniqueView<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Unique> AllStoragesBorrow for NonSync<UniqueView<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Unique> AllStoragesBorrow for NonSendSync<UniqueView<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
        }))
    }
}

impl<T: Send + Sync + Unique> AllStoragesBorrow for UniqueViewMut<'_, T> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Unique> AllStoragesBorrow for NonSend<UniqueViewMut<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Unique> AllStoragesBorrow for NonSync<UniqueViewMut<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Unique> AllStoragesBorrow for NonSendSync<UniqueViewMut<'_, T>> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        let view = all_storages.custom_storage_mut::<UniqueStorage<T>>()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            last_insert: last_run.unwrap_or(unique.last_insert),
            last_modification: last_run.unwrap_or(unique.last_modification),
            current,
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

impl<T: AllStoragesBorrow> AllStoragesBorrow for Option<T> {
    #[inline]
    fn all_borrow(
        all_storages: &AllStorages,
        last_run: Option<u32>,
        current: u32,
    ) -> Result<Self::View<'_>, error::GetStorage> {
        Ok(T::all_borrow(all_storages, last_run, current).ok())
    }
}

macro_rules! impl_all_storages_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AllStoragesBorrow),+> AllStoragesBorrow for ($($type,)+) {
            #[inline]
            fn all_borrow(
                all_storages: &AllStorages,
                last_run: Option<u32>,
                current: u32
            ) -> Result<Self::View<'_>, error::GetStorage> {
                Ok(($($type::all_borrow(all_storages, last_run, current)?,)+))
            }
        }
    }
}

macro_rules! all_storages_borrow {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_all_storages_borrow![$(($type, $index))*];
        all_storages_borrow![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_all_storages_borrow![$(($type, $index))*];
    }
}

all_storages_borrow![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
