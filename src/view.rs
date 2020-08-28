use crate::atomic_refcell::{AtomicRefCell, Borrow};
use crate::atomic_refcell::{Ref, RefMut};
use crate::error;
use crate::sparse_set::{SparseSet, Window};
use crate::{AllStorages, Entities};
use core::convert::TryFrom;
use core::ops::{Deref, DerefMut};

struct AllStoragesView<'a>(Ref<'a, AllStorages>);

impl<'a> TryFrom<&'a AtomicRefCell<AllStorages>> for AllStoragesView<'a> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AtomicRefCell<AllStorages>) -> Result<Self, Self::Error> {
        Ok(AllStoragesView(
            all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        ))
    }
}

impl Deref for AllStoragesView<'_> {
    type Target = AllStorages;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Exclusive view over `AllStorages`.
pub struct AllStoragesViewMut<'a>(RefMut<'a, AllStorages>);

impl<'a> TryFrom<&'a AtomicRefCell<AllStorages>> for AllStoragesViewMut<'a> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AtomicRefCell<AllStorages>) -> Result<Self, Self::Error> {
        Ok(AllStoragesViewMut(
            all_storages
                .try_borrow_mut()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        ))
    }
}

impl Deref for AllStoragesViewMut<'_> {
    type Target = AllStorages;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllStoragesViewMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Shared view over `Entities` storage.
#[derive(Clone)]
pub struct EntitiesView<'a> {
    entities: Ref<'a, Entities>,
    _all_borrow: Borrow<'a>,
}

impl<'a> TryFrom<Ref<'a, AllStorages>> for EntitiesView<'a> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and entities are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        Ok(EntitiesView {
            entities: all_storages
                .entities()
                .map_err(error::GetStorage::Entities)?,
            _all_borrow: all_borrow,
        })
    }
}

impl<'a> TryFrom<&'a AllStorages> for EntitiesView<'a> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AllStorages) -> Result<Self, Self::Error> {
        Ok(EntitiesView {
            entities: all_storages
                .entities()
                .map_err(error::GetStorage::Entities)?,
            _all_borrow: Borrow::None,
        })
    }
}

impl Deref for EntitiesView<'_> {
    type Target = Entities;
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

/// Exclusive view over `Entities` storage.
pub struct EntitiesViewMut<'a> {
    entities: RefMut<'a, Entities>,
    _all_borrow: Borrow<'a>,
}

impl<'a> TryFrom<Ref<'a, AllStorages>> for EntitiesViewMut<'a> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and entities are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        Ok(EntitiesViewMut {
            entities: all_storages
                .entities_mut()
                .map_err(error::GetStorage::Entities)?,
            _all_borrow: all_borrow,
        })
    }
}

impl<'a> TryFrom<&'a AllStorages> for EntitiesViewMut<'a> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AllStorages) -> Result<Self, Self::Error> {
        Ok(EntitiesViewMut {
            entities: all_storages
                .entities_mut()
                .map_err(error::GetStorage::Entities)?,
            _all_borrow: Borrow::None,
        })
    }
}

impl Deref for EntitiesViewMut<'_> {
    type Target = Entities;
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

impl DerefMut for EntitiesViewMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entities
    }
}

/// Shared view over a component storage.
#[derive(Clone)]
pub struct View<'a, T> {
    window: Window<'a, T>,
    _borrow: Borrow<'a>,
    _all_borrow: Borrow<'a>,
}

impl<'a, T: 'static + Send + Sync> TryFrom<Ref<'a, AllStorages>> for View<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and borrow are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) = unsafe { Ref::destructure(all_storages.sparse_set::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
        })
    }
}

impl<'a, T: 'static + Send + Sync> TryFrom<&'a AllStorages> for View<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AllStorages) -> Result<Self, Self::Error> {
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) = unsafe { Ref::destructure(all_storages.sparse_set::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: Borrow::None,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> View<'a, T> {
    pub(crate) fn try_from_non_send(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        // SAFE all_storages and borrow are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) =
            unsafe { Ref::destructure(all_storages.sparse_set_non_send::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
        })
    }
    pub(crate) fn try_storage_from_non_send(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) =
            unsafe { Ref::destructure(all_storages.sparse_set_non_send::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: Borrow::None,
        })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> View<'a, T> {
    pub(crate) fn try_from_non_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        // SAFE all_storages and borrow are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) =
            unsafe { Ref::destructure(all_storages.sparse_set_non_sync::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
        })
    }
    pub(crate) fn try_storage_from_non_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) =
            unsafe { Ref::destructure(all_storages.sparse_set_non_sync::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: Borrow::None,
        })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> View<'a, T> {
    pub(crate) fn try_from_non_send_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        // SAFE all_storages and borrow are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) =
            unsafe { Ref::destructure(all_storages.sparse_set_non_send_sync::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
        })
    }
    pub(crate) fn try_storage_from_non_send_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        // SAFE window is dropped before borrow
        let (sparse_set, borrow) =
            unsafe { Ref::destructure(all_storages.sparse_set_non_send_sync::<T>()?) };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: Borrow::None,
        })
    }
}

