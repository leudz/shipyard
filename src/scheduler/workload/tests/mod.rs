mod batching;
mod conditions;
mod info_tests;
mod ordering;
#[cfg(all(feature = "parallel", feature = "std"))]
mod parallel;
mod pending;
mod prepare;
mod registration;
mod regroup;
mod requirements;
#[cfg(all(feature = "tracing", feature = "std"))]
mod traced;

use super::*;
use crate::component::{Component, Unique};
use crate::scheduler::Step::System;
use crate::scheduler::{Batches, Step, SystemModificator, WorkloadModificator};
use crate::scheduler::{IntoSequentialWorkload, IntoWorkload};
use crate::views::{AllStoragesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
use crate::world::World;
use alloc::string::String;

struct Usize(usize);
#[allow(unused)]
struct U32(u32);
#[allow(unused)]
struct U16(u16);

impl Component for Usize {
    type Tracking = crate::track::Untracked;
}
impl Component for U32 {
    type Tracking = crate::track::Untracked;
}
impl Component for U16 {
    type Tracking = crate::track::Untracked;
}
impl Unique for Usize {}
impl Unique for U32 {}
impl Unique for U16 {}

/// Grouped components, for regroup tests.
#[derive(Clone)]
struct A;
#[derive(Clone)]
struct B;
struct C;
struct D;

impl Component for A {
    type Tracking = crate::track::Untracked;
}
impl Component for B {
    type Tracking = crate::track::Untracked;
}
impl Component for C {
    type Tracking = crate::track::Untracked;
}
impl Component for D {
    type Tracking = crate::track::Untracked;
}

/// A workload's steps as short names, so probes can compare schedules by reading them.
///
/// User systems show their function name, regroups show as `regroup<i>`.
/// A callable in the workload several times shows once per occurrence.
#[derive(Debug, PartialEq, Eq)]
struct Schedule {
    /// Each parallel batch, its calling-thread step first.
    parallel: Vec<Vec<String>>,
    sequential: Vec<String>,
}

impl Schedule {
    /// Returns the schedule of a workload built on its own.
    fn built(workload: Workload) -> Schedule {
        let (scheduled, _) = workload.build().unwrap();

        Schedule::new(&scheduled.batches, &scheduled.system_names)
    }

    /// Returns the schedule of the workload `name`, as `world` registered it.
    fn registered(world: &World, name: &'static str) -> Schedule {
        let scheduler = world.scheduler.borrow().unwrap();
        let batches = scheduler.workload(&name).unwrap();

        Schedule::new(batches, &scheduler.system_names)
    }

    fn new(batches: &Batches, system_names: &[Box<dyn Label>]) -> Schedule {
        let name = |step: &Step| match step {
            System(index) => {
                // Names show as `System(full::type::path)`, we only keep the last segment.
                let label = alloc::format!("{:?}", system_names[*index]);
                let path = label.trim_end_matches(')');

                path.rsplit("::").next().unwrap().into()
            }
            Step::Regroup(index) => alloc::format!("regroup{index}"),
        };

        let parallel = batches
            .parallel
            .iter()
            .map(|(single, steps)| single.iter().chain(steps).map(name).collect())
            .collect();
        let sequential = batches.sequential.iter().map(name).collect();

        Schedule {
            parallel,
            sequential,
        }
    }

    /// Returns the parallel batch running `name`.
    ///
    /// ### Panics
    ///
    /// When `name` isn't in exactly one batch.
    fn batch(&self, name: &str) -> usize {
        let batches: Vec<usize> = (0..self.parallel.len())
            .filter(|&batch| self.parallel[batch].iter().any(|step| step == name))
            .collect();

        assert_eq!(batches.len(), 1, "{name} should be in one batch: {self:?}");

        batches[0]
    }

    /// Returns how many times `name` runs in the parallel and sequential schedules.
    fn count(&self, name: &str) -> (usize, usize) {
        let parallel = self.parallel.iter().flatten().filter(|step| *step == name);
        let sequential = self.sequential.iter().filter(|step| *step == name);

        (parallel.count(), sequential.count())
    }
}

/// Returns `names` as a schedule's steps.
fn steps(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.into()).collect()
}

/// Returns `true` when every component of `T` was evaluated by a regroup.
fn evaluated<T: Component + Send + Sync>(world: &World) -> bool {
    let view = world.borrow::<View<'_, T>>().unwrap();

    view.groups.unevaluated_start == view.len()
}
