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
pub use world::{IntoWorldBorrow, WorldBorrow};

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

pub trait IntoBorrow {
    type Borrow: for<'a> Borrow<'a>;
}

/// Allows a type to be borrowed by [`AllStorages::borrow`] and [`AllStorages::run`].
pub trait Borrow<'a> {
    type View;
    /// This function is where the actual borrowing happens.
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage>;
}

pub struct UnitBorrower;

impl IntoBorrow for () {
    type Borrow = UnitBorrower;
}

impl<'a> Borrow<'a> for UnitBorrower {
    type View = ();

    #[inline]
    fn borrow(
        _: &'a AllStorages,
        _: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }
}

pub struct EntitiesBorrower;

impl IntoBorrow for EntitiesView<'_> {
    type Borrow = EntitiesBorrower;
}

impl<'a> Borrow<'a> for EntitiesBorrower {
    type View = EntitiesView<'a>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        let entities = all_storages.entities()?;

        let (entities, borrow) = unsafe { Ref::destructure(entities) };

        Ok(EntitiesView {
            entities,
            borrow: Some(borrow),
            all_borrow,
        })
    }
}

pub struct EntitiesMutBorrower;

impl IntoBorrow for EntitiesViewMut<'_> {
    type Borrow = EntitiesMutBorrower;
}

impl<'a> Borrow<'a> for EntitiesMutBorrower {
    type View = EntitiesViewMut<'a>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        let entities = all_storages.entities_mut()?;

        let (entities, borrow) = unsafe { RefMut::destructure(entities) };

        Ok(EntitiesViewMut {
            entities,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        })
    }
}

pub struct ViewBorrower<T>(T);

impl<T: 'static + Send + Sync> IntoBorrow for View<'_, T> {
    type Borrow = ViewBorrower<T>;
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for ViewBorrower<T> {
    type View = View<'a, T>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Sync> IntoBorrow for NonSend<View<'_, T>> {
    type Borrow = NonSend<ViewBorrower<T>>;
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<ViewBorrower<T>> {
    type View = NonSend<View<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Send> IntoBorrow for NonSync<View<'_, T>> {
    type Borrow = NonSync<ViewBorrower<T>>;
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<ViewBorrower<T>> {
    type View = NonSync<View<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static> IntoBorrow for NonSendSync<View<'_, T>> {
    type Borrow = NonSendSync<ViewBorrower<T>>;
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<ViewBorrower<T>> {
    type View = NonSendSync<View<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(View {
            sparse_set,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

pub struct ViewMutBorrower<T>(T);

impl<T: 'static + Send + Sync> IntoBorrow for ViewMut<'_, T> {
    type Borrow = ViewMutBorrower<T>;
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for ViewMutBorrower<T> {
    type View = ViewMut<'a, T>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Sync> IntoBorrow for NonSend<ViewMut<'_, T>> {
    type Borrow = NonSend<ViewMutBorrower<T>>;
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<ViewMutBorrower<T>> {
    type View = NonSend<ViewMut<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Send> IntoBorrow for NonSync<ViewMut<'_, T>> {
    type Borrow = NonSync<ViewMutBorrower<T>>;
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<ViewMutBorrower<T>> {
    type View = NonSync<ViewMut<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static> IntoBorrow for NonSendSync<ViewMut<'_, T>> {
    type Borrow = NonSendSync<ViewMutBorrower<T>>;
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<ViewMutBorrower<T>> {
    type View = NonSendSync<ViewMut<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_or_insert_non_send_sync_mut(SparseSet::new)?;

        let (sparse_set, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(ViewMut {
            sparse_set,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

pub struct UniqueViewBorrower<T>(T);

impl<T: 'static + Send + Sync> IntoBorrow for UniqueView<'_, T> {
    type Borrow = UniqueViewBorrower<T>;
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueViewBorrower<T> {
    type View = UniqueView<'a, T>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Sync> IntoBorrow for NonSend<UniqueView<'_, T>> {
    type Borrow = NonSend<UniqueViewBorrower<T>>;
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueViewBorrower<T>> {
    type View = NonSend<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Send> IntoBorrow for NonSync<UniqueView<'_, T>> {
    type Borrow = NonSync<UniqueViewBorrower<T>>;
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueViewBorrower<T>> {
    type View = NonSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static> IntoBorrow for NonSendSync<UniqueView<'_, T>> {
    type Borrow = NonSendSync<UniqueViewBorrower<T>>;
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueViewBorrower<T>> {
    type View = NonSendSync<UniqueView<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage()?;

        let (unique, borrow) = unsafe { Ref::destructure(view) };

        Ok(NonSendSync(UniqueView {
            unique,
            borrow: Some(borrow),
            all_borrow,
        }))
    }
}

pub struct UniqueViewMutBorrower<T>(T);

impl<T: 'static + Send + Sync> IntoBorrow for UniqueViewMut<'_, T> {
    type Borrow = UniqueViewMutBorrower<T>;
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueViewMutBorrower<T> {
    type View = UniqueViewMut<'a, T>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Sync> IntoBorrow for NonSend<UniqueViewMut<'_, T>> {
    type Borrow = NonSend<UniqueViewMutBorrower<T>>;
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueViewMutBorrower<T>> {
    type View = NonSend<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static + Send> IntoBorrow for NonSync<UniqueViewMut<'_, T>> {
    type Borrow = NonSync<UniqueViewMutBorrower<T>>;
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueViewMutBorrower<T>> {
    type View = NonSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
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
impl<T: 'static> IntoBorrow for NonSendSync<UniqueViewMut<'_, T>> {
    type Borrow = NonSendSync<UniqueViewMutBorrower<T>>;
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueViewMutBorrower<T>> {
    type View = NonSendSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        let view = all_storages.custom_storage_mut()?;

        let (unique, borrow) = unsafe { RefMut::destructure(view) };

        Ok(NonSendSync(UniqueViewMut {
            unique,
            _borrow: Some(borrow),
            _all_borrow: all_borrow,
        }))
    }
}

impl<T: IntoBorrow> IntoBorrow for Option<T> {
    type Borrow = Option<T::Borrow>;
}

impl<'a, T: Borrow<'a>> Borrow<'a> for Option<T> {
    type View = Option<T::View>;

    #[inline]
    fn borrow(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::View, error::GetStorage> {
        Ok(T::borrow(all_storages, all_borrow).ok())
    }
}

macro_rules! impl_all_storages_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: IntoBorrow),+> IntoBorrow for ($($type,)+) {
            type Borrow = ($($type::Borrow,)+);
        }

        impl<'a, $($type: Borrow<'a>),+> Borrow<'a> for ($($type,)+) {
            type View = ($($type::View,)+);

            #[inline]
            fn borrow(
                all_storages: &'a AllStorages, all_borrow: Option<SharedBorrow<'a>>
            ) -> Result<Self::View, error::GetStorage> {
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
