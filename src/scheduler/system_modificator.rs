use crate::borrow::{BorrowInfo, WorldBorrow};
use crate::component::{Component, Unique};
use crate::error;
use crate::scheduler::into_workload_run_if::IntoRunIf;
use crate::scheduler::{AsLabel, IntoWorkloadSystem, WorkloadSystem};
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::unique::UniqueStorage;
use crate::views::AllStoragesViewMut;
use alloc::boxed::Box;
use core::ops::Not;

/// Modifies a system.
pub trait SystemModificator<B, R> {
    /// Only run the system if the function evaluates to `true`.
    fn run_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem;
    /// Only run the system if the `T` storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn run_if_storage_empty<T: Component>(self) -> WorkloadSystem
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.run_if_storage_empty_by_id(storage_id)
    }
    /// Only run the system if the `T` unique storage is not present in the `World`.
    fn run_if_missing_unique<T: Unique>(self) -> WorkloadSystem
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.run_if_storage_empty_by_id(storage_id)
    }
    /// Only run the system if the storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn run_if_storage_empty_by_id(self, storage_id: StorageId) -> WorkloadSystem
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
    /// Do not run the system if the function evaluates to `true`.
    fn skip_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem;
    /// Do not run the system if the `T` storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn skip_if_storage_empty<T: Component>(self) -> WorkloadSystem
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    /// Do not run the system if the `T` unique storage is not present in the `World`.
    fn skip_if_missing_unique<T: Unique>(self) -> WorkloadSystem
    where
        Self: Sized,
    {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    /// Do not run the system if the storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    fn skip_if_storage_empty_by_id(self, storage_id: StorageId) -> WorkloadSystem
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
    /// When building a workload, this system will be placed before all invocation of the other system or workload.
    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
    /// When building a workload, this system will be placed after all invocation of the other system or workload.
    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
    /// System name used in error and gui built for shipyard.  
    /// Defaults to the system function name.
    fn display_name<T>(self, name: impl AsLabel<T>) -> WorkloadSystem;
    /// Adds a tag to this system. Tags can be used to control system ordering when running workloads.
    fn tag<T>(self, tag: impl AsLabel<T>) -> WorkloadSystem;
    /// When building a workload, this system will assert that at least one of the other system is present in the workload.
    ///
    /// Does not change system ordering.
    fn require_in_workload<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
    /// When building a workload, this system will assert that at least one of the other system is present before itself in the workload.
    ///
    /// Does not change system ordering.
    fn require_before<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
    /// When building a workload, this system will assert that at least one of the other system is present after itself in the workload.
    ///
    /// Does not change system ordering.
    fn require_after<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
}

pub struct Nothing;

impl<F> SystemModificator<Nothing, ()> for F
where
    F: 'static + Send + Sync + Fn(),
{
    #[track_caller]
    fn run_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();
        let run_if = run_if.into_workload_run_if().unwrap();

        system.run_if = Some(run_if.system_fn);

        system
    }
    #[track_caller]
    fn skip_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
        let mut run_if = run_if.into_workload_run_if().unwrap();

        run_if.system_fn = Box::new(move |world| (run_if.system_fn)(world).map(Not::not));

        self.run_if(run_if)
    }
    #[track_caller]
    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.before_all.add(other);

        system
    }
    #[track_caller]
    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.after_all.add(other);

        system
    }
    #[track_caller]
    fn display_name<T>(self, name: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.display_name = name.as_label();

        system
    }
    #[track_caller]
    fn tag<T>(self, tag: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.tags.push(tag.as_label());

        system
    }
    #[track_caller]
    fn require_in_workload<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.require_in_workload.add(other);

        system
    }
    #[track_caller]
    fn require_before<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.require_before.add(other);

        system
    }
    #[track_caller]
    fn require_after<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let mut system = self.into_workload_system().unwrap();

        system.require_after.add(other);

        system
    }
}

impl SystemModificator<WorkloadSystem, ()> for WorkloadSystem {
    #[track_caller]
    fn run_if<RunB, Run: IntoRunIf<RunB>>(mut self, run_if: Run) -> WorkloadSystem {
        let run_if = run_if.into_workload_run_if().unwrap();

        self.run_if = if let Some(prev_run_if) = self.run_if {
            Some(Box::new(move |world| {
                Ok((prev_run_if)(world)? && (run_if.system_fn)(world)?)
            }))
        } else {
            Some(run_if.system_fn)
        };

        self
    }
    #[track_caller]
    fn skip_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
        let mut run_if = run_if.into_workload_run_if().unwrap();

        run_if.system_fn = Box::new(move |world| (run_if.system_fn)(world).map(Not::not));

        self.run_if(run_if)
    }
    fn before_all<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.before_all.add(other);

        self
    }
    fn after_all<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.after_all.add(other);

        self
    }
    fn display_name<T>(mut self, name: impl AsLabel<T>) -> WorkloadSystem {
        self.display_name = name.as_label();

        self
    }
    fn tag<T>(mut self, tag: impl AsLabel<T>) -> WorkloadSystem {
        self.tags.push(tag.as_label());

        self
    }
    #[track_caller]
    fn require_in_workload<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.require_in_workload.add(other);

        self
    }
    #[track_caller]
    fn require_before<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.require_before.add(other);

        self
    }
    #[track_caller]
    fn require_after<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.require_after.add(other);

        self
    }
}

macro_rules! impl_into_workload_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: WorldBorrow + BorrowInfo,)+ Ret, Func> SystemModificator<($($type,)+), Ret> for Func
        where
            Ret: 'static,
            Func: 'static
                + Send
                + Sync,
            for<'a, 'b> &'b Func:
                Fn($($type),+) -> Ret
                + Fn($($type::WorldView<'a>),+) -> Ret {

            #[track_caller]
            fn run_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();
                let run_if = run_if.into_workload_run_if().unwrap();

                system.run_if = Some(run_if.system_fn);

                system
            }
            #[track_caller]
            fn skip_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
                let mut run_if = run_if.into_workload_run_if().unwrap();

                run_if.system_fn = Box::new(move |world| (run_if.system_fn)(world).map(Not::not));

                SystemModificator::<($($type,)+), Ret>::run_if(self, run_if)
            }
            #[track_caller]
            fn before_all<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.before_all.add(other);

                system
            }
            #[track_caller]
            fn after_all<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.after_all.add(other);

                system
            }
            #[track_caller]
            fn display_name<Label>(self, name: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.display_name = name.as_label();

                system
            }
            #[track_caller]
            fn tag<Label>(self, tag: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.tags.push(tag.as_label());

                system
            }
            #[track_caller]
            fn require_in_workload<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.require_in_workload.add(other);

                system
            }
            #[track_caller]
            fn require_before<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.require_before.add(other);

                system
            }
            #[track_caller]
            fn require_after<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), Ret>::into_workload_system(self).unwrap();

                system.require_after.add(other);

                system
            }
        }
    }
}

macro_rules! into_workload_system {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_into_workload_system![$(($type, $index))*];
        into_workload_system![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_into_workload_system![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
into_workload_system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
into_workload_system![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
