use crate::all_storages::AllStorages;
use crate::borrow::{BorrowInfo, Mutability, WorldBorrow};
use crate::error;
use crate::scheduler::system::{RunIf, WorkloadRunIfFn};
use crate::scheduler::TypeInfo;
use crate::storage::StorageId;
use crate::tracking::TrackingTimestamp;
use crate::World;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

pub trait IntoRunIf<B> {
    fn into_workload_run_if(self) -> Result<RunIf, error::InvalidSystem>;
}

pub struct Nothing;

impl<F> IntoRunIf<Nothing> for F
where
    F: 'static + Send + Sync + Fn() -> bool,
{
    fn into_workload_run_if(self) -> Result<RunIf, error::InvalidSystem> {
        Ok(RunIf {
            system_fn: Box::new(move |_: &World| Ok((self)())),
        })
    }
}

impl IntoRunIf<RunIf> for RunIf {
    fn into_workload_run_if(self) -> Result<RunIf, error::InvalidSystem> {
        Ok(self)
    }
}

macro_rules! impl_into_workload_run_if {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: WorldBorrow + BorrowInfo,)+ Func> IntoRunIf<($($type,)+)> for Func
        where
            Func: 'static
                + Send
                + Sync,
            for<'a, 'b> &'b Func:
                Fn($($type),+) -> bool
                + Fn($($type::WorldView<'a>),+) -> bool {

            fn into_workload_run_if(self) -> Result<RunIf, error::InvalidSystem> {
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

                let last_run = AtomicU64::new(0);
                Ok(RunIf {
                    system_fn: Box::new(move |world: &World| {
                        let current = world.get_current();
                        let last_run = TrackingTimestamp::new(last_run.swap(current.get(), Ordering::Acquire));
                        Ok((&&self)($($type::world_borrow(&world, Some(last_run), current)?),+))
                    }),
                })
            }
        }
    }
}

macro_rules! into_workload_run_if {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_into_workload_run_if![$(($type, $index))*];
        into_workload_run_if![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_into_workload_run_if![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
into_workload_run_if![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
into_workload_run_if![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];

pub trait IntoWorkloadRunIf<B> {
    fn into_workload_run_if(self) -> Result<Box<dyn WorkloadRunIfFn>, error::InvalidSystem>;
}

impl<F> IntoWorkloadRunIf<Nothing> for F
where
    F: 'static + Send + Sync + Clone + Fn() -> bool,
{
    fn into_workload_run_if(self) -> Result<Box<dyn WorkloadRunIfFn>, error::InvalidSystem> {
        Ok(Box::new(move |_: &World| Ok((self)())))
    }
}

macro_rules! impl_into_workload_run_if {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: WorldBorrow + BorrowInfo,)+ Func> IntoWorkloadRunIf<($($type,)+)> for Func
        where
            Func: 'static
                + Send
                + Sync
                + Clone,
            for<'a, 'b> &'b Func:
                Fn($($type),+) -> bool
                + Fn($($type::WorldView<'a>),+) -> bool {

            fn into_workload_run_if(self) -> Result<Box<dyn WorkloadRunIfFn>, error::InvalidSystem> {
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

                let last_run = Arc::new(AtomicU64::new(0));
                Ok(Box::new(move |world: &World| {
                    let current = world.get_current();
                    let last_run = TrackingTimestamp::new(last_run.swap(current.get(), Ordering::Acquire));
                    Ok((&&self)($($type::world_borrow(&world, Some(last_run), current)?),+))
                }))
            }
        }
    }
}

macro_rules! into_workload_run_if {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_into_workload_run_if![$(($type, $index))*];
        into_workload_run_if![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_into_workload_run_if![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
into_workload_run_if![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
into_workload_run_if![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
