use crate::all_storages::AllStorages;
use crate::atomic_refcell::ARef;
use crate::borrow::{Borrow, WorldBorrow};
use crate::component::Unique;
use crate::info::TypeInfo;
use crate::tracking::TrackingTimestamp;
use crate::views::UniqueViewMut;
use crate::world::World;
use crate::{error, BorrowInfo};
use std::ops::{Deref, DerefMut};

/// Exclusive view over a unique component storage.
///
/// If the component is not already present, its default value will be inserted.
pub struct UniqueOrDefaultViewMut<'v, T: Unique + Default>(UniqueViewMut<'v, T>);

impl<'v, T: Unique + Default> Deref for UniqueOrDefaultViewMut<'v, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'v, T: Unique + Default> DerefMut for UniqueOrDefaultViewMut<'v, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'v, T: Unique + Default + Send + Sync> WorldBorrow for UniqueOrDefaultViewMut<'v, T> {
    type WorldView<'a> = UniqueOrDefaultViewMut<'a, T>;

    fn world_borrow(
        world: &World,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::WorldView<'_>, error::GetStorage> {
        let all_storages = world
            .all_storages()
            .map_err(error::GetStorage::AllStoragesBorrow)?;

        match all_storages.borrow::<UniqueViewMut<'_, T>>() {
            Ok(_) => {}
            Err(error::GetStorage::MissingStorage { .. }) => all_storages.add_unique(T::default()),
            Err(err) => return Err(err),
        };

        let (all_storages, all_borrow) = unsafe { ARef::destructure(all_storages) };

        let view = UniqueViewMut::borrow(all_storages, Some(all_borrow), last_run, current)?;

        Ok(UniqueOrDefaultViewMut(view))
    }
}

unsafe impl<'v, T: Unique + Default + Send + Sync> BorrowInfo for UniqueOrDefaultViewMut<'v, T> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        UniqueViewMut::<T>::borrow_info(info);
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        UniqueViewMut::<T>::enable_tracking(enable_tracking_fn);
    }
}
