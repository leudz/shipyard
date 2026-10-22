use crate::error;
use crate::scheduler::info::RegroupTrigger;
use crate::scheduler::{Batches, Label, RegroupStep, Step};
use crate::scratch::ScratchVec;
use crate::storage::StorageId;
use crate::world::World;
use alloc::boxed::Box;

impl World {
    #[cfg(feature = "parallel")]
    #[allow(clippy::type_complexity)]
    pub(crate) fn run_batches_parallel(
        &self,
        systems: &[Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>],
        system_names: &[Box<dyn Label>],
        batches: &Batches,
        #[cfg_attr(not(feature = "tracing"), allow(unused))] workload_name: &dyn Label,
    ) -> Result<(), error::RunWorkload> {
        #[cfg(feature = "tracing")]
        let parent_span = tracing::info_span!("workload", name = ?workload_name);
        #[cfg(feature = "tracing")]
        let _parent_span = parent_span.enter();

        let run_batch = || -> Result<(), error::RunWorkload> {
            // Each run starts with nothing pending, the entry regroups take care of what changed before.
            let mut pending = Pending::default();

            for (batch, batch_run_if) in batches.parallel.iter().zip(&batches.parallel_run_if) {
                // A batch's regroups only depend on the batches before it:
                // its systems don't borrow from the sets it regroups.
                let single_runs = match batch.0 {
                    Some(step) => {
                        self.should_run(batches, system_names, step, batch_run_if.0)?
                            && pending.needs(batches, step)
                    }
                    None => true,
                };
                let mut runs = Runs::new();
                for (&step, &run_if_index) in batch.1.iter().zip(&batch_run_if.1) {
                    runs.push(
                        self.should_run(batches, system_names, step, run_if_index)?
                            && pending.needs(batches, step),
                    );
                }

                #[cfg(feature = "tracing")]
                {
                    if let Some(step) = batch.0.filter(|_| !single_runs) {
                        trace_skipped(&parent_span, system_names, batches, step);
                    }
                    for (&step, &runs) in batch.1.iter().zip(&runs) {
                        if !runs {
                            trace_skipped(&parent_span, system_names, batches, step);
                        }
                    }
                }

                let mut start = 0;
                let single_step = batch.0.filter(|_| single_runs).or_else(|| {
                    let step = batch.1.first().copied().filter(|_| runs[0]);

                    if step.is_some() {
                        start = 1;
                    }

                    step
                });

                // Runs a step on the current thread.
                let run_step = |step| {
                    #[cfg(feature = "tracing")]
                    {
                        self.run_step(systems, system_names, &batches.regroups, &parent_span, step)
                    }
                    #[cfg(not(feature = "tracing"))]
                    {
                        self.run_step(systems, system_names, &batches.regroups, step)
                    }
                };

                // The batch's steps besides `single_step`, with whether they run.
                let rest = batch.1[start..]
                    .iter()
                    .copied()
                    .zip(runs[start..].iter().copied());

                // Waking a worker up takes microseconds, a clean regroup a hundred nanoseconds.
                // So we only spawn the parallel job for a system, or a regroup with work to do.
                // Otherwise every step left is skipped or a clean regroup, and we run them here.
                // Checking a regroup borrows its set, the batch's systems don't borrow from it.
                let spawns = rest.clone().any(|(step, runs)| {
                    runs && match step {
                        Step::System(_) => true,
                        Step::Regroup(index) => !self.is_regroup_clean(&batches.regroups[index]),
                    }
                });

                if spawns {
                    let mut result = Ok(());

                    rayon::in_place_scope(|scope| {
                        scope.spawn(|_| {
                            use rayon::prelude::*;

                            result = batch.1[start..]
                                .par_iter()
                                .zip(&runs[start..])
                                .try_for_each(
                                    |(&step, &runs)| {
                                        if runs { run_step(step) } else { Ok(()) }
                                    },
                                );
                        });

                        single_step.map_or(Ok(()), run_step)
                    })?;

                    result?;
                } else {
                    // Without a parallel job we don't need a scope: entering one has a cost and,
                    // on wasm, waiting on it causes a "condvar wait not supported" error.
                    let single = single_step.map(|step| (step, true));
                    for (step, runs) in single.into_iter().chain(rest) {
                        if runs {
                            run_step(step)?;
                        }
                    }
                }

                // Every job of the batch is done, we can record what they did.
                if let Some(step) = batch.0.filter(|_| single_runs) {
                    pending.ran(batches, step);
                }
                for (&step, &ran) in batch.1.iter().zip(&runs) {
                    if ran {
                        pending.ran(batches, step);
                    }
                }
            }

            Ok(())
        };

        if let Some(thread_pool) = &self.thread_pool {
            thread_pool.scope(|_| run_batch())
        } else {
            // Use non local ThreadPool
            run_batch()
        }
    }

