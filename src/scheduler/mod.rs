mod batches;
pub mod info;
mod into_workload;
mod into_workload_run_if;
mod into_workload_system;
mod label;
mod system;
mod system_modificator;
mod workload;
mod workload_modificator;

pub use into_workload::{IntoSequentialWorkload, IntoWorkload};
pub use into_workload_system::IntoWorkloadSystem;
pub use label::{AsLabel, Label};
pub use system::WorkloadSystem;
pub use system_modificator::SystemModificator;
pub use workload::{ScheduledWorkload, Workload};
pub use workload_modificator::WorkloadModificator;

pub(crate) use batches::{Batches, RegroupStep, Step};
pub(crate) use info::TypeInfo;

use crate::scheduler::info::WorkloadInfo;
use crate::world::World;
use crate::{ShipHashMap, error};
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::any::TypeId;
use hashbrown::hash_map::EntryRef;

/// Stores systems from all workloads and their scheduling.
//
// systems are stored in an array to easily find if a system was already added
// this wouldn't be possible if they were in the HashMap
//
// a batch lists systems that can run in parallel
#[allow(clippy::type_complexity)]
pub(crate) struct Scheduler {
    pub(crate) systems: Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    pub(crate) system_names: Vec<Box<dyn Label>>,
    pub(crate) system_generators:
        Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    // system's `TypeId` to an index into both systems and system_names
    lookup_table: ShipHashMap<TypeId, usize>,
    /// workload name to list of "batches"
    pub(crate) workloads: ShipHashMap<Box<dyn Label>, Batches>,
    pub(crate) workloads_info: ShipHashMap<Box<dyn Label>, WorkloadInfo>,
    pub(crate) default: Box<dyn Label>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            systems: Vec::new(),
            system_names: Vec::new(),
            system_generators: Vec::new(),
            lookup_table: ShipHashMap::new(),
            workloads: ShipHashMap::new(),
            workloads_info: ShipHashMap::new(),
            default: Box::new(""),
        }
    }
}

impl Scheduler {
    pub(crate) fn set_default<L: Label>(
        &mut self,
        label: L,
    ) -> Result<(), error::SetDefaultWorkload> {
        let label: Box<dyn Label> = Box::new(label);
        if self.workloads.contains_key(&label) {
            self.default = label;
            Ok(())
        } else {
            Err(error::SetDefaultWorkload::MissingWorkload)
        }
    }
    pub(crate) fn workload(&self, name: &dyn Label) -> Result<&Batches, error::RunWorkload> {
        if let Some(batches) = self.workloads.get(name) {
            Ok(batches)
        } else {
            Err(error::RunWorkload::MissingWorkload)
        }
    }
    pub(crate) fn default_workload(&self) -> &Batches {
        &self.workloads[&self.default]
    }
    pub(crate) fn contains_workload(&self, name: &dyn Label) -> bool {
        self.workloads.contains_key(name)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.workloads.is_empty()
    }
    pub(crate) fn rename(&mut self, old: &dyn Label, new: Box<dyn Label>) {
        let EntryRef::Occupied(workload) = self.workloads.entry_ref(old) else {
            return;
        };

        let name = format!("{new:?}");
        let workload_label = new.clone();
        let new_default = (&*self.default == old).then(|| new.clone());
        let (old_info_label, mut info) = self
            .workloads_info
            .remove_entry(old)
            .expect("registered workloads have exported information");
        let (old_workload_label, batches) = workload.remove_entry();
        info.name = name;

        // An equal key can have a different debug name. Use the new label in both maps.
        let replaced_workload = self.workloads.remove_entry(new.as_ref());
        let replaced_info = self.workloads_info.remove_entry(new.as_ref());
        self.workloads.insert(workload_label, batches);
        self.workloads_info.insert(new, info);
        if let Some(default) = new_default {
            self.default = default;
        }

        drop((
            old_workload_label,
            old_info_label,
            replaced_workload,
            replaced_info,
        ));
    }
}

impl core::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("Scheduler");

        debug_struct.field("default_workload", &self.default);
        debug_struct.field("workload_count", &self.workloads.len());
        debug_struct.field("workloads", &self.workloads.keys());
        debug_struct.field("system_count", &self.system_names.len());
        debug_struct.field("systems", &self.system_names);

        debug_struct.finish()
    }
}
