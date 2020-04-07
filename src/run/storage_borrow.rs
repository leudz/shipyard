use crate::storage::{AllStorages, Entities, EntitiesMut};
use crate::views::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
#[cfg(feature = "non_send")]
use crate::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
use crate::NonSendSync;
#[cfg(feature = "non_sync")]
use crate::NonSync;
use crate::{error, Unique};
use core::convert::TryInto;

pub trait StorageBorrow<'a> {
    type View;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage>;
}

impl<'a> StorageBorrow<'a> for () {
    type View = ();

    fn try_borrow(_: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        Ok(())
    }
}

impl<'a> StorageBorrow<'a> for Entities {
    type View = EntitiesView<'a>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a> StorageBorrow<'a> for EntitiesMut {
    type View = EntitiesViewMut<'a>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> StorageBorrow<'a> for &T {
    type View = View<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> StorageBorrow<'a> for &mut T {
    type View = ViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> StorageBorrow<'a> for Unique<&T> {
    type View = UniqueView<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }
}

impl<'a, T: 'static + Send + Sync> StorageBorrow<'a> for Unique<&mut T> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        all_storages.try_into()
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> StorageBorrow<'a> for NonSend<&T> {
    type View = View<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        View::try_storage_from_non_send(all_storages)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> StorageBorrow<'a> for NonSend<&mut T> {
    type View = ViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        ViewMut::try_storage_from_non_send(all_storages)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> StorageBorrow<'a> for Unique<NonSend<&T>> {
    type View = UniqueView<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        UniqueView::try_storage_from_non_send(all_storages)
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> StorageBorrow<'a> for Unique<NonSend<&mut T>> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        UniqueViewMut::try_storage_from_non_send(all_storages)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> StorageBorrow<'a> for NonSync<&T> {
    type View = View<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        View::try_storage_from_non_sync(all_storages)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> StorageBorrow<'a> for NonSync<&mut T> {
    type View = ViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        ViewMut::try_storage_from_non_sync(all_storages)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> StorageBorrow<'a> for Unique<NonSync<&T>> {
    type View = UniqueView<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        UniqueView::try_storage_from_non_sync(all_storages)
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> StorageBorrow<'a> for Unique<NonSync<&mut T>> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        UniqueViewMut::try_storage_from_non_sync(all_storages)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> StorageBorrow<'a> for NonSendSync<&T> {
    type View = View<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        View::try_storage_from_non_send_sync(all_storages)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> StorageBorrow<'a> for NonSendSync<&mut T> {
    type View = ViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        ViewMut::try_storage_from_non_send_sync(all_storages)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> StorageBorrow<'a> for Unique<NonSendSync<&T>> {
    type View = UniqueView<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        UniqueView::try_storage_from_non_send_sync(all_storages)
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> StorageBorrow<'a> for Unique<NonSendSync<&mut T>> {
    type View = UniqueViewMut<'a, T>;

    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self::View, error::GetStorage> {
        UniqueViewMut::try_storage_from_non_send_sync(all_storages)
    }
}

macro_rules! impl_system_data {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: StorageBorrow<'a>),+> StorageBorrow<'a> for ($($type,)+) {
            type View = ($($type::View,)+);

            fn try_borrow(
                storages: &'a AllStorages,
            ) -> Result<Self::View, error::GetStorage> {
                    Ok(($(
                        <$type as StorageBorrow>::try_borrow(storages)?,
                    )+))
            }
        }
    }
}

macro_rules! system_data {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_system_data![$(($type, $index))*];
        system_data![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_system_data![$(($type, $index))*];
    }
}

system_data![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
