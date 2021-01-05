use super::info::{BatchInfo, Conflict, SystemId, SystemInfo, TypeInfo, WorkloadInfo};
use super::{Batches, Scheduler, WorkloadSystem};
use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::error;
use crate::storage::StorageId;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::borrow::Cow;
// this is the macro, not the module
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
use core::iter::Extend;

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

/// Keeps information to create a workload.
///
/// A workload is a collection of systems. They will execute as much in parallel as possible.  
/// They are evaluated first to last when they can't be parallelized.  
/// The default workload will automatically be set to the first workload added.
#[derive(Default)]
pub struct WorkloadBuilder {
    pub(super) systems: Vec<WorkloadSystem>,
    name: Cow<'static, str>,
}

impl WorkloadBuilder {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: crate::WorkloadBuilder
    ///
    /// ### Example
    /// ```
    /// use shipyard::{system, EntitiesViewMut, IntoIter, View, ViewMut, Workload, World};
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
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///         entities.add_entity((&mut usizes, &mut u32s), (2, 3));
    ///         entities.add_entity((&mut usizes, &mut u32s), (4, 5));
    ///     },
    /// );
    ///
    /// Workload::builder("Add & Check")
    ///     .with_system(system!(add))
    ///     .with_system(system!(check))
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
    /// It is recommended to use the [system] and [try_system] macros.  
    /// If the two functions in the tuple don't match, the workload could fail to run every time.  
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{system, EntitiesViewMut, IntoIter, View, ViewMut, Workload, World};
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
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///         entities.add_entity((&mut usizes, &mut u32s), (2, 3));
    ///         entities.add_entity((&mut usizes, &mut u32s), (4, 5));
    ///     },
    /// );
    ///
    /// Workload::builder("Add & Check")
    ///     .with_system(system!(add))
    ///     .with_system(system!(check))
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    ///
    /// [system]: crate::system!
    /// [try_system]: crate::try_system!
    #[inline]
    pub fn with_system(&mut self, system: WorkloadSystem) -> &mut Self {
        self.systems.push(system);

        self
    }
    /// Moves all systems of `other` into `Self`, leaving `other` empty.  
    /// This allows us to collect systems in different builders before joining them together.
    pub fn append(&mut self, other: &mut Self) -> &mut Self {
        self.systems.extend(other.systems.drain(..));

        self
    }
    /// Finishes the workload creation and stores it in the [`World`].
    ///
    /// ### Borrows
    ///
    /// - Scheduler (exclusive)
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload with an identical name already present.
    ///
    /// [`World`]: crate::World
    pub fn add_to_world(&mut self, world: &World) -> Result<(), error::AddWorkload> {
        self.add_to_world_with_info(world).map(drop)
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
    ///
    /// [`World`]: crate::World
    #[allow(clippy::blocks_in_if_conditions)]
    pub fn add_to_world_with_info(
        &mut self,
        world: &World,
    ) -> Result<WorkloadInfo, error::AddWorkload> {
        let mut scheduler = world
            .scheduler
            .try_borrow_mut()
            .map_err(|_| error::AddWorkload::Borrow)?;

        let Scheduler {
            systems,
            system_names,
            lookup_table,
            workloads,
            default,
        } = &mut *scheduler;

        let mut workload_info;

        if self.systems.is_empty() {
            // if the workload doesn't have systems we just register it with no batch
            // and register the batch info
            let is_empty = workloads.is_empty();
            match workloads.entry(self.name.clone()) {
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    if is_empty {
                        *default = entry.key().clone();
                    }

                    entry.insert(Batches::default());
                }
                hashbrown::hash_map::Entry::Occupied(_) => {
                    return Err(error::AddWorkload::AlreadyExists);
                }
            };

            workload_info = WorkloadInfo {
                name: core::mem::take(&mut self.name),
                batch_info: Vec::new(),
            };
        } else if self.systems.len() == 1 {
            // with a single system there is just one batch configuration possible
            let is_empty = workloads.is_empty();
            let batches = match workloads.entry(self.name.clone()) {
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    if is_empty {
                        *default = entry.key().clone();
                    }

                    entry.insert(Batches::default())
                }
                hashbrown::hash_map::Entry::Occupied(_) => {
                    return Err(error::AddWorkload::AlreadyExists);
                }
            };

            let WorkloadSystem {
                borrow_constraints,
                system_fn,
                system_type_id,
                system_type_name,
            } = self.systems.pop().unwrap();

            let system_index = *lookup_table.entry(system_type_id).or_insert_with(|| {
                systems.push(system_fn);
                system_names.push(system_type_name);
                systems.len() - 1
            });

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

            workload_info = WorkloadInfo {
                name: core::mem::take(&mut self.name),
                batch_info: vec![batch_info],
            };
        } else {
            // with multiple systems we have to create batches
            // a system can't be added to a batch with a conflicting borrow and can't jump over a conflicting borrow either
            // systems borrowing !Send and !Sync types are currently always scheduled on their own to make them run on World's thread
            let is_empty = workloads.is_empty();
            let batches = match workloads.entry(self.name.clone()) {
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    if is_empty {
                        *default = entry.key().clone();
                    }

                    entry.insert(Batches::default())
                }
                hashbrown::hash_map::Entry::Occupied(_) => {
                    return Err(error::AddWorkload::AlreadyExists);
                }
            };

            workload_info = WorkloadInfo {
                name: core::mem::take(&mut self.name),
                batch_info: vec![],
            };

