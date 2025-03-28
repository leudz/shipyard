use crate::component::{Component, Unique};
use crate::error;
use crate::scheduler::into_workload_run_if::IntoWorkloadRunIf;
use crate::scheduler::label::WorkloadLabel;
use crate::scheduler::workload::Workload;
use crate::scheduler::AsLabel;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::type_id::TypeId;
use crate::unique::UniqueStorage;
use crate::views::AllStoragesViewMut;
use crate::world::World;
use alloc::boxed::Box;
use core::any::type_name;
use core::ops::Not;

/// Modifies a workload.
pub trait WorkloadModificator {
    /// Only run the workload if the function evaluates to `true`.
    fn run_if<RunB, Run: IntoWorkloadRunIf<RunB>>(self, run_if: Run) -> Workload;
    /// Only run the workload if the `T` storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn run_if_storage_empty<T: Component>(self) -> Workload
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.run_if_storage_empty_by_id(storage_id)
    }
    /// Only run the workload if the `T` unique storage is not present in the `World`.
    fn run_if_missing_unique<T: Unique>(self) -> Workload
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.run_if_storage_empty_by_id(storage_id)
    }
    /// Only run the workload if the storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn run_if_storage_empty_by_id(self, storage_id: StorageId) -> Workload
    where
        Self: Sized,
    {
        use crate::all_storages::CustomStorageAccess;

        let run_if = move |all_storages: AllStoragesViewMut<'_>| match all_storages
            .custom_storage_by_id(storage_id)
        {
            Ok(storage) => storage.is_empty(),
            Err(error::GetStorage::MissingStorage { .. }) => true,
            Err(_) => false,
        };

        self.run_if(run_if)
    }
    /// Do not run the workload if the function evaluates to `true`.
    fn skip_if<RunB, Run: IntoWorkloadRunIf<RunB>>(self, run_if: Run) -> Workload;
    /// Do not run the workload if the `T` storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn skip_if_storage_empty<T: Component>(self) -> Workload
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    /// Do not run the workload if the `T` unique storage is not present in the `World`.
    fn skip_if_missing_unique<T: Unique>(self) -> Workload
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    /// Do not run the workload if the storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn skip_if_storage_empty_by_id(self, storage_id: StorageId) -> Workload
    where
        Self: Sized,
    {
        use crate::all_storages::CustomStorageAccess;

        let should_skip = move |all_storages: AllStoragesViewMut<'_>| match all_storages
            .custom_storage_by_id(storage_id)
        {
            Ok(storage) => storage.is_empty(),
            Err(error::GetStorage::MissingStorage { .. }) => true,
            Err(_) => false,
        };

        self.skip_if(should_skip)
    }
    /// When building a workload, all systems within this workload will be placed before all invocation of the other system or workload.
    fn before_all<T>(self, other: impl AsLabel<T>) -> Workload;
    /// When building a workload, all systems within this workload will be placed after all invocation of the other system or workload.
    fn after_all<T>(self, other: impl AsLabel<T>) -> Workload;
    /// Changes the name of this workload.
    fn rename<T>(self, name: impl AsLabel<T>) -> Workload;
    /// Adds a tag to this workload. Tags can be used to control system ordering when running workloads.
    fn tag<T>(self, tag: impl AsLabel<T>) -> Workload;
}

