use crate::error;
use crate::scheduler::into_workload_run_if::IntoWorkloadRunIf;
use crate::scheduler::into_workload_system::Nothing;
use crate::scheduler::workload::Workload;
use crate::storage::StorageId;
use crate::AllStoragesViewMut;
use crate::AsLabel;
use crate::Component;
use crate::SparseSet;
use crate::Unique;
use crate::UniqueStorage;

/// Converts to a collection of systems.
pub trait IntoNestedWorkload<B, R> {
    /// Converts to a collection of systems.
    fn into_nested_workload(self) -> Workload;
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

impl<F> IntoNestedWorkload<Nothing, Workload> for F
where
    F: 'static + Send + Sync + Fn() -> Workload,
{
    fn into_nested_workload(self) -> Workload {
        (self)()
    }
    #[track_caller]
    fn run_if<RunB, Run: IntoWorkloadRunIf<RunB>>(self, run_if: Run) -> Workload {
        (self)().run_if(run_if)
    }
    #[track_caller]
    fn skip_if<RunB, Run: IntoWorkloadRunIf<RunB>>(self, should_skip: Run) -> Workload {
        (self)().skip_if(should_skip)
    }
    #[track_caller]
    fn before_all<T>(self, other: impl AsLabel<T>) -> Workload {
        (self)().before_all(other)
    }
    #[track_caller]
    fn after_all<T>(self, other: impl AsLabel<T>) -> Workload {
        (self)().after_all(other)
    }
    #[track_caller]
    fn rename<T>(self, name: impl AsLabel<T>) -> Workload {
        (self)().rename(name)
    }
    #[track_caller]
    fn tag<T>(self, tag: impl AsLabel<T>) -> Workload {
        (self)().tag(tag)
    }
}
