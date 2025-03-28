use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ARef, SharedBorrow};
use crate::borrow::{Borrow, BorrowInfo, WorldBorrow};
use crate::component::Unique;
use crate::error;
use crate::scheduler::info::TypeInfo;
use crate::tracking::TrackingTimestamp;
use crate::views::UniqueViewMut;
use crate::world::World;
use alloc::vec::Vec;
use core::cell::OnceCell;

/// Exclusive view over a unique component storage.
///
/// The component can be initialized with this view.
pub struct UniqueOrInitViewMut<'v, T: Unique + Default> {
    cell: OnceCell<UniqueViewMut<'v, T>>,
    all_storages: &'v AllStorages,
    all_borrow: SharedBorrow<'v>,
    last_run: Option<TrackingTimestamp>,
    current: TrackingTimestamp,
}

impl<'v, T: Unique + Default + Send + Sync> UniqueOrInitViewMut<'v, T> {
    /// Gets a reference to the inner [`UniqueViewMut`].
    ///
    /// Returns `None` if this view doesn't contains the inner [`UniqueViewMut`].
    pub fn get(&self) -> Option<&UniqueViewMut<'v, T>> {
        self.cell.get()
    }

    /// Adds the unique component to the `World`.
    ///
    /// Returns `true` if the component was inserted.
    ///
    /// ### Borrows
    ///
    /// - `Unique<T>` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `Unique<T>` storage borrow failed (it can be initialized and borrowed elsewhere).
    pub fn get_mut(&mut self) -> Option<&mut UniqueViewMut<'v, T>> {
        self.cell.get_mut()
    }

    /// Fetches the unique component from the `World`.
    ///
    /// Returns `true` if the component was fetched.
    ///
    /// ### Borrows
    ///
    /// - `Unique<T>` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `Unique<T>` storage borrow failed (it can be initialized and borrowed elsewhere).
    pub fn set(&self, unique: T) -> Result<bool, error::GetStorage> {
        if self.cell.get().is_some() {
            return Ok(false);
        }

        self.all_storages.add_unique(unique);

        self.cell
            .set(UniqueViewMut::borrow(
                self.all_storages,
                Some(self.all_borrow.clone()),
                self.last_run,
                self.current,
            )?)
            .unwrap_or_else(|_| unreachable!("Cell is expected to be empty"));

        Ok(true)
    }

    /// Fetches the unique component from the `World`.
    ///
    /// Returns `true` if the component was fetched.
    ///
    /// ### Borrows
    ///
    /// - `Unique<T>` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `Unique<T>` storage borrow failed (it can be initialized and borrowed elsewhere).
    pub fn fetch(&self) -> Result<bool, error::GetStorage> {
        if self.cell.get().is_some() {
            return Ok(false);
        }

        self.cell
            .set(UniqueViewMut::borrow(
                self.all_storages,
                Some(self.all_borrow.clone()),
                self.last_run,
                self.current,
            )?)
            .unwrap_or_else(|_| unreachable!("Cell is expected to be empty"));

        Ok(true)
    }

    /// Gets the unique component from the `World`.\
    /// Adds it to the `World` if not present.
    ///
    /// ### Borrows
    ///
    /// - `Unique<T>` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `Unique<T>` storage borrow failed (it can be initialized and borrowed elsewhere).
    pub fn get_or_init(
        &self,
        f: impl FnOnce() -> T,
    ) -> Result<&UniqueViewMut<'v, T>, error::GetStorage> {
        if let Some(view) = self.cell.get() {
            return Ok(view);
        }

        let view = match UniqueViewMut::borrow(
            self.all_storages,
            Some(self.all_borrow.clone()),
            self.last_run,
            self.current,
        ) {
            Ok(view) => view,
            Err(error::GetStorage::MissingStorage { .. }) => {
                self.all_storages.add_unique(f());

                UniqueViewMut::borrow(
                    self.all_storages,
                    Some(self.all_borrow.clone()),
                    self.last_run,
                    self.current,
                )?
            }
            Err(err) => return Err(err),
        };

        self.cell
            .set(view)
            .unwrap_or_else(|_| unreachable!("Cell is expected to be empty"));

        Ok(self
            .cell
            .get()
            .unwrap_or_else(|| unreachable!("Cell is expected to be initialized")))
    }
}

impl<'v, T: Unique + Default + Send + Sync> WorldBorrow for UniqueOrInitViewMut<'v, T> {
    type WorldView<'a> = UniqueOrInitViewMut<'a, T>;

    fn world_borrow(
        world: &World,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::WorldView<'_>, error::GetStorage> {
        let all_storages = world
            .all_storages()
            .map_err(error::GetStorage::AllStoragesBorrow)?;

        let (all_storages, all_borrow) = unsafe { ARef::destructure(all_storages) };
        let cell = OnceCell::new();

        match UniqueViewMut::borrow(all_storages, Some(all_borrow.clone()), last_run, current) {
            Ok(view) => cell
                .set(view)
                .unwrap_or_else(|_| unreachable!("Cell is expected to be empty")),
            Err(error::GetStorage::MissingStorage { .. }) => {}
            Err(err) => return Err(err),
        };

        Ok(UniqueOrInitViewMut {
            cell,
            all_storages,
            all_borrow,
            last_run,
            current,
        })
    }
}

unsafe impl<'v, T: Unique + Default + Send + Sync> BorrowInfo for UniqueOrInitViewMut<'v, T> {
    fn borrow_info(info: &mut Vec<TypeInfo>) {
        UniqueViewMut::<T>::borrow_info(info);
    }

    fn enable_tracking(
        enable_tracking_fn: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    ) {
        UniqueViewMut::<T>::enable_tracking(enable_tracking_fn);
    }
}
