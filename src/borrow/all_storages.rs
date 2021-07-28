use core::marker::PhantomData;

#[cfg(feature = "thread_local")]
use super::NonSend;
#[cfg(feature = "thread_local")]
use super::NonSendSync;
#[cfg(feature = "thread_local")]
use super::NonSync;
use super::{
    Borrow, EntitiesBorrower, EntitiesMutBorrower, UniqueViewBorrower, UniqueViewMutBorrower,
    UnitBorrower, ViewBorrower, ViewMutBorrower,
};
use crate::all_storages::{AllStorages, CustomStorageAccess};
use crate::atomic_refcell::{Ref, RefMut};
use crate::component::Component;
use crate::error;
use crate::sparse_set::SparseSet;
use crate::track;
use crate::view::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};

/// Allows a type to be borrowed by [`AllStorages::borrow`] and [`AllStorages::run`].
pub trait AllStoragesBorrow<'a>: Borrow<'a> {
    /// This function is where the actual borrowing happens.
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage>;
}

impl<'a> AllStoragesBorrow<'a> for UnitBorrower {
    #[inline]
    fn all_borrow(_: &'a AllStorages) -> Result<Self::View, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesBorrower {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { Ref::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow: None,
        })
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesMutBorrower {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { RefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: None,
        })
    }
}

impl<'a, T: Send + Sync + Component> AllStoragesBorrow<'a> for ViewBorrower<T>
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Send + Sync,
{
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> AllStoragesBorrow<'a> for NonSend<ViewBorrower<T>>
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Sync,
{
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> AllStoragesBorrow<'a> for NonSync<ViewBorrower<T>>
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Send,
{
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> AllStoragesBorrow<'a> for NonSendSync<ViewBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow: None,
        }))
    }
}

impl<'a, T: Send + Sync + Component> AllStoragesBorrow<'a> for ViewMutBorrower<T>
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Send + Sync,
{
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> AllStoragesBorrow<'a> for NonSend<ViewMutBorrower<T>>
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Sync,
{
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> AllStoragesBorrow<'a> for NonSync<ViewMutBorrower<T>>
where
    <T::Tracking as track::Tracking<T>>::DeletionData: Send,
{
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> AllStoragesBorrow<'a> for NonSendSync<ViewMutBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: None,
        }))
    }
}

impl<'a, T: Send + Sync + Component> AllStoragesBorrow<'a> for UniqueViewBorrower<T> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            _phantom: PhantomData,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> AllStoragesBorrow<'a> for NonSend<UniqueViewBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> AllStoragesBorrow<'a> for NonSync<UniqueViewBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> AllStoragesBorrow<'a> for NonSendSync<UniqueViewBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow: None,
            _phantom: PhantomData,
        }))
    }
}

impl<'a, T: Send + Sync + Component> AllStoragesBorrow<'a> for UniqueViewMutBorrower<T> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
            _phantom: PhantomData,
        })
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Sync + Component> AllStoragesBorrow<'a> for NonSend<UniqueViewMutBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Send + Component> AllStoragesBorrow<'a> for NonSync<UniqueViewMutBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
            _phantom: PhantomData,
        }))
    }
}

#[cfg(feature = "thread_local")]
impl<'a, T: Component> AllStoragesBorrow<'a> for NonSendSync<UniqueViewMutBorrower<T>> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: None,
            _phantom: PhantomData,
        }))
    }
}

impl<'a, T: AllStoragesBorrow<'a>> AllStoragesBorrow<'a> for Option<T> {
    #[inline]
    fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        Ok(T::all_borrow(all_storages).ok())
    }
}

macro_rules! impl_all_storages_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AllStoragesBorrow<'a>),+> AllStoragesBorrow<'a> for ($($type,)+) {
            #[inline]
            fn all_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
                Ok(($($type::all_borrow(all_storages)?,)+))
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