impl WorkloadModificator for Workload {
    #[track_caller]
    fn run_if<RunB, Run: IntoWorkloadRunIf<RunB>>(mut self, run_if: Run) -> Workload {
        let run_if = run_if.into_workload_run_if().unwrap();

        self.run_if = if let Some(prev_run_if) = self.run_if.take() {
            Some(Box::new(move |world: &World| {
                Ok(prev_run_if.run(world)? && run_if.run(world)?)
            }))
        } else {
            Some(run_if)
        };

        self
    }
    fn run_if_storage_empty<T: Component>(self) -> Workload {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.run_if_storage_empty_by_id(storage_id)
    }
    fn run_if_missing_unique<T: Unique>(self) -> Workload {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.run_if_storage_empty_by_id(storage_id)
    }
    fn run_if_storage_empty_by_id(self, storage_id: StorageId) -> Workload {
        use crate::all_storages::CustomStorageAccess;

        let run_if = move |all_storages: AllStoragesViewMut<'_>| match all_storages
            .custom_storage_by_id(storage_id)
        {
            Ok(storage) => storage.is_empty(),
            Err(error::GetStorage::MissingStorage { .. }) => true,
            Err(_) => false,
        };

        self.run_if(run_if)
    }
    fn skip_if<RunB, Run: IntoWorkloadRunIf<RunB>>(mut self, should_skip: Run) -> Self {
        let mut should_skip = should_skip.into_workload_run_if().unwrap();

        should_skip = Box::new(move |world: &World| should_skip.run(world).map(Not::not));

        self.run_if = if let Some(prev_run_if) = self.run_if.take() {
            Some(Box::new(move |world: &World| {
                Ok(prev_run_if.run(world)? && should_skip.run(world)?)
            }))
        } else {
            Some(should_skip)
        };

        self
    }
    fn skip_if_storage_empty<T: Component>(self) -> Self {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    fn skip_if_missing_unique<T: Unique>(self) -> Self {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    fn skip_if_storage_empty_by_id(self, storage_id: StorageId) -> Self {
        use crate::all_storages::CustomStorageAccess;

        let should_skip = move |all_storages: AllStoragesViewMut<'_>| match all_storages
            .custom_storage_by_id(storage_id)
        {
            Ok(storage) => storage.is_empty(),
            Err(error::GetStorage::MissingStorage { .. }) => true,
            Err(_) => false,
        };

        self.skip_if(should_skip)
    }
    fn before_all<T>(mut self, other: impl AsLabel<T>) -> Workload {
        self.before_all.add(other);

        self
    }
    fn after_all<T>(mut self, other: impl AsLabel<T>) -> Workload {
        self.after_all.add(other);

        self
    }
    fn rename<T>(mut self, name: impl AsLabel<T>) -> Workload {
        self.name = name.as_label();
        self.overwritten_name = true;
        self.tags.push(self.name.clone());

        self
    }
    fn tag<T>(mut self, tag: impl AsLabel<T>) -> Workload {
        self.tags.push(tag.as_label());

        self
    }
}

impl<W> WorkloadModificator for W
where
    W: 'static + Send + Sync + Fn() -> Workload,
{
    fn run_if<RunB, Run: IntoWorkloadRunIf<RunB>>(self, run_if: Run) -> Workload {
        let mut workload = (self)();

        let label = WorkloadLabel {
            type_id: TypeId::of::<W>(),
            name: type_name::<W>().as_label(),
        };

        workload = workload.tag(label.clone());
        workload.name = Box::new(label);

        workload.run_if(run_if)
    }
    fn skip_if<RunB, Run: IntoWorkloadRunIf<RunB>>(self, should_skip: Run) -> Workload {
        let mut workload = (self)();

        let label = WorkloadLabel {
            type_id: TypeId::of::<W>(),
            name: type_name::<W>().as_label(),
        };

        workload = workload.tag(label.clone());
        workload.name = Box::new(label);

        workload.skip_if(should_skip)
    }
    fn before_all<T>(self, other: impl AsLabel<T>) -> Workload {
        let mut workload = (self)();

        let label = WorkloadLabel {
            type_id: TypeId::of::<W>(),
            name: type_name::<W>().as_label(),
        };

        workload = workload.tag(label.clone());
        workload.name = Box::new(label);

        workload.before_all(other)
    }
    fn after_all<T>(self, other: impl AsLabel<T>) -> Workload {
        let mut workload = (self)();

        let label = WorkloadLabel {
            type_id: TypeId::of::<W>(),
            name: type_name::<W>().as_label(),
        };

        workload = workload.tag(label.clone());
        workload.name = Box::new(label);

        workload.after_all(other)
    }
    fn rename<T>(self, name: impl AsLabel<T>) -> Workload {
        let mut workload = (self)();

        let label = WorkloadLabel {
            type_id: TypeId::of::<W>(),
            name: type_name::<W>().as_label(),
        };

        workload = workload.tag(label.clone());
        workload.name = Box::new(label);

        workload.rename(name)
    }
    fn tag<T>(self, tag: impl AsLabel<T>) -> Workload {
        let mut workload = (self)();

        let label = WorkloadLabel {
            type_id: TypeId::of::<W>(),
            name: type_name::<W>().as_label(),
        };

        workload = workload.tag(label.clone());
        workload.name = Box::new(label);

        workload.tag(tag)
    }
}
