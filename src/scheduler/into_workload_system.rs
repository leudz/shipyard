use crate::all_storages::AllStorages;
use crate::borrow::{Borrow, BorrowInfo, IntoBorrow, Mutability};
use crate::error;
use crate::info::Requirements;
use crate::scheduler::{AsLabel, TypeInfo, WorkloadSystem};
use crate::storage::StorageId;
use crate::type_id::TypeId;
use crate::World;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::type_name;
#[cfg(not(feature = "std"))]
use core::any::Any;
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
    /// When building a workload, this system or workload will be placed before all invocation of the other system or workload.
    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
    /// When building a workload, this system or workload will be placed after all invocation of the other system or workload.
    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
}

pub struct Nothing;

impl<R: 'static, F> IntoWorkloadSystem<Nothing, R> for F
where
    F: 'static + Send + Sync + Fn() -> R,
{
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        let system_type_name = type_name::<F>();

        if TypeId::of::<R>() == TypeId::of::<crate::scheduler::Workload>() {
            return Err(error::InvalidSystem::WorkloadUsedAsSystem(system_type_name));
        }

        Ok(WorkloadSystem::System {
            borrow_constraints: Vec::new(),
            before: Requirements::new(),
            after: Requirements::new(),
            system_fn: Box::new(move |_: &World| {
                (self)();
                Ok(())
            }),
            system_type_id: TypeId::of::<F>(),
            system_type_name,
            generator: |_| TypeId::of::<F>(),
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

        if TypeId::of::<R>() == TypeId::of::<crate::scheduler::Workload>() {
            return Err(error::InvalidSystem::WorkloadUsedAsSystem(system_type_name));
        }

        Ok(WorkloadSystem::System {
            borrow_constraints: Vec::new(),
            before: Requirements::new(),
            after: Requirements::new(),
            system_fn: Box::new(move |_: &World| {
                (self)().into().map_err(error::Run::from_custom)?;
                Ok(())
            }),
            system_type_id: TypeId::of::<F>(),
            system_type_name,
            generator: |_| TypeId::of::<F>(),
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

        if TypeId::of::<R>() == TypeId::of::<crate::scheduler::Workload>() {
            return Err(error::InvalidSystem::WorkloadUsedAsSystem(system_type_name));
        }

        Ok(WorkloadSystem::System {
            borrow_constraints: Vec::new(),
            before: Requirements::new(),
            after: Requirements::new(),
            system_fn: Box::new(move |_: &World| {
                (self)().into().map_err(error::Run::from_custom)?;
                Ok(())
            }),
            system_type_id: TypeId::of::<F>(),
            system_type_name,
            generator: |_| TypeId::of::<F>(),
        })
    }
    #[track_caller]
    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let system_type_name = type_name::<F>();

        if TypeId::of::<R>() == TypeId::of::<crate::scheduler::Workload>() {
            panic!(
                "{}",
                error::InvalidSystem::WorkloadUsedAsSystem(system_type_name)
            );
        }

        let mut before = Requirements::new();
        before.add(other.as_label());

        WorkloadSystem::System {
            borrow_constraints: Vec::new(),
            before,
            after: Requirements::new(),
            system_fn: Box::new(move |_: &World| {
                (self)();
                Ok(())
            }),
            system_type_id: TypeId::of::<F>(),
            system_type_name,
            generator: |_| TypeId::of::<F>(),
        }
    }
    #[track_caller]
    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        let system_type_name = type_name::<F>();

        if TypeId::of::<R>() == TypeId::of::<crate::scheduler::Workload>() {
            panic!(
                "{}",
                error::InvalidSystem::WorkloadUsedAsSystem(system_type_name)
            );
        }

        let mut after = Requirements::new();
        after.add(other.as_label());

        WorkloadSystem::System {
            borrow_constraints: Vec::new(),
            before: Requirements::new(),
            after,
            system_fn: Box::new(move |_: &World| {
                (self)();
                Ok(())
            }),
            system_type_id: TypeId::of::<F>(),
            system_type_name,
            generator: |_| TypeId::of::<F>(),
        }
    }
}

impl IntoWorkloadSystem<(), ()> for WorkloadSystem {
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
    fn before_all<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        match &mut self {
            WorkloadSystem::System { before, .. } => before.add(other.as_label()),
            WorkloadSystem::Workload(workload) => workload.before.add(other.as_label()),
        };

        self
    }
    fn after_all<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        match &mut self {
            WorkloadSystem::System { after, .. } => after.add(other.as_label()),
            WorkloadSystem::Workload(workload) => workload.after.add(other.as_label()),
        };

        self
    }
}

