pub mod info;
mod into_workload;
mod into_workload_system;
mod label;
mod system;
mod workload;

pub use into_workload::IntoWorkload;
pub use into_workload_system::IntoWorkloadSystem;
pub use label::{AsLabel, Label};
pub use system::WorkloadSystem;
pub use workload::{ScheduledWorkload, Workload};

pub(crate) use info::TypeInfo;

use crate::error;
use crate::info::Requirements;
use crate::type_id::TypeId;
use crate::World;
use alloc::boxed::Box;
use alloc::vec::Vec;
use hashbrown::HashMap;

/// List of indexes into both systems and system_names
#[derive(Default)]
pub(super) struct Batches {
    pub(super) parallel: Vec<(Option<usize>, Vec<usize>)>,
    pub(super) sequential: Vec<usize>,
    pub(super) skip_if:
        Vec<Box<dyn Fn(crate::view::AllStoragesView<'_>) -> bool + Send + Sync + 'static>>,
}

#[cfg(test)]
impl core::fmt::Debug for Batches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Batches")
            .field("parallel", &self.parallel)
            .field("sequential", &self.sequential)
            .finish()
    }
}

#[cfg(test)]
impl PartialEq for Batches {
    fn eq(&self, other: &Self) -> bool {
        self.parallel == other.parallel && self.sequential == other.sequential
    }
}

#[cfg(test)]
impl Eq for Batches {}

// systems are stored in an array to easily find if a system was already added
// this wouldn't be possible if they were in the HashMap
//
// a batch lists systems that can run in parallel
#[allow(clippy::type_complexity)]
pub(crate) struct Scheduler {
    pub(crate) systems: Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    pub(crate) system_names: Vec<&'static str>,
    pub(crate) system_generators: Vec<fn(&mut Vec<TypeInfo>) -> TypeId>,
    // Workload label and system index in the workload's sequential ordering to system requirements
    pub(crate) system_labels: HashMap<(Box<dyn Label>, usize), Box<dyn Label>>,
    // Workload label and system index in the workload's sequential ordering to system requirements
    pub(crate) system_before: HashMap<(Box<dyn Label>, usize), Requirements>,
    // Workload label and system index in the workload's sequential ordering to system requirements
    pub(crate) system_after: HashMap<(Box<dyn Label>, usize), Requirements>,
    // system's `TypeId` to an index into both systems and system_names
    lookup_table: HashMap<TypeId, usize>,
    /// workload name to list of "batches"
    pub(crate) workloads: HashMap<Box<dyn Label>, Batches>,
    pub(crate) default: Box<dyn Label>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            systems: Vec::new(),
            system_names: Vec::new(),
            system_generators: Vec::new(),
            system_labels: HashMap::new(),
            system_before: HashMap::new(),
            system_after: HashMap::new(),
            lookup_table: HashMap::new(),
            workloads: HashMap::new(),
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
        if let Some(batches) = self.workloads.remove(old) {
            if &*self.default == old {
                self.default = new.clone();
            }

            self.workloads.insert(new, batches);
        }
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
