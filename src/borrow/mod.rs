mod borrow_info;
mod fake_borrow;
#[cfg(feature = "non_send")]
mod non_send;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
mod non_send_sync;
#[cfg(feature = "non_sync")]
mod non_sync;
mod world;

pub use borrow_info::BorrowInfo;
pub use fake_borrow::FakeBorrow;
#[cfg(feature = "non_send")]
pub use non_send::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
pub use non_send_sync::NonSendSync;
#[cfg(feature = "non_sync")]
pub use non_sync::NonSync;
pub use world::WorldBorrow;

use crate::all_storages::AllStorages;
use crate::atomic_refcell::{Ref, RefMut, SharedBorrow};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::view::{EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutability {
    Shared,
    Exclusive,
}

/// Allows a type to be borrowed by [`AllStorages::borrow`] and [`AllStorages::run`].
pub trait Borrow<'a> {
    /// This function is where the actual borrowing happens.
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage>
    where
        Self: Sized;
}

impl<'a> Borrow<'a> for () {
    #[inline]
    fn borrow(_: &'a AllStorages, _: Option<SharedBorrow<'a>>) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'a> Borrow<'a> for EntitiesView<'a> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { Ref::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow,
        })
    }
}

impl<'a> Borrow<'a> for EntitiesViewMut<'a> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { RefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        })
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for View<'a, T> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<View<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<View<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<View<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for ViewMut<'a, T> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<ViewMut<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<ViewMut<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<ViewMut<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueView<'a, T> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueView<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSend(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueView<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueView<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueViewMut<'a, T> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSend(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

impl<'a, T: Borrow<'a>> Borrow<'a> for Option<T> {
    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self, error::GetStorage> {
        Ok(T::borrow(all_storages, all_borrow).ok())
    }
}

macro_rules! impl_all_storages_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: Borrow<'a>),+> Borrow<'a> for ($($type,)+) {
            #[inline]
            fn borrow(
                all_storages: &'a AllStorages, all_borrow: Option<SharedBorrow<'a>>
            ) -> Result<Self, error::GetStorage> {
                    Ok(($(
                        <$type as Borrow>::borrow(all_storages, all_borrow.clone())?,
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
