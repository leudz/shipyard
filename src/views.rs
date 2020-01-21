use crate::atomic_refcell::{AtomicRefCell, Borrow};
use crate::atomic_refcell::{Ref, RefMut};
use crate::error;
use crate::sparse_set::{SparseSet, Window};
use crate::{AllStorages, Entities};
use core::any::type_name;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};

pub struct AllStoragesView<'a>(Ref<'a, AllStorages>);

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

impl Deref for EntitiesView<'_> {
    type Target = Entities;
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

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
        let (sparse_set, borrow) = unsafe {
            Ref::destructure(
                all_storages
                    .get::<T>()
                    .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            )
        };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
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
        let (sparse_set, borrow) = unsafe {
            Ref::destructure(
                all_storages
                    .get_non_send::<T>()
                    .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            )
        };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
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
        let (sparse_set, borrow) = unsafe {
            Ref::destructure(
                all_storages
                    .get_non_sync::<T>()
                    .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            )
        };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
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
        let (sparse_set, borrow) = unsafe {
            Ref::destructure(
                all_storages
                    .get_non_send_sync::<T>()
                    .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            )
        };
        Ok(View {
            window: sparse_set.window(),
            _borrow: borrow,
            _all_borrow: all_borrow,
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
            sparse_set: all_storages
                .get_mut::<T>()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            _all_borrow: all_borrow,
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
            sparse_set: all_storages
                .get_non_send_mut::<T>()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            _all_borrow: all_borrow,
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
            sparse_set: all_storages
                .get_non_sync_mut::<T>()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            _all_borrow: all_borrow,
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
            sparse_set: all_storages
                .get_non_send_sync_mut::<T>()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            _all_borrow: all_borrow,
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

pub struct UniqueView<'a, T> {
    unique: Ref<'a, T>,
    _all_borrow: Borrow<'a>,
}

impl<'a, T: 'static + Send + Sync> TryFrom<Ref<'a, AllStorages>> for UniqueView<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and unique are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        let unique = Ref::try_map(
            all_storages
                .get::<T>()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            |sparse_set| {
                if sparse_set.is_unique() {
                    // SAFE unique storage have data there
                    Ok(unsafe { sparse_set.data.get_unchecked(0) })
                } else {
                    Err(error::GetStorage::NonUnique((
                        type_name::<T>(),
                        error::Borrow::Shared,
                    )))
                }
            },
        )?;
        Ok(UniqueView {
            unique,
            _all_borrow: all_borrow,
        })
    }
}

impl<T> Deref for UniqueView<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.unique
    }
}

pub struct UniqueViewMut<'a, T> {
    unique: RefMut<'a, T>,
    _all_borrow: Borrow<'a>,
}

impl<'a, T: 'static + Send + Sync> TryFrom<Ref<'a, AllStorages>> for UniqueViewMut<'a, T> {
    type Error = error::GetStorage;
    fn try_from(all_storages: Ref<'a, AllStorages>) -> Result<Self, Self::Error> {
        // SAFE all_storages and unique are dropped before all_borrow
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };
        let unique = RefMut::try_map(
            all_storages
                .get_mut::<T>()
                .map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))?,
            |sparse_set| {
                if sparse_set.is_unique() {
                    // SAFE unique storage have data there
                    Ok(unsafe { sparse_set.data.get_unchecked_mut(0) })
                } else {
                    Err(error::GetStorage::NonUnique((
                        type_name::<T>(),
                        error::Borrow::Unique,
                    )))
                }
            },
        )?;
        Ok(UniqueViewMut {
            unique,
            _all_borrow: all_borrow,
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
