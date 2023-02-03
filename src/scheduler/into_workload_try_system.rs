use crate::all_storages::AllStorages;
use crate::borrow::{Borrow, BorrowInfo, Mutability};
use crate::info::DedupedLabels;
use crate::scheduler::into_workload_system::Nothing;
use crate::scheduler::label::SystemLabel;
use crate::scheduler::{TypeInfo, WorkloadSystem};
use crate::storage::StorageId;
use crate::type_id::TypeId;
use crate::World;
use crate::{error, AsLabel};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
#[cfg(not(feature = "std"))]
use core::any::Any;
use core::sync::atomic::{AtomicU32, Ordering};
#[cfg(feature = "std")]
use std::error::Error;

/// Trait used to add fallible systems to a workload.
pub trait IntoWorkloadTrySystem<Views, R> {
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
}

impl<R: 'static, F> IntoWorkloadTrySystem<Nothing, R> for F
where
    F: 'static + Send + Sync + Fn() -> R,
{
    #[cfg(feature = "std")]
    fn into_workload_try_system<Ok, Err: Into<Box<dyn Error + Send + Sync>>>(
        self,
    ) -> Result<WorkloadSystem, error::InvalidSystem>
    where
        R: Into<Result<Ok, Err>>,
    {
        let system_type_name = type_name::<F>();

        Ok(WorkloadSystem {
            borrow_constraints: Vec::new(),
            tracking_to_enable: Vec::new(),
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
                name: system_type_name.as_label(),
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

        Ok(WorkloadSystem {
            borrow_constraints: Vec::new(),
            tracking_to_enable: Vec::new(),
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
                name: system_type_name.as_label(),
            })],
            run_if: None,
            require_in_workload: DedupedLabels::new(),
            require_before: DedupedLabels::new(),
            require_after: DedupedLabels::new(),
        })
    }
}

macro_rules! impl_into_workload_try_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Borrow + BorrowInfo,)+ R: 'static, Func> IntoWorkloadTrySystem<($($type,)+), R> for Func
        where
            Func: 'static
                + Send
                + Sync,
            for<'a, 'b> &'b Func:
                Fn($($type),+) -> R
                + Fn($($type::View<'a>),+) -> R
        {
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

                let mut tracking_to_enable = Vec::new();
                $(
                    $type::enable_tracking(&mut tracking_to_enable);
                )+

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem {
                    borrow_constraints: borrows,
                    tracking_to_enable,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::borrow(&world, Some(last_run), current)?),+).into().map_err(error::Run::from_custom)?))
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

                let mut tracking_to_enable = Vec::new();
                $(
                    $type::enable_tracking(&mut tracking_to_enable);
                )+

                let last_run = AtomicU32::new(0);
                Ok(WorkloadSystem {
                    borrow_constraints: borrows,
                    tracking_to_enable,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = last_run.swap(current, Ordering::Acquire);
                        Ok(drop((&&self)($($type::borrow(&world, Some(last_run), current)?),+).into().map_err(error::Run::from_custom)?))
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
        }
    }
}

macro_rules! into_workload_try_system {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_into_workload_try_system![$(($type, $index))*];
        into_workload_try_system![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_into_workload_try_system![$(($type, $index))*];
    }
}

into_workload_try_system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
