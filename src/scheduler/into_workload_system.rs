use crate::all_storages::AllStorages;
use crate::borrow::{BorrowInfo, Mutability, WorldBorrow};
use crate::error;
use crate::scheduler::info::DedupedLabels;
use crate::scheduler::label::{SystemLabel, WorkloadLabel};
use crate::scheduler::{AsLabel, Label, TypeInfo, Workload, WorkloadSystem};
use crate::storage::StorageId;
use crate::tracking::TrackingTimestamp;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
use core::sync::atomic::{AtomicU64, Ordering};

/// Trait used to add systems to a workload.
///
/// Usually you don't have to use it directly except if you want to handle the error.\
/// To modify the system execution see [SystemModificator](crate::SystemModificator).
pub trait IntoWorkloadSystem<B, R> {
    /// Wraps a function in a struct containing all information required by a workload.
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem>;
    // This can't be removed because of `WorkloadSystem`
    #[doc(hidden)]
    fn label(&self) -> Box<dyn Label>;
    #[doc(hidden)]
    fn call(&self) -> R;
}

pub struct Nothing;

impl<R, F> IntoWorkloadSystem<Nothing, R> for F
where
    R: 'static,
    F: 'static + Send + Sync + Fn() -> R,
{
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        let system_type_name = type_name::<F>();

        Ok(WorkloadSystem {
            borrow_constraints: Vec::new(),
            tracking_to_enable: Vec::new(),
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
    fn call(&self) -> R {
        (self)()
    }
}

impl IntoWorkloadSystem<WorkloadSystem, ()> for WorkloadSystem {
    fn into_workload_system(self) -> Result<WorkloadSystem, error::InvalidSystem> {
        Ok(self)
    }
    fn label(&self) -> Box<dyn Label> {
        Box::new(SystemLabel {
            type_id: self.type_id,
            name: self.display_name.clone(),
        })
    }
    fn call(&self) {
        unreachable!()
    }
}

macro_rules! impl_into_workload_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: WorldBorrow + BorrowInfo,)+ Ret, Func> IntoWorkloadSystem<($($type,)+), Ret> for Func
        where
            Ret: 'static,
            Func: 'static
                + Send
                + Sync,
            for<'a, 'b> &'b Func:
                Fn($($type),+) -> Ret
                + Fn($($type::WorldView<'a>),+) -> Ret {

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

                let mut tracking_to_enable = Vec::new();
                $(
                    $type::enable_tracking(&mut tracking_to_enable);
                )+

                let last_run = AtomicU64::new(0);
                Ok(WorkloadSystem {
                    borrow_constraints: borrows,
                    tracking_to_enable,
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = TrackingTimestamp::new(last_run.swap(current.get(), Ordering::Acquire));
                        Ok(drop((&&self)($($type::world_borrow(&world, Some(last_run), current)?),+)))
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
            fn label(&self) -> Box<dyn Label> {
                Box::new(SystemLabel {
                    type_id: TypeId::of::<Func>(),
                    name: type_name::<Func>().as_label(),
                })
            }
            fn call(&self) -> Ret {
                unreachable!()
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
