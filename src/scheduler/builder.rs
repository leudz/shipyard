use super::info::{BatchInfo, Conflict, SystemId, SystemInfo, TypeInfo, WorkloadInfo};
use super::{Batches, IntoWorkloadSystem, Scheduler, WorkloadSystem};
use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::error;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::borrow::Cow;
// this is the macro, not the module
use alloc::vec;
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::any::Any;
use core::iter::Extend;
#[cfg(feature = "std")]
use std::error::Error;

/// Used to create a [`WorkloadBuilder`].
///
/// You can also use [`WorkloadBuilder::new`] or [`WorkloadBuilder::default`].
///
/// [`WorkloadBuilder`]: crate::WorkloadBuilder
/// [`WorkloadBuilder::new`]: crate::WorkloadBuilder::new()
/// [`WorkloadBuilder::default`]: crate::WorkloadBuilder::default()
pub struct Workload;

impl Workload {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: crate::WorkloadBuilder
    pub fn builder<N: Into<Cow<'static, str>>>(name: N) -> WorkloadBuilder {
        WorkloadBuilder::new(name)
    }
}

pub(super) enum WorkUnit {
    System(WorkloadSystem),
    Workload(Cow<'static, str>),
}

impl From<WorkloadSystem> for WorkUnit {
    fn from(system: WorkloadSystem) -> Self {
        WorkUnit::System(system)
    }
}

impl From<Cow<'static, str>> for WorkUnit {
    fn from(workload: Cow<'static, str>) -> Self {
        WorkUnit::Workload(workload)
    }
}

/// Keeps information to create a workload.
///
/// A workload is a collection of systems. They will execute as much in parallel as possible.  
/// They are evaluated first to last when they can't be parallelized.  
/// The default workload will automatically be set to the first workload added.
#[derive(Default)]
pub struct WorkloadBuilder {
    pub(super) systems: Vec<WorkUnit>,
    name: Cow<'static, str>,
}

