//use super::FakeBorrow;
use crate::error;
use crate::storage::AllStorages;
use crate::view::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
#[cfg(feature = "non_send")]
use crate::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
use crate::NonSendSync;
#[cfg(feature = "non_sync")]
use crate::NonSync;

pub trait AllStoragesBorrow<'a> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage>
    where
        Self: Sized;
}

impl<'a> AllStoragesBorrow<'a> for () {
    #[inline]
    fn try_borrow(_: &'a AllStorages) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesView<'a> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        EntitiesView::from_reference(all_storages)
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesViewMut<'a> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        EntitiesViewMut::from_reference(all_storages)
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for View<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::from_reference(all_storages)
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for ViewMut<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::from_reference(all_storages)
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for UniqueView<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueView::from_reference(all_storages)
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for UniqueViewMut<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_reference(all_storages)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<View<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::from_reference_non_send(all_storages).map(NonSend)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::from_reference_non_send(all_storages).map(NonSend)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<View<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::from_reference_non_sync(all_storages).map(NonSync)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::from_reference_non_sync(all_storages).map(NonSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<View<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::from_reference_non_send_sync(all_storages).map(NonSendSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::from_reference_non_send_sync(all_storages).map(NonSendSync)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueView::from_reference(all_storages).map(NonSend)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_reference(all_storages).map(NonSend)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueView::from_reference(all_storages).map(NonSync)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_reference(all_storages).map(NonSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueView::from_reference(all_storages).map(NonSendSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_reference(all_storages).map(NonSendSync)
    }
}

impl<'a, T: AllStoragesBorrow<'a>> AllStoragesBorrow<'a> for Option<T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        Ok(T::try_borrow(all_storages).ok())
    }
}

macro_rules! impl_all_storages_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AllStoragesBorrow<'a>),+> AllStoragesBorrow<'a> for ($($type,)+) {
            #[inline]
            fn try_borrow(
                all_storages: &'a AllStorages,
            ) -> Result<Self, error::GetStorage> {
                    Ok(($(
                        <$type as AllStoragesBorrow>::try_borrow(all_storages)?,
                    )+))
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
