use crate::error;
use crate::scheduler::{Batches, Label};
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
            for (batch, batches_run_if) in batches.parallel.iter().zip(&batches.parallel_run_if) {
                let mut result = Ok(());
                let run_if = (
                    if let Some(run_if_index) = batches_run_if.0 {
                        if let Some(run_if) = &batches.sequential_run_if[run_if_index] {
                            (run_if)(self).map_err(|err| {
                                error::RunWorkload::Run((
                                    system_names[batch.0.unwrap()].clone(),
                                    err,
                                ))
                            })?
                        } else {
                            true
                        }
                    } else {
                        true
                    },
                    batches_run_if
                        .1
                        .iter()
                        .map(|run_if_index| {
                            if let Some(run_if) = &batches.sequential_run_if[*run_if_index] {
                                (run_if)(self).map_err(|err| {
                                    error::RunWorkload::Run((
                                        system_names[batches.sequential[*run_if_index]].clone(),
                                        err,
                                    ))
                                })
                            } else {
                                Ok(true)
                            }
                        })
                        .collect::<Result<alloc::vec::Vec<_>, error::RunWorkload>>()?,
                );

                let mut start = 0;
                let single_system = batch.0.filter(|_| run_if.0).or_else(|| {
                    let system = batch.1.first().copied().filter(|_| run_if.1[0]);

                    if system.is_some() {
                        start = 1;
                    }

                    system
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
                                .try_for_each(|(&index, should_run)| {
                                    if !should_run {
                                        return Ok(());
                                    }

                                    #[cfg(feature = "tracing")]
                                    {
                                        self.run_single_system(
                                            systems,
                                            system_names,
                                            &parent_span,
                                            index,
                                        )
                                    }
                                    #[cfg(not(feature = "tracing"))]
                                    {
                                        self.run_single_system(systems, system_names, index)
                                    }
                                });
                        });
                    }

                    if let Some(index) = single_system {
                        #[cfg(feature = "tracing")]
                        self.run_single_system(systems, system_names, &parent_span, index)?;
                        #[cfg(not(feature = "tracing"))]
                        self.run_single_system(systems, system_names, index)?;
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
            .try_for_each(|(&index, run_if)| {
                if let Some(run_if) = run_if.as_ref() {
                    let should_run = (run_if)(self).map_err(|err| {
                        error::RunWorkload::Run((system_names[index].clone(), err))
                    })?;

                    if !should_run {
                        return Ok(());
                    }
                }

                #[cfg(feature = "tracing")]
                {
                    self.run_single_system(systems, system_names, &parent_span, index)
                }
                #[cfg(not(feature = "tracing"))]
                {
                    self.run_single_system(systems, system_names, index)
                }
            })
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
}
