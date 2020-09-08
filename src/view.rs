use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut, SharedBorrow};
use crate::error;
use crate::sparse_set::{SparseSet, Window};
use crate::{AllStorages, Entities};
use core::ops::{Deref, DerefMut};

/// Exclusive view over `AllStorages`.
pub struct AllStoragesViewMut<'a>(RefMut<'a, AllStorages>);

impl<'a> AllStoragesViewMut<'a> {
    #[inline]
    pub(crate) fn new(
        all_storages: &'a AtomicRefCell<AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        Ok(AllStoragesViewMut(
            all_storages
                .try_borrow_mut()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        ))
    }
}

impl Deref for AllStoragesViewMut<'_> {
    type Target = AllStorages;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllStoragesViewMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<AllStorages> for AllStoragesViewMut<'_> {
    #[inline]
    fn as_ref(&self) -> &AllStorages {
        &self.0
    }
}

impl AsMut<AllStorages> for AllStoragesViewMut<'_> {
    #[inline]
    fn as_mut(&mut self) -> &mut AllStorages {
        &mut self.0
    }
}

/// Shared view over `Entities` storage.
#[derive(Clone)]
pub struct EntitiesView<'a> {
    entities: Ref<'a, Entities>,
    all_borrow: Option<SharedBorrow<'a>>,
}

impl<'a> EntitiesView<'a> {
    #[inline]
    pub(crate) fn from_ref(all_storages: Ref<'a, AllStorages>) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(EntitiesView {
            entities: all_storages
                .entities()
                .map_err(error::GetStorage::Entities)?,
            all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        Ok(EntitiesView {
            entities: all_storages
                .entities()
                .map_err(error::GetStorage::Entities)?,
            all_borrow: None,
        })
    }
}

impl Deref for EntitiesView<'_> {
    type Target = Entities;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

/// Exclusive view over `Entities` storage.
pub struct EntitiesViewMut<'a> {
    entities: RefMut<'a, Entities>,
    _all_borrow: Option<SharedBorrow<'a>>,
}

impl<'a> EntitiesViewMut<'a> {
    #[inline]
    pub(crate) fn from_ref(all_storages: Ref<'a, AllStorages>) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(EntitiesViewMut {
            entities: all_storages
                .entities_mut()
                .map_err(error::GetStorage::Entities)?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        Ok(EntitiesViewMut {
            entities: all_storages
                .entities_mut()
                .map_err(error::GetStorage::Entities)?,
            _all_borrow: None,
        })
    }
}

impl Deref for EntitiesViewMut<'_> {
    type Target = Entities;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

impl DerefMut for EntitiesViewMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entities
    }
}

/// Shared view over a component storage.
#[derive(Clone)]
pub struct View<'a, T> {
    window: Window<'a, T>,
    borrow: SharedBorrow<'a>,
    all_borrow: Option<SharedBorrow<'a>>,
}

impl<'a, T: 'static + Send + Sync> View<'a, T> {
    #[inline]
    pub(crate) fn from_ref(all_storages: Ref<'a, AllStorages>) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };
        let (sparse_set, borrow) = unsafe { all_storages.sparse_set::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        let (sparse_set, borrow) = unsafe { all_storages.sparse_set::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: None,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> View<'a, T> {
    #[inline]
    pub(crate) fn from_ref_non_send(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };
        let (sparse_set, borrow) =
            unsafe { all_storages.sparse_set_non_send::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference_non_send(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        let (sparse_set, borrow) =
            unsafe { all_storages.sparse_set_non_send::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: None,
        })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> View<'a, T> {
    #[inline]
    pub(crate) fn from_ref_non_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };
        let (sparse_set, borrow) =
            unsafe { all_storages.sparse_set_non_sync::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference_non_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        let (sparse_set, borrow) =
            unsafe { all_storages.sparse_set_non_sync::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: None,
        })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> View<'a, T> {
    #[inline]
    pub(crate) fn from_ref_non_send_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };
        let (sparse_set, borrow) =
            unsafe { all_storages.sparse_set_non_send_sync::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference_non_send_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        let (sparse_set, borrow) =
            unsafe { all_storages.sparse_set_non_send_sync::<T>()?.destructure() };

        Ok(View {
            window: sparse_set.window(),
            borrow,
            all_borrow: None,
        })
    }
}

impl<'a, T> Deref for View<'a, T> {
    type Target = Window<'a, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl<'a, T> AsRef<Window<'a, T>> for View<'a, T> {
    #[inline]
    fn as_ref(&self) -> &Window<'a, T> {
        &self.window
    }
}

/// Exclusive view over a component storage.
pub struct ViewMut<'a, T> {
    sparse_set: RefMut<'a, SparseSet<T>>,
    _all_borrow: Option<SharedBorrow<'a>>,
}