            for WorkloadSystem {
                borrow_constraints,
                system_fn,
                system_type_id,
                system_type_name,
            } in self.systems.drain(..)
            {
                let system_index = *lookup_table.entry(system_type_id).or_insert_with(|| {
                    systems.push(system_fn);
                    system_names.push(system_type_name);
                    systems.len() - 1
                });

                batches.sequential.push(system_index);

                if borrow_constraints
                    .iter()
                    .fold(true, |is_send_sync, type_info| {
                        is_send_sync && type_info.is_send && type_info.is_sync
                    })
                {
                    let mut conflict: Option<Conflict> = None;

                    let mut valid = batches.parallel.len();

                    'batch: for (i, batch_info) in workload_info.batch_info.iter().enumerate().rev()
                    {
                        for system in &batch_info.systems {
                            for system_type_info in system.borrow.iter() {
                                for type_info in borrow_constraints.iter() {
                                    match type_info.mutability {
                                        Mutability::Exclusive => {
                                            if type_info.storage_id == system_type_info.storage_id
                                                || type_info.storage_id
                                                    == TypeId::of::<AllStorages>()
                                                || system_type_info.storage_id
                                                    == TypeId::of::<AllStorages>()
                                            {
                                                conflict = Some(Conflict::Borrow {
                                                    type_info: type_info.clone(),
                                                    system: SystemId {
                                                        name: system.name,
                                                        type_id: system.type_id,
                                                    },
                                                });

                                                break 'batch;
                                            }
                                        }
                                        Mutability::Shared => {
                                            if (type_info.storage_id == system_type_info.storage_id
                                                && system_type_info.mutability
                                                    == Mutability::Exclusive)
                                                || type_info.storage_id
                                                    == TypeId::of::<AllStorages>()
                                                || system_type_info.storage_id
                                                    == TypeId::of::<AllStorages>()
                                            {
                                                conflict = Some(Conflict::Borrow {
                                                    type_info: type_info.clone(),
                                                    system: SystemId {
                                                        name: system.name,
                                                        type_id: system.type_id,
                                                    },
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
                } else {
                    let system_info = SystemInfo {
                        name: system_type_name,
                        type_id: system_type_id,
                        borrow: vec![TypeInfo {
                            name: type_name::<AllStorages>(),
                            mutability: Mutability::Exclusive,
                            storage_id: StorageId::of::<AllStorages>(),
                            is_send: true,
                            is_sync: true,
                        }],
                        conflict: Some(Conflict::NotSendSync),
                    };

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

#[test]
fn single_immutable() {
    use crate::{system, View, World};

    fn system1(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("System1")
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("System1")
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system1))
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system1))
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, View, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system1))
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(system2))
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, View, ViewMut, World};

    fn system_a1(_: View<'_, usize>, _: ViewMut<'_, u32>) {}
    fn system_a2(_: View<'_, usize>, _: ViewMut<'_, u32>) {}
    fn system_b1(_: View<'_, usize>) {}

    let world = World::new();

    let mut group_a = Workload::builder("Group A");
    group_a
        .with_system(system!(system_a1))
        .with_system(system!(system_a2));

    let mut group_b = Workload::builder("Group B");
    group_b.with_system(system!(system_b1));

    Workload::builder("Combined")
        .append(&mut group_a)
        .append(&mut group_b)
        .add_to_world_with_info(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, AllStoragesViewMut, View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: AllStoragesViewMut<'_>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(system2))
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(system1))
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(system2))
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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

#[cfg(feature = "non_send")]
#[test]
fn non_send() {
    use crate::{system, NonSend, View, ViewMut, World};

    struct NotSend(*const ());
    unsafe impl Sync for NotSend {}

    fn sys1(_: NonSend<View<'_, NotSend>>) {}
    fn sys2(_: NonSend<ViewMut<'_, NotSend>>) {}
    fn sys3(_: View<'_, usize>) {}
    fn sys4(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Test")
        .with_system(system!(sys1))
        .with_system(system!(sys1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(sys1))
        .with_system(system!(sys2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(sys2))
        .with_system(system!(sys1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(sys1))
        .with_system(system!(sys3))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
        .with_system(system!(sys1))
        .with_system(system!(sys4))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
fn fake_borrow() {
    use crate::{system, FakeBorrow, SparseSet, View, World};

    fn system1(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system1))
        .with_system(system!(|_: FakeBorrow<SparseSet<usize>>| {}))
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 3);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0], vec![1], vec![2]],
            sequential: vec![0, 1, 2]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn unique_fake_borrow() {
    use crate::{system, FakeBorrow, Unique, UniqueView, View, World};

    fn system1(_: UniqueView<'_, usize>, _: View<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system1))
        .with_system(system!(system2))
        .with_system(system!(|_: FakeBorrow<Unique<usize>>| {}))
        .with_system(system!(system2))
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 5);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&Batches {
            parallel: vec![vec![0, 1, 3], vec![2], vec![4]],
            sequential: vec![0, 1, 2, 3, 4]
        })
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn unique_and_non_unique() {
    use crate::{system, UniqueViewMut, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: UniqueViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .with_system(system!(system1))
        .with_system(system!(system2))
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
    use crate::{system, View, ViewMut, World};

    fn sys_a1(_: ViewMut<'_, usize>, _: ViewMut<'_, u32>) {}
    fn sys_a2(_: View<'_, usize>, _: ViewMut<'_, u32>) {}
    fn sys_b1(_: View<'_, usize>) {}
    fn sys_c1(_: View<'_, u16>) {}

    let world = World::new();

    let mut group_a = Workload::builder("Group A");
    group_a
        .with_system(system!(sys_a1))
        .with_system(system!(sys_a2));
    let mut group_b = Workload::builder("Group B");
    group_b.with_system(system!(sys_b1));
    let mut group_c = Workload::builder("Group C");
    group_c.with_system(system!(sys_c1));

    Workload::builder("Combined")
        .append(&mut group_a)
        .append(&mut group_b)
        .append(&mut group_c)
        .add_to_world_with_info(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
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