    #[cfg(not(feature = "parallel"))]
    #[allow(clippy::type_complexity)]
    pub(crate) fn run_batches_sequential(
        &self,
        systems: &[Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>],
        system_names: &[Box<dyn Label>],
        batches: &Batches,
        #[cfg_attr(not(feature = "tracing"), allow(unused))] workload_name: &dyn Label,
    ) -> Result<(), error::RunWorkload> {
        #[cfg(feature = "tracing")]
        let parent_span = tracing::info_span!("workload", name = ?workload_name);
        #[cfg(feature = "tracing")]
        let _parent_span = parent_span.enter();

        // Each run starts with nothing pending, the entry regroups take care of what changed before.
        let mut pending = Pending::default();

        for (&step, &run_if_index) in batches.sequential.iter().zip(&batches.sequential_run_if) {
            if !self.should_run(batches, system_names, step, run_if_index)?
                || !pending.needs(batches, step)
            {
                #[cfg(feature = "tracing")]
                trace_skipped(&parent_span, system_names, batches, step);

                continue;
            }

            #[cfg(feature = "tracing")]
            self.run_step(systems, system_names, &batches.regroups, &parent_span, step)?;
            #[cfg(not(feature = "tracing"))]
            self.run_step(systems, system_names, &batches.regroups, step)?;

            pending.ran(batches, step);
        }

        Ok(())
    }

    /// Returns `true` when `regroup` would have nothing to do.
    ///
    /// `false` when its set can't be borrowed, running the regroup reports the error.
    #[cfg(feature = "parallel")]
    #[inline]
    fn is_regroup_clean(&self, regroup: &RegroupStep) -> bool {
        let Ok(all_storages) = self.all_storages.borrow() else {
            return false;
        };

        all_storages.is_set_clean(&regroup.set).unwrap_or(false)
    }

    /// Evaluates `step`'s condition, the one at `run_if_index` in `batches.systems_run_if`.
    ///
    /// `usize::MAX` means `step` has no condition. Regroups never have one.
    #[inline]
    fn should_run(
        &self,
        batches: &Batches,
        system_names: &[Box<dyn Label>],
        step: Step,
        run_if_index: usize,
    ) -> Result<bool, error::RunWorkload> {
        if run_if_index == usize::MAX {
            return Ok(true);
        }

        let Step::System(index) = step else {
            unreachable!("regroups have no condition");
        };

        (batches.systems_run_if[run_if_index])(self)
            .map_err(|err| error::RunWorkload::Run((system_names[index].clone(), err)))
    }

    #[allow(clippy::type_complexity)]
    #[inline]
    fn run_step(
        &self,
        systems: &[Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync>],
        system_names: &[Box<dyn Label>],
        regroups: &[RegroupStep],
        #[cfg(feature = "tracing")] parent_span: &tracing::Span,
        step: Step,
    ) -> Result<(), error::RunWorkload> {
        match step {
            Step::System(index) => {
                #[cfg(feature = "tracing")]
                {
                    self.run_single_system(systems, system_names, parent_span, index)
                }
                #[cfg(not(feature = "tracing"))]
                {
                    self.run_single_system(systems, system_names, index)
                }
            }
            Step::Regroup(index) => {
                #[cfg(feature = "tracing")]
                {
                    self.run_regroup(&regroups[index], parent_span)
                }
                #[cfg(not(feature = "tracing"))]
                {
                    self.run_regroup(&regroups[index])
                }
            }
        }
    }

    #[allow(clippy::type_complexity)]
    #[inline]
    fn run_single_system(
        &self,
        systems: &[Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync>],
        system_names: &[Box<dyn Label>],
        #[cfg(feature = "tracing")] parent_span: &tracing::Span,
        index: usize,
    ) -> Result<(), error::RunWorkload> {
        #[cfg(feature = "tracing")]
        let system_span =
            tracing::info_span!(parent: parent_span.clone(), "system", name = ?system_names[index]);
        #[cfg(feature = "tracing")]
        let _system_span = system_span.enter();

        (systems[index])(self)
            .map_err(|err| error::RunWorkload::Run((system_names[index].clone(), err)))
    }