macro_rules! impl_system {
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

                let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

                for a_type_info in &borrows[..mid] {
                    for b_type_info in &borrows[mid..] {
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

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem::System {
                    borrow_constraints: borrows,
                    before: Requirements::new(),
                    after: Requirements::new(),
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+)))
                    }),
                    system_type_id: TypeId::of::<Func>(),
                    system_type_name: type_name::<Func>(),
                    generator: |constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    },
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

                let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

                for a_type_info in &borrows[..mid] {
                    for b_type_info in &borrows[mid..] {
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

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem::System {
                    borrow_constraints: borrows,
                    before: Requirements::new(),
                    after: Requirements::new(),
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+).into().map_err(error::Run::from_custom)?))
                    }),
                    system_type_id: TypeId::of::<Func>(),
                    system_type_name: type_name::<Func>(),
                    generator: |constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    },
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

                let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

                for a_type_info in &borrows[..mid] {
                    for b_type_info in &borrows[mid..] {
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

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem::System {
                    borrow_constraints: borrows,
                    before: Requirements::new(),
                    after: Requirements::new(),
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+).into().map_err(error::Run::from_custom)?))
                    }),                    system_type_id: TypeId::of::<Func>(),
                    system_type_name: type_name::<Func>(),
                    generator: |constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    },
                })
            }
            #[track_caller]
            fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
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
                    panic!("{}", error::InvalidSystem::AllStorages);
                }

                let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

                for a_type_info in &borrows[..mid] {
                    for b_type_info in &borrows[mid..] {
                        if a_type_info.storage_id == b_type_info.storage_id {
                            match (a_type_info.mutability, b_type_info.mutability) {
                                (Mutability::Exclusive, Mutability::Exclusive) => {
                                    panic!("{}", error::InvalidSystem::MultipleViewsMut);
                                }
                                (Mutability::Exclusive, Mutability::Shared)
                                | (Mutability::Shared, Mutability::Exclusive) => {
                                    panic!("{}", error::InvalidSystem::MultipleViews);
                                }
                                (Mutability::Shared, Mutability::Shared) => {}
                            }
                        }
                    }
                }

                let mut before = Requirements::new();
                before.add(other.as_label());

                let last_run = AtomicU32::new(0);
                WorkloadSystem::System {
                    borrow_constraints: borrows,
                    before,
                    after: Requirements::new(),
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+)))
                    }),
                    system_type_id: TypeId::of::<Func>(),
                    system_type_name: type_name::<Func>(),
                    generator: |constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    },
                }
            }
            #[track_caller]
            fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
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
                    panic!("{}", error::InvalidSystem::AllStorages);
                }

                let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

                for a_type_info in &borrows[..mid] {
                    for b_type_info in &borrows[mid..] {
                        if a_type_info.storage_id == b_type_info.storage_id {
                            match (a_type_info.mutability, b_type_info.mutability) {
                                (Mutability::Exclusive, Mutability::Exclusive) => {
                                    panic!("{}", error::InvalidSystem::MultipleViewsMut);
                                }
                                (Mutability::Exclusive, Mutability::Shared)
                                | (Mutability::Shared, Mutability::Exclusive) => {
                                    panic!("{}", error::InvalidSystem::MultipleViews);
                                }
                                (Mutability::Shared, Mutability::Shared) => {}
                            }
                        }
                    }
                }

                let mut after = Requirements::new();
                after.add(other.as_label());

                let last_run = AtomicU32::new(0);
                WorkloadSystem::System {
                    borrow_constraints: borrows,
                    before: Requirements::new(),
                    after,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::Borrow::borrow(&world, Some(last_run), current)?),+)))
                    }),
                    system_type_id: TypeId::of::<Func>(),
                    system_type_name: type_name::<Func>(),
                    generator: |constraints| {
                        $(
                            $type::borrow_info(constraints);
                        )+

                        TypeId::of::<Func>()
                    },
                }
            }
        }
    }
}

macro_rules! system {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_system![$(($type, $index))*];
        system![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_system![$(($type, $index))*];
    }
}

system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
