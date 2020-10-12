use crate::error;
use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, Entities};
use crate::view::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
#[cfg(feature = "non_send")]
use crate::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
use crate::NonSendSync;
#[cfg(feature = "non_sync")]
use crate::NonSync;

/// Allows a type to be borrowed by [`AllStorages::borrow`] and [`AllStorages::run`].
pub trait AllStoragesBorrow<'a> {
    /// This function is where the actual borrowing happens.
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
        all_storages
            .private_get_or_insert(Entities::new)
            .map(|entities| EntitiesView {
                entities,
                all_borrow: None,
            })
            .map_err(error::GetStorage::Entities)
    }
}

impl<'a> AllStoragesBorrow<'a> for EntitiesViewMut<'a> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .private_get_or_insert_mut(Entities::new)
            .map(|entities| EntitiesViewMut {
                entities,
                _all_borrow: None,
            })
            .map_err(error::GetStorage::Entities)
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for View<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert(SparseSet::new)
            .map(|sparse_set| View {
                sparse_set,
                all_borrow: None,
            })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<View<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_non_send(SparseSet::new)
            .map(|sparse_set| {
                NonSend(View {
                    sparse_set,
                    all_borrow: None,
                })
            })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<View<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_non_sync(SparseSet::new)
            .map(|sparse_set| {
                NonSync(View {
                    sparse_set,
                    all_borrow: None,
                })
            })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<View<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_non_send_sync(SparseSet::new)
            .map(|sparse_set| {
                NonSendSync(View {
                    sparse_set,
                    all_borrow: None,
                })
            })
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for ViewMut<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_mut(SparseSet::new)
            .map(|sparse_set| ViewMut {
                sparse_set,
                _all_borrow: None,
            })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_non_send_mut(SparseSet::new)
            .map(|sparse_set| {
                NonSend(ViewMut {
                    sparse_set,
                    _all_borrow: None,
                })
            })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_non_sync_mut(SparseSet::new)
            .map(|sparse_set| {
                NonSync(ViewMut {
                    sparse_set,
                    _all_borrow: None,
                })
            })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages
            .get_or_insert_non_send_sync_mut(SparseSet::new)
            .map(|sparse_set| {
                NonSendSync(ViewMut {
                    sparse_set,
                    _all_borrow: None,
                })
            })
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for UniqueView<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get().map(|unique| UniqueView {
            unique,
            all_borrow: None,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get().map(|unique| {
            NonSend(UniqueView {
                unique,
                all_borrow: None,
            })
        })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get().map(|unique| {
            NonSync(UniqueView {
                unique,
                all_borrow: None,
            })
        })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get().map(|unique| {
            NonSendSync(UniqueView {
                unique,
                all_borrow: None,
            })
        })
    }
}

impl<'a, T: 'static + Send + Sync> AllStoragesBorrow<'a> for UniqueViewMut<'a, T> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get_mut().map(|unique| UniqueViewMut {
            unique,
            _all_borrow: None,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> AllStoragesBorrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get_mut().map(|unique| {
            NonSend(UniqueViewMut {
                unique,
                _all_borrow: None,
            })
        })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> AllStoragesBorrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get_mut().map(|unique| {
            NonSync(UniqueViewMut {
                unique,
                _all_borrow: None,
            })
        })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> AllStoragesBorrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        all_storages.get_mut().map(|unique| {
            NonSendSync(UniqueViewMut {
                unique,
                _all_borrow: None,
            })
        })
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
