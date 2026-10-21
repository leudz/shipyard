use crate::error;
use crate::scheduler::{Batches, Label, RegroupStep, Step};
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
            for (batch, batch_run_if) in batches.parallel.iter().zip(&batches.parallel_run_if) {
                let mut result = Ok(());

                let run_if = (
                    match batch.0 {
                        Some(step) => {
                            self.should_run(batches, system_names, step, batch_run_if.0)?
                        }
                        None => true,
                    },
                    batch
                        .1
                        .iter()
                        .zip(&batch_run_if.1)
                        .map(|(&step, &run_if_index)| {
                            self.should_run(batches, system_names, step, run_if_index)
                        })
                        .collect::<Result<alloc::vec::Vec<_>, error::RunWorkload>>()?,
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

        batches
            .sequential
            .iter()
            .zip(&batches.sequential_run_if)
            .try_for_each(|(&step, &run_if_index)| {
                if !self.should_run(batches, system_names, step, run_if_index)? {
                    return Ok(());
                }

                #[cfg(feature = "tracing")]
                {
                    self.run_step(systems, system_names, &batches.regroups, &parent_span, step)
                }
                #[cfg(not(feature = "tracing"))]
                {
                    self.run_step(systems, system_names, &batches.regroups, step)
                }
            })
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
        // We don't track which storages systems wrote to yet,
        // so pending regroups run every time, like entry ones.
        let all_storages = self.all_storages.borrow().map_err(|borrow| {
            error::RunWorkload::Regroup(error::GetStorage::AllStoragesBorrow(borrow))
        })?;

        all_storages
            .regroup_connected_set(&regroup.set)
            .map_err(error::RunWorkload::Regroup)
    }
}