impl<'a, T: 'static + Send + Sync> ViewMut<'a, T> {
    #[inline]
    pub(crate) fn from_ref(all_storages: Ref<'a, AllStorages>) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_mut::<T>()?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_mut::<T>()?,
            _all_borrow: None,
        })
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> ViewMut<'a, T> {
    #[inline]
    pub(crate) fn from_ref_non_send(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_mut::<T>()?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference_non_send(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_mut::<T>()?,
            _all_borrow: None,
        })
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> ViewMut<'a, T> {
    #[inline]
    pub(crate) fn from_ref_non_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_sync_mut::<T>()?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference_non_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_sync_mut::<T>()?,
            _all_borrow: None,
        })
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> ViewMut<'a, T> {
    #[inline]
    pub(crate) fn from_ref_non_send_sync(
        all_storages: Ref<'a, AllStorages>,
    ) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_sync_mut::<T>()?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference_non_send_sync(
        all_storages: &'a AllStorages,
    ) -> Result<Self, error::GetStorage> {
        Ok(ViewMut {
            sparse_set: all_storages.sparse_set_non_send_sync_mut::<T>()?,
            _all_borrow: None,
        })
    }
}

impl<T> Deref for ViewMut<'_, T> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sparse_set
    }
}

impl<T> DerefMut for ViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sparse_set
    }
}

impl<'a, T> AsRef<SparseSet<T>> for ViewMut<'a, T> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        &self.sparse_set
    }
}

impl<'a, T> AsMut<SparseSet<T>> for ViewMut<'a, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut SparseSet<T> {
        &mut self.sparse_set
    }
}

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T> {
    unique: Ref<'a, T>,
    _all_borrow: Option<SharedBorrow<'a>>,
}

impl<'a, T: 'static> UniqueView<'a, T> {
    #[inline]
    pub(crate) fn from_ref(all_storages: Ref<'a, AllStorages>) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(UniqueView {
            unique: all_storages.unique::<T>()?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        Ok(UniqueView {
            unique: all_storages.unique::<T>()?,
            _all_borrow: None,
        })
    }
}

impl<T> Deref for UniqueView<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique
    }
}

impl<T> AsRef<T> for UniqueView<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique
    }
}

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T> {
    unique: RefMut<'a, T>,
    _all_borrow: Option<SharedBorrow<'a>>,
}

impl<'a, T: 'static> UniqueViewMut<'a, T> {
    #[inline]
    pub(crate) fn from_ref(all_storages: Ref<'a, AllStorages>) -> Result<Self, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe { all_storages.destructure() };

        Ok(UniqueViewMut {
            unique: all_storages.unique_mut::<T>()?,
            _all_borrow: Some(all_borrow),
        })
    }
    #[inline]
    pub(crate) fn from_reference(all_storages: &'a AllStorages) -> Result<Self, error::GetStorage> {
        Ok(UniqueViewMut {
            unique: all_storages.unique_mut::<T>()?,
            _all_borrow: None,
        })
    }
}

impl<T> Deref for UniqueViewMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique
    }
}

impl<T> DerefMut for UniqueViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unique
    }
}

impl<T> AsRef<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique
    }
}

impl<T> AsMut<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.unique
    }
}
