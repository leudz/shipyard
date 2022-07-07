use crate::all_storages::AllStorages;
use crate::borrow::{Borrow, BorrowInfo, IntoBorrow, Mutability};
use crate::info::DedupedLabels;
use crate::scheduler::into_workload_run_if::IntoRunIf;
use crate::scheduler::label::{SystemLabel, WorkloadLabel};
use crate::scheduler::workload::Workload;
use crate::scheduler::{TypeInfo, WorkloadSystem};
use crate::storage::StorageId;
use crate::type_id::TypeId;
use crate::{error, AllStoragesViewMut, AsLabel, Label, Unique, UniqueStorage};
use crate::{Component, SparseSet, World};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
#[cfg(not(feature = "std"))]
use core::any::Any;
use core::ops::Not;
use core::sync::atomic::{AtomicU32, Ordering};
#[cfg(feature = "std")]
use std::error::Error;

/// Trait used to add systems to a workload.
///
/// Usually you don't have to use it directly.
pub trait IntoWorkloadSystem<B, R> {
    /// Wraps a function in a struct containing all information required by a workload.
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem>;
    /// Wraps a fallible function in a struct containing all information required by a workload.  
    /// The workload will stop if an error is returned.
    #[cfg(feature = "std")]
    fn into_workload_try_system<Ok, Err: Into<Box<dyn Error + Send + Sync>>>(
        self,
    ) -> Result<WorkloadSystem, error::InvalidSystem>
    where
        R: Into<Result<Ok, Err>>;
    /// Wraps a fallible function in a struct containing all information required by a workload.  
    /// The workload will stop if an error is returned.
    #[cfg(not(feature = "std"))]
    fn into_workload_try_system<Ok, Err: 'static + Send + Any>(
        self,
    ) -> Result<WorkloadSystem, error::InvalidSystem>
    where
        R: Into<Result<Ok, Err>>;
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
    /// Returns this systems's label.
    fn label(&self) -> Box<dyn Label>;
}

pub struct Nothing;

impl<R: 'static, F> IntoWorkloadSystem<Nothing, R> for F
where
    F: 'static + Send + Sync + Fn() -> R,
{
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        let system_type_name = type_name::<F>();

        if TypeId::of::<R>() == TypeId::of::<Workload>() {
            return Err(error::InvalidSystem::WorkloadUsedAsSystem(system_type_name));
        }

        Ok(WorkloadSystem {
            borrow_constraints: Vec::new(),
            system_fn: Box::new(move |_: &World| {
                (self)();
                Ok(())
            }),
            type_id: TypeId::of::<F>(),
            display_name: Box::new(system_type_name),
            generator: Box::new(|_| TypeId::of::<F>()),
            before_all: DedupedLabels::new(),
            after_all: DedupedLabels::new(),
            tags: vec![Box::new(SystemLabel {
                type_id: TypeId::of::<F>(),
                name: type_name::<F>().as_label(),
            })],
            run_if: None,
            require_in_workload: DedupedLabels::new(),
            require_before: DedupedLabels::new(),
            require_after: DedupedLabels::new(),
        })
    }
    #[cfg(feature = "std")]
    fn into_workload_try_system<Ok, Err: Into<Box<dyn Error + Send + Sync>>>(
        self,
    ) -> Result<WorkloadSystem, error::InvalidSystem>
    where
        R: Into<Result<Ok, Err>>,
    {
        let system_type_name = type_name::<F>();

        if TypeId::of::<R>() == TypeId::of::<Workload>() {
            return Err(error::InvalidSystem::WorkloadUsedAsSystem(system_type_name));
        }

        Ok(WorkloadSystem {
            borrow_constraints: Vec::new(),
            system_fn: Box::new(move |_: &World| {
                (self)().into().map_err(error::Run::from_custom)?;
                Ok(())
            }),
            type_id: TypeId::of::<F>(),
            display_name: Box::new(system_type_name),
            generator: Box::new(|_| TypeId::of::<F>()),
            before_all: DedupedLabels::new(),
            after_all: DedupedLabels::new(),
            tags: vec![Box::new(SystemLabel {
                type_id: TypeId::of::<F>(),
                name: type_name::<F>().as_label(),
            })],
            run_if: None,
            require_in_workload: DedupedLabels::new(),
            require_before: DedupedLabels::new(),
            require_after: DedupedLabels::new(),
        })
    }
    #[cfg(not(feature = "std"))]
    fn into_workload_try_system<Ok, Err: 'static + Send + Any>(
        self,
    ) -> Result<WorkloadSystem, error::InvalidSystem>
    where
        R: Into<Result<Ok, Err>>,
    {
        let system_type_name = type_name::<F>();

        if TypeId::of::<R>() == TypeId::of::<Workload>() {
            return Err(error::InvalidSystem::WorkloadUsedAsSystem(system_type_name));
        }

        Ok(WorkloadSystem {
            borrow_constraints: Vec::new(),
            system_fn: Box::new(move |_: &World| {
                (self)().into().map_err(error::Run::from_custom)?;
                Ok(())
            }),
            type_id: TypeId::of::<F>(),
            display_name: Box::new(system_type_name),
            generator: Box::new(|_| TypeId::of::<F>()),
            before_all: DedupedLabels::new(),
            after_all: DedupedLabels::new(),
            tags: vec![Box::new(SystemLabel {
                type_id: TypeId::of::<F>(),
                name: type_name::<F>().as_label(),
            })],
            run_if: None,
            require_in_workload: DedupedLabels::new(),
            require_before: DedupedLabels::new(),
            require_after: DedupedLabels::new(),
        })
    }
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
    fn label(&self) -> Box<dyn Label> {
        if TypeId::of::<R>() == TypeId::of::<Workload>() {
            Box::new(WorkloadLabel {
                type_id: TypeId::of::<F>(),
                name: type_name::<F>().as_label(),
            })
        } else {
            Box::new(SystemLabel {
                type_id: TypeId::of::<F>(),
                name: type_name::<F>().as_label(),
            })
        }
    }
}

