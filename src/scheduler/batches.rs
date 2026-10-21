use crate::all_storages::{ConnectedSet, Grouping};
use crate::borrow::Mutability;
use crate::error;
use crate::scheduler::info::{RegroupInfo, RegroupTrigger, TypeInfo};
use crate::scheduler::system::WorkloadRunIfFn;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// A workload's steps, in parallel and sequential order.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub(crate) struct Batches {
    /// Steps of each batch, the first one runs on the calling thread.
    pub(crate) parallel: Vec<(Option<Step>, Vec<Step>)>,
    /// Index into `systems_run_if`
    pub(crate) parallel_run_if: Vec<(usize, Vec<usize>)>,
    /// Steps in the order they run without parallelism.
    pub(crate) sequential: Vec<Step>,
    /// Index into `systems_run_if`
    pub(crate) sequential_run_if: Vec<usize>,
    pub(crate) workload_run_if: Option<Box<dyn WorkloadRunIfFn>>,
    pub(crate) systems_run_if: Vec<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync>>,
    /// The regroups `Step::Regroup` points to.
    pub(crate) regroups: Vec<RegroupStep>,
}

/// A unit of work in a workload.
///
/// A callable can be in a workload several times, each occurrence is its own step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Runs the user system at this index in the systems list.
    System(usize),
    /// Runs the regroup at this index in [`Batches::regroups`].
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "workloads don't plan regroups yet")
    )]
    Regroup(usize),
}

/// A regroup a workload runs between its systems.
///
/// Holds ids only, the storages are borrowed each time it runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RegroupStep {
    /// The connected set to regroup.
    pub(crate) set: ConnectedSet,
    /// Every member of the set, borrowed exclusively.
    pub(crate) borrow: Vec<TypeInfo>,
    pub(crate) trigger: RegroupTrigger,
}

impl RegroupStep {
    /// Returns the step regrouping `set`, one of `grouping`'s connected sets.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "workloads don't plan regroups yet")
    )]
    pub(crate) fn new(set: &ConnectedSet, grouping: &Grouping, trigger: RegroupTrigger) -> Self {
        let borrow = set
            .members
            .iter()
            .map(|&storage_id| {
                let member = grouping
                    .member(storage_id)
                    .expect("a connected set's members are declared");

                TypeInfo {
                    name: member.storage_name.into(),
                    mutability: Mutability::Exclusive,
                    storage_id,
                    thread_safe: member.thread_safe,
                }
            })
            .collect();

        RegroupStep {
            set: set.clone(),
            borrow,
            trigger,
        }
    }

    /// Returns what workload information shows about this step.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "workloads don't plan regroups yet")
    )]
    pub(crate) fn info(&self) -> RegroupInfo {
        RegroupInfo {
            borrow: self.borrow.clone(),
            trigger: self.trigger,
        }
    }
}

#[cfg(test)]
impl core::fmt::Debug for Batches {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Batches")
            .field("parallel", &self.parallel)
            .field("sequential", &self.sequential)
            .field("regroups", &self.regroups)
            .finish()
    }
}

#[cfg(test)]
impl PartialEq for Batches {
    fn eq(&self, other: &Self) -> bool {
        self.parallel == other.parallel
            && self.sequential == other.sequential
            && self.regroups == other.regroups
    }
}

#[cfg(test)]
impl Eq for Batches {}
