use crate::all_storages::AllStorages;
use crate::atomic_refcell::SharedBorrow;
use crate::borrow::Borrow;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Unique;
use crate::error;
use crate::views::{UniqueView, UniqueViewMut};

/// Trait used as bound for [`World::get_unique`] and [`AllStorages::get_unique`].
///
/// [`World::get_unique`]: crate::World::get_unique
/// [`AllStorages::get_unique`]: crate::AllStorages::get_unique
pub trait GetUnique {
    #[allow(missing_docs)]
    type Out<'a>;

    #[allow(missing_docs)]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage>;
}

impl<T: Unique + Send + Sync> GetUnique for &'_ T {
    type Out<'a> = UniqueView<'a, T>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <UniqueView<'a, T> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Sync> GetUnique for NonSend<&'_ T> {
    type Out<'a> = NonSend<UniqueView<'a, T>>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <NonSend<UniqueView<'a, T>> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Send> GetUnique for NonSync<&'_ T> {
    type Out<'a> = NonSync<UniqueView<'a, T>>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <NonSync<UniqueView<'a, T>> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique> GetUnique for NonSendSync<&'_ T> {
    type Out<'a> = NonSendSync<UniqueView<'a, T>>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <NonSendSync<UniqueView<'a, T>> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

impl<T: Unique + Send + Sync> GetUnique for &'_ mut T {
    type Out<'a> = UniqueViewMut<'a, T>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <UniqueViewMut<'a, T> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Sync> GetUnique for NonSend<&'_ mut T> {
    type Out<'a> = NonSend<UniqueViewMut<'a, T>>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <NonSend<UniqueViewMut<'a, T>> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique + Send> GetUnique for NonSync<&'_ mut T> {
    type Out<'a> = NonSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <NonSync<UniqueViewMut<'a, T>> as Borrow>::borrow(all_storages, all_borrow, None, current)
    }
}

#[cfg(feature = "thread_local")]
impl<T: Unique> GetUnique for NonSendSync<&'_ mut T> {
    type Out<'a> = NonSendSync<UniqueViewMut<'a, T>>;

    #[inline]
    fn get_unique<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
    ) -> Result<Self::Out<'a>, error::GetStorage> {
        let current = all_storages.get_current();

        <NonSendSync<UniqueViewMut<'a, T>> as Borrow>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )
    }
}