impl<'a, T> Deref for View<'a, T> {
    type Target = Window<'a, T>;
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl<'a, T> AsRef<Window<'a, T>> for View<'a, T> {
    fn as_ref(&self) -> &Window<'a, T> {
        &self.window
    }
}

/// Exclusive view over a component storage.
pub struct ViewMut<'a, T> {
    sparse_set: RefMut<'a, SparseSet<T>>,
    _all_borrow: Borrow<'a>,
}

impl<'a, T: 'static + Send + Sync> TryFrom<Ref<'a, AllStorages>> for ViewMut<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and sprase_set are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_mut::<T>()?,
            _all_borrow: all_borrow,
        })
    }
}

impl<'a, T: 'static + Send + Sync> TryFrom<&'a AllStorages> for ViewMut<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AllStorages) -> Result<Self, Self::Error> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_mut::<T>()?,
            _all_borrow: Borrow::None,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> ViewMut<'a, T> {
    pub(crate) fn try_from_non_send(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        // SAFE all_storages and sprase_set are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_mut::<T>()?,
            _all_borrow: all_borrow,
        })
    }
    pub(crate) fn try_storage_from_non_send(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_mut::<T>()?,
            _all_borrow: Borrow::None,
        })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> ViewMut<'a, T> {
    pub(crate) fn try_from_non_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        // SAFE all_storages and sprase_set are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_sync_mut::<T>()?,
            _all_borrow: all_borrow,
        })
    }
    pub(crate) fn try_storage_from_non_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_sync_mut::<T>()?,
            _all_borrow: Borrow::None,
        })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> ViewMut<'a, T> {
    pub(crate) fn try_from_non_send_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        // SAFE all_storages and sprase_set are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_sync_mut::<T>()?,
            _all_borrow: all_borrow,
        })
    }
    pub(crate) fn try_storage_from_non_send_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_sync_mut::<T>()?,
            _all_borrow: Borrow::None,
        })
    }
}

impl<T> Deref for ViewMut<'_, T> {
    type Target = SparseSet<T>;
    fn deref(&self) -> &Self::Target {
        &self.sparse_set
    }
}

impl<T> DerefMut for ViewMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sparse_set
    }
}

impl<'a, T> AsRef<SparseSet<T>> for ViewMut<'a, T> {
    fn as_ref(&self) -> &SparseSet<T> {
        &self.sparse_set
    }
}

impl<'a, T> AsMut<SparseSet<T>> for ViewMut<'a, T> {
    fn as_mut(&mut self) -> &mut SparseSet<T> {
        &mut self.sparse_set
    }
}

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T> {
    unique: Ref<'a, T>,
    _all_borrow: Borrow<'a>,
}

impl<'a, T: 'static> TryFrom<Ref<'a, AllStorages>> for UniqueView<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and unique are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };

        Ok(UniqueView {
            unique: all_storages.unique::<T>()?,
            _all_borrow: all_borrow,
        })
    }
}

impl<'a, T: 'static> TryFrom<&'a AllStorages> for UniqueView<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AllStorages) -> Result<Self, Self::Error> {
        Ok(UniqueView {
            unique: all_storages.unique::<T>()?,
            _all_borrow: Borrow::None,
        })
    }
}

impl<T> Deref for UniqueView<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.unique
    }
}

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T> {
    unique: RefMut<'a, T>,
    _all_borrow: Borrow<'a>,
}

impl<'a, T: 'static> TryFrom<Ref<'a, AllStorages>> for UniqueViewMut<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and unique are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };

        Ok(UniqueViewMut {
            unique: all_storages.unique_mut::<T>()?,
            _all_borrow: all_borrow,
        })
    }
}

impl<'a, T: 'static> TryFrom<&'a AllStorages> for UniqueViewMut<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: &'a AllStorages) -> Result<Self, Self::Error> {
        Ok(UniqueViewMut {
            unique: all_storages.unique_mut::<T>()?,
            _all_borrow: Borrow::None,
        })
    }
}

impl<T> Deref for UniqueViewMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.unique
    }
}

impl<T> DerefMut for UniqueViewMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unique
    }
}

/// Shared view over the thread_pool.
#[cfg(feature = "parallel")]
#[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
#[derive(Clone)]
pub struct ThreadPoolView<'a>(pub(crate) &'a rayon::ThreadPool);

#[cfg(feature = "parallel")]
impl AsRef<rayon::ThreadPool> for ThreadPoolView<'_> {
    fn as_ref(&self) -> &rayon::ThreadPool {
        &self.0
    }
}

#[cfg(feature = "parallel")]
impl Deref for ThreadPoolView<'_> {
    type Target = rayon::ThreadPool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
