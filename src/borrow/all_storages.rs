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
use core::convert::TryInto;

pub trait AllStoragesBorrow<'a> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage>
    where
        Self: Sized;
}

impl<'a> AllStoragesBorrow<'a> for () {
    fn try_borrow(_: &'a AllStorages) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesView<'a> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesViewMut<'a> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for View<'a, T> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for ViewMut<'a, T> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for UniqueView<'a, T> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for UniqueViewMut<'a, T> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into()
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<View<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::try_storage_from_non_send(all_storages).map(NonSend)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<ViewMut<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::try_storage_from_non_send(all_storages).map(NonSend)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<View<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::try_storage_from_non_sync(all_storages).map(NonSync)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<ViewMut<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::try_storage_from_non_sync(all_storages).map(NonSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSendSync<View<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        View::try_storage_from_non_send_sync(all_storages).map(NonSendSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSendSync<ViewMut<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        ViewMut::try_storage_from_non_send_sync(all_storages).map(NonSendSync)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<UniqueView<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into().map(NonSend)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into().map(NonSend)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<UniqueView<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into().map(NonSync)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into().map(NonSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSendSync<UniqueView<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into().map(NonSendSync)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.try_into().map(NonSendSync)
    }
}

macro_rules! impl_all_storages_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AllStoragesBorrow<'a>),+> AllStoragesBorrow<'a> for ($($type,)+) {
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