impl WorkloadBuilder {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: crate::WorkloadBuilder
    ///
    /// ### Example
    /// ```
    /// use shipyard::{IntoIter, View, ViewMut, Workload, World};
    ///
    /// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
    ///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///         *x += y as usize;
    ///     }
    /// }
    ///
    /// fn check(usizes: View<usize>) {
    ///     let mut iter = usizes.iter();
    ///     assert_eq!(iter.next(), Some(&1));
    ///     assert_eq!(iter.next(), Some(&5));
    ///     assert_eq!(iter.next(), Some(&9));
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity((0usize, 1u32));
    /// world.add_entity((2usize, 3u32));
    /// world.add_entity((4usize, 5u32));
    ///
    /// Workload::builder("Add & Check")
    ///     .with_system(&add)
    ///     .with_system(&check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    pub fn new<N: Into<Cow<'static, str>>>(name: N) -> Self {
        WorkloadBuilder {
            systems: Vec::new(),
            name: name.into(),
        }
    }
    /// Adds a system to the workload being created.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{EntitiesViewMut, IntoIter, View, ViewMut, Workload, World};
    ///
    /// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
    ///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///         *x += y as usize;
    ///     }
    /// }
    ///
    /// fn check(usizes: View<usize>) {
    ///     let mut iter = usizes.iter();
    ///     assert_eq!(iter.next(), Some(&1));
    ///     assert_eq!(iter.next(), Some(&5));
    ///     assert_eq!(iter.next(), Some(&9));
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity((0usize, 1u32));
    /// world.add_entity((2usize, 3u32));
    /// world.add_entity((4usize, 5u32));
    ///
    /// Workload::builder("Add & Check")
    ///     .with_system(&add)
    ///     .with_system(&check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    #[track_caller]
    pub fn with_system<B, R, S: IntoWorkloadSystem<B, R>>(&mut self, system: S) -> &mut Self {
        self.systems
            .push(system.into_workload_system().unwrap().into());

        self
    }
    /// Adds a failible system to the workload being created.  
    /// The workload's execution will stop if any error is encountered.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{EntitiesViewMut, Get, IntoIter, IntoWithId, View, ViewMut, Workload, World};
    /// use shipyard::error::MissingComponent;
    ///
    /// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
    ///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///         *x += y as usize;
    ///     }
    /// }
    ///
    /// fn check(usizes: View<usize>) -> Result<(), MissingComponent> {
    ///     for (id, i) in usizes.iter().with_id() {
    ///         assert!(usizes.get(id)? == i);
    ///     }
    ///
    ///     Ok(())
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity((0usize, 1u32));
    /// world.add_entity((2usize, 3u32));
    /// world.add_entity((4usize, 5u32));
    ///
    /// Workload::builder("Add & Check")
    ///     .with_system(&add)
    ///     .with_try_system(&check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    #[track_caller]
    #[cfg(feature = "std")]
    pub fn with_try_system<
        B,
        Ok,
        Err: 'static + Send + Error,
        R: Into<Result<Ok, Err>>,
        S: IntoWorkloadSystem<B, R>,
    >(
        &mut self,
        system: S,
    ) -> &mut Self {
        self.systems
            .push(system.into_workload_try_system::<Ok, Err>().unwrap().into());

        self
    }
    #[track_caller]
    #[cfg(not(feature = "std"))]
    pub fn with_try_system<
        B,
        Ok,
        Err: 'static + Send + Any,
        R: Into<Result<Ok, Err>>,
        S: IntoWorkloadSystem<B, R>,
    >(
        &mut self,
        system: S,
    ) -> &mut Self {
        self.systems
            .push(system.into_workload_try_system::<Ok, Err>().unwrap().into());

        self
    }
    /// Nests a workload by adding all its systems.  
    /// This other workload must be present in the `World` by the time `add_to_world` is called.
    pub fn with_workload<W: Into<Cow<'static, str>> + 'static>(
        &mut self,
        workload: W,
    ) -> &mut Self {
        let workload = workload.into();

        self.systems.push(workload.into());

        self
    }
    /// Moves all systems of `other` into `Self`, leaving `other` empty.  
    /// This allows us to collect systems in different builders before joining them together.
    pub fn append(&mut self, other: &mut Self) -> &mut Self {
        self.systems.extend(other.systems.drain(..));

        self
    }
    /// Finishes the workload creation and stores it in the [`World`].  
    /// Returns a struct with describing how the workload has been split in batches.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (exclusive)
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload with an identical name already present.
    /// - Nested workload is not present in `world`.
    ///
    /// [`World`]: crate::World
    #[allow(clippy::blocks_in_if_conditions)]
    pub fn add_to_world(&mut self, world: &World) -> Result<WorkloadInfo, error::AddWorkload> {
        let Scheduler {
            systems,
            system_names,
            system_generators,
            lookup_table,
            workloads,
            default,
        } = &mut *world
            .scheduler
            .borrow_mut()
            .map_err(|_| error::AddWorkload::Borrow)?;

        if workloads.contains_key(&self.name) {
            return Err(error::AddWorkload::AlreadyExists);
        }

        if self.systems.is_empty() {
            if workloads.is_empty() {
                *default = self.name.clone();
            }

            workloads.insert(self.name.clone(), Batches::default());

            Ok(WorkloadInfo {
                name: core::mem::take(&mut self.name),
                batch_info: Vec::new(),
            })
        } else {
            for work_unit in &self.systems {
                if let WorkUnit::Workload(workload) = work_unit {
                    if !workloads.contains_key(workload) {
                        return Err(error::AddWorkload::UnknownWorkload(
                            core::mem::take(&mut self.name),
                            workload.clone(),
                        ));
                    }
                }
            }

            let mut collected_systems: Vec<(TypeId, &'static str, usize, Vec<TypeInfo>)> =
                Vec::with_capacity(self.systems.len());

            for work_unit in self.systems.drain(..) {
                match work_unit {
                    WorkUnit::System(mut system) => {
                        let borrow_constraints = core::mem::take(&mut system.borrow_constraints);
                        let system_type_name = system.system_type_name;
                        let system_type_id = system.system_type_id;

                        let system_index = *lookup_table
                            .entry(system.system_type_id)
                            .or_insert_with(|| {
                                systems.push(system.system_fn);
                                system_names.push(system.system_type_name);
                                system_generators.push(system.generator);
                                systems.len() - 1
                            });

                        collected_systems.push((
                            system_type_id,
                            system_type_name,
                            system_index,
                            borrow_constraints,
                        ));
                    }
                    WorkUnit::Workload(workload) => {
                        for &system_index in &workloads[&workload].sequential {
                            let mut borrow = Vec::new();

                            collected_systems.push((
                                system_generators[system_index](&mut borrow),
                                system_names[system_index],
                                system_index,
                                borrow,
                            ));
                        }
                    }
                }
            }

            if workloads.is_empty() {
                *default = self.name.clone();
            }

            let batches = workloads.entry(self.name.clone()).or_default();

            if collected_systems.len() == 1 {
                let (system_type_id, system_type_name, system_index, borrow_constraints) =
                    collected_systems.pop().unwrap();

                batches.parallel.push(vec![system_index]);
                batches.sequential.push(system_index);

                let batch_info = BatchInfo {
                    systems: vec![SystemInfo {
                        name: system_type_name,
                        type_id: system_type_id,
                        borrow: borrow_constraints,
                        conflict: None,
                    }],
                };

                Ok(WorkloadInfo {
                    name: core::mem::take(&mut self.name),
                    batch_info: vec![batch_info],
                })
            } else {
                let mut workload_info = WorkloadInfo {
                    name: core::mem::take(&mut self.name),
                    batch_info: vec![],
                };

                for (system_type_id, system_type_name, system_index, borrow_constraints) in
                    collected_systems
                {
                    batches.sequential.push(system_index);

                    let mut non_send_sync = None;

                    for type_info in &borrow_constraints {
                        if !type_info.is_send || !type_info.is_sync {
                            non_send_sync = Some(type_info.clone());
                            break;
                        }
                    }

                    if let Some(type_info) = non_send_sync {
                        let conflict = if batches.parallel.is_empty() {
                            None
                        } else {
                            Some(Conflict::NotSendSync(type_info.clone()))
                        };

                        let system_info = SystemInfo {
                            name: system_type_name,
                            type_id: system_type_id,
                            borrow: vec![type_info.clone()],
                            conflict,
                        };

                        batches.parallel.push(vec![system_index]);

                        workload_info.batch_info.push(BatchInfo {
                            systems: vec![system_info],
                        });
                    } else {
                        let mut conflict: Option<Conflict> = None;

                        let mut valid = batches.parallel.len();

                        'batch: for (i, batch_info) in
                            workload_info.batch_info.iter().enumerate().rev()
                        {
                            for other_system in &batch_info.systems {
                                for other_type_info in &other_system.borrow {
                                    for type_info in &borrow_constraints {
                                        match type_info.mutability {
                                            Mutability::Exclusive => {
                                                if !other_type_info.is_send
                                                    || !other_type_info.is_sync
                                                {
                                                    conflict = Some(Conflict::OtherNotSendSync {
                                                        system: SystemId {
                                                            name: other_system.name,
                                                            type_id: other_system.type_id,
                                                        },
                                                        type_info: other_type_info.clone(),
                                                    });

                                                    break 'batch;
                                                }

                                                if type_info.storage_id
                                                    == other_type_info.storage_id
                                                    || type_info.storage_id
                                                        == TypeId::of::<AllStorages>()
                                                    || other_type_info.storage_id
                                                        == TypeId::of::<AllStorages>()
                                                {
                                                    conflict = Some(Conflict::Borrow {
                                                        type_info: type_info.clone(),
                                                        other_system: SystemId {
                                                            name: other_system.name,
                                                            type_id: other_system.type_id,
                                                        },
                                                        other_type_info: other_type_info.clone(),
                                                    });

                                                    break 'batch;
                                                }
                                            }
                                            Mutability::Shared => {
                                                if !other_type_info.is_send
                                                    || !other_type_info.is_sync
                                                {
                                                    conflict = Some(Conflict::OtherNotSendSync {
                                                        system: SystemId {
                                                            name: other_system.name,
                                                            type_id: other_system.type_id,
                                                        },
                                                        type_info: other_type_info.clone(),
                                                    });

                                                    break 'batch;
                                                }

                                                if (type_info.storage_id
                                                    == other_type_info.storage_id
                                                    && other_type_info.mutability
                                                        == Mutability::Exclusive)
                                                    || type_info.storage_id
                                                        == TypeId::of::<AllStorages>()
                                                    || other_type_info.storage_id
                                                        == TypeId::of::<AllStorages>()
                                                {
                                                    conflict = Some(Conflict::Borrow {
                                                        type_info: type_info.clone(),
                                                        other_system: SystemId {
                                                            name: other_system.name,
                                                            type_id: other_system.type_id,
                                                        },
                                                        other_type_info: other_type_info.clone(),
                                                    });

                                                    break 'batch;
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            valid = i;
                        }

                        let system_info = SystemInfo {
                            name: system_type_name,
                            type_id: system_type_id,
                            borrow: borrow_constraints,
                            conflict,
                        };

                        if valid < batches.parallel.len() {
                            batches.parallel[valid].push(system_index);
                            workload_info.batch_info[valid].systems.push(system_info);
                        } else {
                            batches.parallel.push(vec![system_index]);
                            workload_info.batch_info.push(BatchInfo {
                                systems: vec![system_info],
                            });
                        }
                    }
                }

                Ok(workload_info)
            }
        }
    }
}

#[test]
fn single_immutable() {
    use crate::{View, World};

    fn system1(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("System1")
        .with_system(&system1)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("System1"),
        Some(&Batches {
            parallel: vec![vec![0]],
            sequential: vec![0],
        })
    );
    assert_eq!(scheduler.default, "System1");
}

#[test]
fn single_mutable() {
    use crate::{ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("System1")
        .with_system(&system1)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("System1"),
        Some(&Batches {
            parallel: vec![vec![0]],
            sequential: vec![0],
        })
    );
    assert_eq!(scheduler.default, "System1");
}

#[test]
fn multiple_immutable() {
    use crate::{IntoWorkloadSystem, View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system1)
        .with_system(system2.into_workload_system().unwrap())
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0, 1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn multiple_mutable() {
    use crate::{ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system1)
        .with_system(&system2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1],
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn multiple_mixed() {
    use crate::{View, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system1)
        .with_system(&system2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system2)
        .with_system(&system1)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn append_optimizes_batches() {
    use crate::{View, ViewMut, World};

    fn system_a1(_: View<'_, usize>, _: ViewMut<'_, u32>) {}
    fn system_a2(_: View<'_, usize>, _: ViewMut<'_, u32>) {}
    fn system_b1(_: View<'_, usize>) {}

    let world = World::new();

    let mut group_a = Workload::builder("Group A");
    group_a.with_system(&system_a1).with_system(&system_a2);

    let mut group_b = Workload::builder("Group B");
    group_b.with_system(&system_b1);

    Workload::builder("Combined")
        .append(&mut group_a)
        .append(&mut group_b)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 3);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Combined"),
        Some(&Batches {
            parallel: vec![vec![0, 2], vec![1]],
            sequential: vec![0, 1, 2]
        })
    );
    assert_eq!(scheduler.default, "Combined");
}

#[test]
fn all_storages() {
    use crate::{AllStoragesViewMut, View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: AllStoragesViewMut<'_>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0]],
            sequential: vec![0]
        })
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system2)
        .with_system(&system2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![0]],
            sequential: vec![0, 0]
        })
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system1)
        .with_system(&system2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system2)
        .with_system(&system1)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::{NonSend, View, ViewMut, World};

    struct NotSend(*const ());
    unsafe impl Sync for NotSend {}

    fn sys1(_: NonSend<View<'_, NotSend>>) {}
    fn sys2(_: NonSend<ViewMut<'_, NotSend>>) {}
    fn sys3(_: View<'_, usize>) {}
    fn sys4(_: ViewMut<'_, usize>) {}

    let world = World::new();

    let info = Workload::builder("Test")
        .with_system(&sys1)
        .with_system(&sys1)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&Batches {
            parallel: vec![vec![0], vec![0]],
            sequential: vec![0, 0]
        })
    );
    assert_eq!(scheduler.default, "Test");
    assert!(info.batch_info[0].systems[0].conflict.is_none());

    let world = World::new();

    Workload::builder("Test")
        .with_system(&sys1)
        .with_system(&sys2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Test");

    let world = World::new();

    Workload::builder("Test")
        .with_system(&sys2)
        .with_system(&sys1)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Test");

    let world = World::new();

    let info = Workload::builder("Test")
        .with_system(&sys1)
        .with_system(&sys3)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Test");
    assert!(info.batch_info[0].systems[0].conflict.is_none());

    let world = World::new();

    Workload::builder("Test")
        .with_system(&sys1)
        .with_system(&sys4)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Test");
}

#[test]
fn unique_and_non_unique() {
    use crate::{UniqueViewMut, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: UniqueViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(&system1)
        .with_system(&system2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0, 1]],
            sequential: vec![0, 1]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn empty_workload() {
    use crate::World;

    let world = World::new();

    Workload::builder("Systems").add_to_world(&world).unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 0);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![],
            sequential: vec![]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn append_ensures_multiple_batches_can_be_optimized_over() {
    use crate::{View, ViewMut, World};

    fn sys_a1(_: ViewMut<'_, usize>, _: ViewMut<'_, u32>) {}
    fn sys_a2(_: View<'_, usize>, _: ViewMut<'_, u32>) {}
    fn sys_b1(_: View<'_, usize>) {}
    fn sys_c1(_: View<'_, u16>) {}

    let world = World::new();

    let mut group_a = Workload::builder("Group A");
    group_a.with_system(&sys_a1).with_system(&sys_a2);
    let mut group_b = Workload::builder("Group B");
    group_b.with_system(&sys_b1);
    let mut group_c = Workload::builder("Group C");
    group_c.with_system(&sys_c1);

    Workload::builder("Combined")
        .append(&mut group_a)
        .append(&mut group_b)
        .append(&mut group_c)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 4);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Combined"),
        Some(&Batches {
            parallel: vec![vec![0, 3], vec![1, 2]],
            sequential: vec![0, 1, 2, 3]
        })
    );
    assert_eq!(scheduler.default, "Combined");
}

#[test]
fn workload_flattening() {
    use crate::{View, ViewMut, World};

    fn sys1(_: View<'_, u32>) {}
    fn sys2(_: ViewMut<'_, u32>) {}

    let world = World::new();

    Workload::builder("1")
        .with_system(&sys1)
        .with_system(&sys2)
        .with_system(&sys1)
        .add_to_world(&world)
        .unwrap();

    let debug_info = Workload::builder("2")
        .with_workload("1")
        .with_system(&sys1)
        .with_workload("1")
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(debug_info.batch_info.len(), 5);
}

#[test]
fn empty_workload_flattening() {
    use crate::World;

    let world = World::new();

    Workload::builder("1").add_to_world(&world).unwrap();

    let debug_info = Workload::builder("2")
        .with_workload("1")
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 0);
    assert_eq!(debug_info.batch_info.len(), 0);
}
