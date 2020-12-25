mod builder;
pub mod info;

pub use builder::{Workload, WorkloadBuilder};

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
#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub(super) struct Batches {
    pub(super) parallel: Vec<Vec<usize>>,
    pub(super) sequential: Vec<usize>,
}

// systems are stored in an array to easily find if a system was already added
// this wouldn't be possible if they were in the HashMap
//
// a batch lists systems that can run in parallel
#[allow(clippy::type_complexity)]
pub(crate) struct Scheduler {
    pub(super) systems: Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    pub(super) system_names: Vec<&'static str>,
    // system's `TypeId` to an index into both systems and system_names
    lookup_table: HashMap<TypeId, usize>,
    /// workload name to list of "batches"
    workloads: HashMap<Cow<'static, str>, Batches>,
    default: Cow<'static, str>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            systems: Vec::new(),
            system_names: Vec::new(),
            lookup_table: HashMap::new(),
            workloads: HashMap::new(),
            default: "".into(),
        }
    }
}

impl Scheduler {
    pub(super) fn set_default(
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
    pub(super) fn workload(&self, name: &str) -> Result<&Batches, error::RunWorkload> {
        if let Some(batches) = self.workloads.get(name) {
            Ok(batches)
        } else {
            Err(error::RunWorkload::MissingWorkload)
        }
    }
    pub(super) fn default_workload(&self) -> &Batches {
        &self.workloads[&self.default]
    }
    pub(super) fn is_empty(&self) -> bool {
        self.workloads.is_empty()
    }
}