impl IntoWorkloadSystem<WorkloadSystem, ()> for WorkloadSystem {
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        Ok(self)
    }
    #[cfg(feature = "std")]
    fn into_workload_try_system<Ok, Err>(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        Ok(self)
    }
    #[cfg(not(feature = "std"))]
    fn into_workload_try_system<Ok, Err>(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        Ok(self)
    }
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
    fn label(&self) -> Box<dyn Label> {
        Box::new(SystemLabel {
            type_id: self.type_id,
            name: self.display_name.clone(),
        })
    }
}

macro_rules! impl_into_workload_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: IntoBorrow + BorrowInfo,)+ R, Func> IntoWorkloadSystem<($($type,)+), R> for Func
        where
            Func: 'static
                + Send
                + Sync,
            for<'a, 'b> &'b Func:
                Fn($($type),+) -> R
                + Fn($(<$type::Borrow as Borrow<'a>>::View),+) -> R {

            fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem> {
                let mut borrows = Vec::new();
                $(
                    $type::borrow_info(&mut borrows);
                )+

                if borrows.contains(&TypeInfo {
                    name: "".into(),
                    storage_id: StorageId::of::<AllStorages>(),
                    mutability: Mutability::Exclusive,
                    thread_safe: true,
                }) && borrows.len() > 1
                {
                    return Err(error::InvalidSystem::AllStorages);
                }

                if borrows.len() > 1 {
                    for (i, a_type_info) in borrows[..borrows.len() - 1].iter().enumerate() {
                        for b_type_info in &borrows[i + 1..] {
                            if a_type_info.storage_id == b_type_info.storage_id {
                                match (a_type_info.mutability, b_type_info.mutability) {
                                    (Mutability::Exclusive, Mutability::Exclusive) => {
                                        return Err(error::InvalidSystem::MultipleViewsMut)
                                    }
                                    (Mutability::Exclusive, Mutability::Shared)
                                    | (Mutability::Shared, Mutability::Exclusive) => {
                                        return Err(error::InvalidSystem::MultipleViews)
                                    }
                                    (Mutability::Shared, Mutability::Shared) => {}
                                }
                            }
                        }
                    }
                }

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem {
                    borrow_constraints: borrows,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+)))
                    }),
                    type_id: TypeId::of::<Func>(),
                    display_name: Box::new(type_name::<Func>()),
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    tags: vec![Box::new(SystemLabel {
                        type_id: TypeId::of::<Func>(),
                        name: type_name::<Func>().as_label(),
                    })],
                    generator: Box::new(|constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    }),
                    run_if: None,
                    require_in_workload: DedupedLabels::new(),
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                })
            }
            #[cfg(feature = "std")]
            fn into_workload_try_system<Ok, Err: Into<Box<dyn Error + Send + Sync>>>(self) -> Result<WorkloadSystem, error::InvalidSystem> where R: Into<Result<Ok, Err>> {
                let mut borrows = Vec::new();
                $(
                    $type::borrow_info(&mut borrows);
                )+

                if borrows.contains(&TypeInfo {
                    name: "".into(),
                    storage_id: StorageId::of::<AllStorages>(),
                    mutability: Mutability::Exclusive,
                    thread_safe: true,
                }) && borrows.len() > 1
                {
                    return Err(error::InvalidSystem::AllStorages);
                }

                if borrows.len() > 1 {
                    for (i, a_type_info) in borrows[..borrows.len() - 1].iter().enumerate() {
                        for b_type_info in &borrows[i + 1..] {
                            if a_type_info.storage_id == b_type_info.storage_id {
                                match (a_type_info.mutability, b_type_info.mutability) {
                                    (Mutability::Exclusive, Mutability::Exclusive) => {
                                        return Err(error::InvalidSystem::MultipleViewsMut)
                                    }
                                    (Mutability::Exclusive, Mutability::Shared)
                                    | (Mutability::Shared, Mutability::Exclusive) => {
                                        return Err(error::InvalidSystem::MultipleViews)
                                    }
                                    (Mutability::Shared, Mutability::Shared) => {}
                                }
                            }
                        }
                    }
                }

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem {
                    borrow_constraints: borrows,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+).into().map_err(error::Run::from_custom)?))
                    }),
                    type_id: TypeId::of::<Func>(),
                    display_name: Box::new(type_name::<Func>()),
                    generator: Box::new(|constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    }),
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    tags: vec![Box::new(SystemLabel {
                        type_id: TypeId::of::<Func>(),
                        name: type_name::<Func>().as_label(),
                    })],
                    run_if: None,
                    require_in_workload: DedupedLabels::new(),
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                })
            }
            #[cfg(not(feature = "std"))]
            fn into_workload_try_system<Ok, Err: 'static + Send + Any>(self) -> Result<WorkloadSystem, error::InvalidSystem> where R: Into<Result<Ok, Err>> {
                let mut borrows = Vec::new();
                $(
                    $type::borrow_info(&mut borrows);
                )+

                if borrows.contains(&TypeInfo {
                    name: "".into(),
                    storage_id: StorageId::of::<AllStorages>(),
                    mutability: Mutability::Exclusive,
                    thread_safe: true,
                }) && borrows.len() > 1
                {
                    return Err(error::InvalidSystem::AllStorages);
                }

                if borrows.len() > 1 {
                    for (i, a_type_info) in borrows[..borrows.len() - 1].iter().enumerate() {
                        for b_type_info in &borrows[i + 1..] {
                            if a_type_info.storage_id == b_type_info.storage_id {
                                match (a_type_info.mutability, b_type_info.mutability) {
                                    (Mutability::Exclusive, Mutability::Exclusive) => {
                                        return Err(error::InvalidSystem::MultipleViewsMut)
                                    }
                                    (Mutability::Exclusive, Mutability::Shared)
                                    | (Mutability::Shared, Mutability::Exclusive) => {
                                        return Err(error::InvalidSystem::MultipleViews)
                                    }
                                    (Mutability::Shared, Mutability::Shared) => {}
                                }
                            }
                        }
                    }
                }

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem {
                    borrow_constraints: borrows,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+).into().map_err(error::Run::from_custom)?))
                    }),
                    type_id: TypeId::of::<Func>(),
                    display_name: Box::new(type_name::<Func>()),
                    generator: Box::new(|constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    }),
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    tags: vec![Box::new(SystemLabel {
                        type_id: TypeId::of::<Func>(),
                        name: type_name::<Func>().as_label(),
                    })],
                    run_if: None,
                    require_in_workload: DedupedLabels::new(),
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                })
            }
            #[track_caller]
            fn run_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();
                let run_if = run_if.into_workload_run_if().unwrap();

                system.run_if = Some(run_if.system_fn);

                system
            }
            #[track_caller]
            fn skip_if<RunB, Run: IntoRunIf<RunB>>(self, run_if: Run) -> WorkloadSystem {
                let mut run_if = run_if.into_workload_run_if().unwrap();

                run_if.system_fn = Box::new(move |world| (run_if.system_fn)(world).map(Not::not));

                IntoWorkloadSystem::<($($type,)+), R>::run_if(self, run_if)
            }
            #[track_caller]
            fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.before_all.add(other);

                system
            }
            #[track_caller]
            fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.after_all.add(other);

                system
            }
            #[track_caller]
            fn display_name<T>(self, name: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.display_name = name.as_label();

                system
            }
            #[track_caller]
            fn tag<T>(self, tag: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.tags.push(tag.as_label());

                system
            }
            #[track_caller]
            fn require_in_workload<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.require_in_workload.add(other);

                system
            }
            #[track_caller]
            fn require_before<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.require_before.add(other);

                system
            }
            #[track_caller]
            fn require_after<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
                let mut system = IntoWorkloadSystem::<($($type,)+), R>::into_workload_system(self).unwrap();

                system.require_after.add(other);

                system
            }
            fn label(&self) -> Box<dyn Label> {
                Box::new(SystemLabel {
                    type_id: TypeId::of::<Func>(),
                    name: type_name::<Func>().as_label(),
                })
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

into_workload_system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
