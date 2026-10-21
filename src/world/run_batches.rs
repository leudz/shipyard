use crate::error;
use crate::scheduler::info::RegroupTrigger;
use crate::scheduler::{Batches, Label, RegroupStep, Step};
use crate::storage::StorageId;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec::Vec;

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
                let mut result = Ok(());

                // A batch's regroups only depend on the batches before it:
                // its systems don't borrow from the sets it regroups.
                let run_if = (
                    match batch.0 {
                        Some(step) => {
                            self.should_run(batches, system_names, step, batch_run_if.0)?
                                && pending.needs(batches, step)
                        }
                        None => true,
                    },
                    batch
                        .1
                        .iter()
                        .zip(&batch_run_if.1)
                        .map(|(&step, &run_if_index)| {
                            Ok(self.should_run(batches, system_names, step, run_if_index)?
                                && pending.needs(batches, step))
                        })
                        .collect::<Result<Vec<_>, error::RunWorkload>>()?,
                );

                let mut start = 0;
                let single_step = batch.0.filter(|_| run_if.0).or_else(|| {
                    let step = batch.1.first().copied().filter(|_| run_if.1[0]);

                    if step.is_some() {
                        start = 1;
                    }

                    step
                });

                rayon::in_place_scope(|scope| {
                    // This check exists to avoid spawning a parallel job when possible.
                    // On wasm it causes a "condvar wait not supported" error.
                    if start < batch.1.len() {
                        scope.spawn(|_| {
                            use rayon::prelude::*;

                            result = batch.1[start..]
                                .par_iter()
                                .zip(&run_if.1[start..])
                                .try_for_each(|(&step, should_run)| {
                                    if !should_run {
                                        return Ok(());
                                    }

                                    #[cfg(feature = "tracing")]
                                    {
                                        self.run_step(
                                            systems,
                                            system_names,
                                            &batches.regroups,
                                            &parent_span,
                                            step,
                                        )
                                    }
                                    #[cfg(not(feature = "tracing"))]
                                    {
                                        self.run_step(
                                            systems,
                                            system_names,
                                            &batches.regroups,
                                            step,
                                        )
                                    }
                                });
                        });
                    }

                    if let Some(step) = single_step {
                        #[cfg(feature = "tracing")]
                        self.run_step(
                            systems,
                            system_names,
                            &batches.regroups,
                            &parent_span,
                            step,
                        )?;
                        #[cfg(not(feature = "tracing"))]
                        self.run_step(systems, system_names, &batches.regroups, step)?;
                    }

                    Ok(())
                })?;

                result?;

                // Every job of the batch is done, we can record what they did.
                if let Some(step) = batch.0.filter(|_| run_if.0) {
                    pending.ran(batches, step);
                }
                for (&step, &ran) in batch.1.iter().zip(&run_if.1) {
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

    /// Evaluates `step`'s condition, the one at `run_if_index` in `batches.systems_run_if`.
    ///
    /// `usize::MAX` means `step` has no condition. Regroups never have one.
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
            Step::Regroup(index) => self.run_regroup(&regroups[index]),
        }
    }

    #[allow(clippy::type_complexity)]
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
    fn run_regroup(&self, regroup: &RegroupStep) -> Result<(), error::RunWorkload> {
        #[cfg(test)]
        if let Some(hook) = &regroup.hook {
            (hook.0)();
        }

        let all_storages = self.all_storages.borrow().map_err(|borrow| {
            error::RunWorkload::Regroup(error::GetStorage::AllStoragesBorrow(borrow))
        })?;

        all_storages
            .regroup_connected_set(&regroup.set)
            .map_err(error::RunWorkload::Regroup)
    }
}

/// The grouped storages a workload run left pending, sorted.
///
/// A storage is pending after a system that borrows it exclusively ran,
/// until a regroup of its connected set.
#[derive(Default)]
struct Pending(Vec<StorageId>);

impl Pending {
    /// Returns `false` for a regroup with nothing to do.
    ///
    /// Entry regroups always run, for the changes made outside the workload.
    /// The others only run when a member of their set is pending.
    fn needs(&self, batches: &Batches, step: Step) -> bool {
        let Step::Regroup(index) = step else {
            return true;
        };
        let regroup = &batches.regroups[index];

        regroup.trigger == RegroupTrigger::Entry
            || regroup
                .set
                .members
                .iter()
                .any(|storage_id| self.0.binary_search(storage_id).is_ok())
    }

    /// Records the effects of `step`, that just ran successfully.
    fn ran(&mut self, batches: &Batches, step: Step) {
        match step {
            Step::System(index) => {
                // Workloads that didn't plan regroups don't have writes for their systems.
                let writes = batches
                    .grouped_writes
                    .get(index)
                    .map_or(&[][..], Vec::as_slice);

                for &storage_id in writes {
                    if let Err(position) = self.0.binary_search(&storage_id) {
                        self.0.insert(position, storage_id);
                    }
                }
            }
            Step::Regroup(index) => {
                let members = &batches.regroups[index].set.members;

                self.0
                    .retain(|storage_id| members.binary_search(storage_id).is_err());
            }
        }
    }
}
