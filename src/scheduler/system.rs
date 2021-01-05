use super::{TypeInfo, WorkloadBuilder};
use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::error;
use crate::storage::StorageId;
use crate::system::System;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::type_name;

/// Self contained system that may be inserted into a [`WorkloadBuilder`].
///
/// ### Example:
///
/// ```rust
/// use shipyard::{system, View, Workload, WorkloadSystem, World};
///
/// fn sys1(u32s: View<u32>) {}
/// fn sys2(usizes: View<usize>) {}
///
/// let workload_sys1: WorkloadSystem =
///     WorkloadSystem::new(|world| world.run(sys1), sys1).unwrap();
/// // or with the macro
/// let workload_sys2: WorkloadSystem = system!(sys2);
///
/// let mut workload = Workload::builder("my_workload");
/// workload.with_system(workload_sys1);
/// workload.with_system(workload_sys2);
/// ```
///
/// [`WorkloadBuilder`]: crate::WorkloadBuilder
pub struct WorkloadSystem {
    pub(super) system_type_id: TypeId,
    pub(super) system_type_name: &'static str,
    pub(super) system_fn: Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>,
    /// access information
    pub(super) borrow_constraints: Vec<TypeInfo>,
}

impl WorkloadSystem {
    /// Bundles all information needed by [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: crate::WorkloadBuilder
    pub fn new<
        'a,
        B,
        R,
        S: Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static,
        F: System<'a, (), B, R>,
    >(
        system: S,
        _: F,
    ) -> Result<WorkloadSystem, error::InvalidSystem> {
        let mut borrows = Vec::new();
        F::borrow_info(&mut borrows);

        if borrows.contains(&TypeInfo {
            name: "",
            storage_id: StorageId::of::<AllStorages>(),
            mutability: Mutability::Exclusive,
            is_send: true,
            is_sync: true,
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

        Ok(WorkloadSystem {
            borrow_constraints: borrows,
            system_fn: Box::new(system),
            system_type_id: TypeId::of::<S>(),
            system_type_name: type_name::<F>(),
        })
    }
}

impl Extend<WorkloadSystem> for WorkloadBuilder {
    fn extend<T: IntoIterator<Item = WorkloadSystem>>(&mut self, iter: T) {
        self.systems.extend(iter);
    }
}