    /// Regroups `regroup`'s connected set, borrowing all its storages exclusively.
    #[inline]
    fn run_regroup(
        &self,
        regroup: &RegroupStep,
        #[cfg(feature = "tracing")] parent_span: &tracing::Span,
    ) -> Result<(), error::RunWorkload> {
        // We only know if the set was clean once we borrowed it, `clean` is recorded at the end.
        #[cfg(feature = "tracing")]
        let regroup_span = tracing::info_span!(
            parent: parent_span.clone(),
            "regroup",
            set = ?set_names(regroup),
            trigger = ?regroup.trigger,
            clean = tracing::field::Empty,
        );
        #[cfg(feature = "tracing")]
        let _regroup_span = regroup_span.enter();

        #[cfg(test)]
        if let Some(hook) = &regroup.hook {
            (hook.0)();
        }

        let all_storages = self.all_storages.borrow().map_err(|borrow| {
            error::RunWorkload::Regroup(error::GetStorage::AllStoragesBorrow(borrow))
        })?;

        #[cfg_attr(not(feature = "tracing"), allow(unused))]
        let regrouped = all_storages
            .regroup_connected_set(&regroup.set)
            .map_err(error::RunWorkload::Regroup)?;

        #[cfg(feature = "tracing")]
        regroup_span.record("clean", !regrouped);

        Ok(())
    }
}

/// Reports a step that doesn't run this time: a system whose condition is `false`,
/// or a regroup with nothing pending.
#[cfg(feature = "tracing")]
fn trace_skipped(
    parent_span: &tracing::Span,
    system_names: &[Box<dyn Label>],
    batches: &Batches,
    step: Step,
) {
    match step {
        Step::System(index) => {
            tracing::info!(parent: parent_span, system = ?system_names[index], "skipped system");
        }
        Step::Regroup(index) => {
            let regroup = &batches.regroups[index];

            tracing::info!(
                parent: parent_span,
                set = ?set_names(regroup),
                trigger = ?regroup.trigger,
                "skipped regroup"
            );
        }
    }
}

/// Returns the names of the storages `regroup` borrows, its whole connected set.
#[cfg(feature = "tracing")]
fn set_names(regroup: &RegroupStep) -> alloc::vec::Vec<&str> {
    regroup
        .borrow
        .iter()
        .map(|type_info| &*type_info.name)
        .collect()
}

/// Whether each parallel step of a batch runs.
///
/// Batches rarely hold more than a few steps, so we keep them inline.
#[cfg(feature = "parallel")]
type Runs = ScratchVec<bool, 16>;

/// The grouped storages a workload run left pending.
///
/// A storage is pending after a system that borrows it exclusively ran,
/// until a regroup of its connected set.
///
/// Workloads only write a handful of grouped storages, so we keep them inline and look them up in
/// order. Every item is `Some`, `None` only fills the unused inline slots.
#[derive(Default)]
struct Pending(ScratchVec<Option<StorageId>, 8>);

impl Pending {
    /// Returns `false` for a regroup with nothing to do.
    ///
    /// Entry regroups always run, for the changes made outside the workload.
    /// The others only run when a member of their set is pending.
    #[inline]
    fn needs(&self, batches: &Batches, step: Step) -> bool {
        let Step::Regroup(index) = step else {
            return true;
        };
        let regroup = &batches.regroups[index];

        regroup.trigger == RegroupTrigger::Entry || self.holds_any(&regroup.set.members)
    }

    /// Records the effects of `step`, that just ran successfully.
    #[inline]
    fn ran(&mut self, batches: &Batches, step: Step) {
        match step {
            Step::System(index) => {
                // Workloads that didn't plan regroups don't have writes for their systems.
                if let Some(writes) = batches.grouped_writes.get(index)
                    && !writes.is_empty()
                {
                    self.add(writes);
                }
            }
            Step::Regroup(index) => self.remove(&batches.regroups[index].set.members),
        }
    }

    // The loops below only run for regroups and for systems writing a grouped storage, which
    // always come with a regroup costing far more than a call. Keeping them apart keeps `needs`
    // and `ran` small enough to inline for every step.

    /// Returns `true` if one of `members` is pending.
    #[inline(never)]
    fn holds_any(&self, members: &[StorageId]) -> bool {
        members
            .iter()
            .any(|&storage_id| self.0.contains(&Some(storage_id)))
    }

    /// Makes `writes` pending.
    #[inline(never)]
    fn add(&mut self, writes: &[StorageId]) {
        for &storage_id in writes {
            if !self.0.contains(&Some(storage_id)) {
                self.0.push(Some(storage_id));
            }
        }
    }

    /// Makes the sorted `members` of a regrouped set no longer pending.
    #[inline(never)]
    fn remove(&mut self, members: &[StorageId]) {
        self.0.retain(|pending| {
            !pending.is_some_and(|storage_id| members.binary_search(&storage_id).is_ok())
        });
    }
}
