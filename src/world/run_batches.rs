use crate::error;
use crate::scheduler::{Batches, Label, RegroupStep, Step};
use crate::world::World;
use alloc::boxed::Box;

impl World {
    /// Runs the batches in order, the steps of a batch in parallel.
    ///
    /// The batch loop stays on the calling thread, even with a local pool:
    /// the calling-thread step has to run on the world's thread.
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

        let run_step = |step, run_if_index| {
            #[cfg(feature = "tracing")]
            {
                self.run_step(
                    systems,
                    system_names,
                    batches,
                    &parent_span,
                    step,
                    run_if_index,
                )
            }
            #[cfg(not(feature = "tracing"))]
            {
                self.run_step(systems, system_names, batches, step, run_if_index)
            }
        };

        for (batch, batch_run_if) in batches.parallel.iter().zip(&batches.parallel_run_if) {
            // The calling thread runs the batch's calling-thread step,
            // or its first step when it doesn't have one. The workers run the rest.
            let (single, rest, rest_run_if) = match (batch.0, batch.1.split_first()) {
                (Some(step), _) => (Some((step, batch_run_if.0)), &*batch.1, &*batch_run_if.1),
                (None, Some((&step, rest))) => {
                    (Some((step, batch_run_if.1[0])), rest, &batch_run_if.1[1..])
                }
                (None, None) => (None, &[][..], &[][..]),
            };
            let run_single = || single.map_or(Ok(()), |(step, run_if)| run_step(step, run_if));

            if rest.is_empty() {
                // Without a parallel job we don't need a scope: entering one has a cost and,
                // on wasm, waiting on it causes a "condvar wait not supported" error.
                run_single()?;

                continue;
            }

            let mut result = Ok(());

            self.in_place_scope(|scope| {
                scope.spawn(|_| {
                    use rayon::prelude::*;

                    result = rest
                        .par_iter()
                        .zip(rest_run_if)
                        .try_for_each(|(&step, &run_if)| run_step(step, run_if));
                });

                run_single()
            })?;

            result?;
        }

        Ok(())
    }

    /// Runs `op` on the current thread, in a scope whose spawned jobs run in the world's pool,
    /// or the global pool when the world doesn't have one.
    #[cfg(feature = "parallel")]
    fn in_place_scope<'scope, R>(&self, op: impl FnOnce(&rayon::Scope<'scope>) -> R) -> R {
        match &self.thread_pool {
            Some(thread_pool) => thread_pool.in_place_scope(op),
            None => rayon::in_place_scope(op),
        }
    }

    /// Runs the steps one after the other, in sequential order.
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

        for (&step, &run_if_index) in batches.sequential.iter().zip(&batches.sequential_run_if) {
            #[cfg(feature = "tracing")]
            self.run_step(
                systems,
                system_names,
                batches,
                &parent_span,
                step,
                run_if_index,
            )?;
            #[cfg(not(feature = "tracing"))]
            self.run_step(systems, system_names, batches, step, run_if_index)?;
        }

        Ok(())
    }

    /// Runs `step` on the current thread, its condition right before it.
    ///
    /// `run_if_index` is its condition's index in `batches.systems_run_if`,
    /// `usize::MAX` when it doesn't have one. Regroups never have one.
    #[allow(clippy::type_complexity)]
    #[inline]
    fn run_step(
        &self,
        systems: &[Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync>],
        system_names: &[Box<dyn Label>],
        batches: &Batches,
        #[cfg(feature = "tracing")] parent_span: &tracing::Span,
        step: Step,
        run_if_index: usize,
    ) -> Result<(), error::RunWorkload> {
        match step {
            Step::System(index) => {
                if !self.should_run(batches, system_names, index, run_if_index)? {
                    #[cfg(feature = "tracing")]
                    tracing::info!(parent: parent_span, system = ?system_names[index], "skipped system");

                    return Ok(());
                }

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
                    self.run_regroup(&batches.regroups[index], parent_span)
                }
                #[cfg(not(feature = "tracing"))]
                {
                    self.run_regroup(&batches.regroups[index])
                }
            }
        }
    }

    /// Evaluates the condition of the system at `index`, the one at `run_if_index` in
    /// `batches.systems_run_if`.
    ///
    /// `usize::MAX` means the system has no condition.
    #[inline]
    fn should_run(
        &self,
        batches: &Batches,
        system_names: &[Box<dyn Label>],
        index: usize,
        run_if_index: usize,
    ) -> Result<bool, error::RunWorkload> {
        if run_if_index == usize::MAX {
            return Ok(true);
        }

        (batches.systems_run_if[run_if_index])(self)
            .map_err(|err| error::RunWorkload::Run((system_names[index].clone(), err)))
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
    ///
    /// A set with nothing to move only costs the borrow and a check.
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

/// Returns the names of the storages `regroup` borrows, its whole connected set.
#[cfg(feature = "tracing")]
fn set_names(regroup: &RegroupStep) -> alloc::vec::Vec<&str> {
    regroup
        .borrow
        .iter()
        .map(|type_info| &*type_info.name)
        .collect()
}
