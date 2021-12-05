mod builder;
pub mod info;
mod into_workload_system;
mod system;

pub use builder::{Workload, WorkloadBuilder};
pub use into_workload_system::IntoWorkloadSystem;
pub use system::WorkloadSystem;

pub(crate) use info::TypeInfo;

use crate::error;
use crate::type_id::TypeId;
use crate::World;
use alloc::borrow::Cow;
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
    // system's `TypeId` to an index into both systems and system_names
    lookup_table: HashMap<TypeId, usize>,
    /// workload name to list of "batches"
    pub(crate) workloads: HashMap<Cow<'static, str>, Batches>,
    default: Cow<'static, str>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            systems: Vec::new(),
            system_names: Vec::new(),
            system_generators: Vec::new(),
            lookup_table: HashMap::new(),
            workloads: HashMap::new(),
            default: "".into(),
        }
    }
}

impl Scheduler {
    pub(crate) fn set_default(
        &mut self,
        name: Cow<'static, str>,
    ) -> Result<(), error::SetDefaultWorkload> {
        if self.workloads.contains_key(&name) {
            self.default = name;
            Ok(())
        } else {
            Err(error::SetDefaultWorkload::MissingWorkload)
        }
    }
    pub(crate) fn workload(&self, name: &str) -> Result<&Batches, error::RunWorkload> {
        if let Some(batches) = self.workloads.get(name) {
            Ok(batches)
        } else {
            Err(error::RunWorkload::MissingWorkload)
        }
    }
    pub(crate) fn default_workload(&self) -> &Batches {
        &self.workloads[&self.default]
    }
    pub(crate) fn contains_workload(&self, name: &str) -> bool {
        self.workloads.contains_key(name)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.workloads.is_empty()
    }
    pub(crate) fn rename(&mut self, old: Cow<'static, str>, new: Cow<'static, str>) {
        if let Some(batches) = self.workloads.remove(&old) {
            self.workloads.insert(new.clone(), batches);

            if self.default == old {
                self.default = new;
            }
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
