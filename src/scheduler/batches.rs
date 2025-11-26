use crate::error;
use crate::scheduler::system::WorkloadRunIfFn;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// List of indexes into both systems and system_names
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub(crate) struct Batches {
    /// Index into the list of systems
    pub(crate) parallel: Vec<(Option<usize>, Vec<usize>)>,
    /// Index into `systems_run_if`
    pub(crate) parallel_run_if: Vec<(usize, Vec<usize>)>,
    /// Index into the list of systems
    pub(crate) sequential: Vec<usize>,
    /// Index into `systems_run_if`
    pub(crate) sequential_run_if: Vec<usize>,
    pub(crate) workload_run_if: Option<Box<dyn WorkloadRunIfFn>>,
    pub(crate) systems_run_if: Vec<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync>>,
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
