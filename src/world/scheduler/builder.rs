use super::info::{BatchInfo, Conflict, SystemId, SystemInfo, TypeInfo, WorkloadInfo};
use super::Scheduler;
use crate::borrow::Mutability;
use crate::error;
use crate::storage::AllStorages;
use crate::system::System;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::borrow::Cow;
use alloc::boxed::Box;
// this is the macro, not the module
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
use core::ops::Range;

/// Used to create a [`WorkloadBuilder`].
///
/// You can also use [`WorkloadBuilder::new`] or [`WorkloadBuilder::default`].
///
/// [`WorkloadBuilder`]: struct.WorkloadBuilder.html
/// [`WorkloadBuilder::new`]: struct.WorkloadBuilder.html#method.new
/// [`WorkloadBuilder::default`]: struct.WorkloadBuilder.html#impl-Default
pub struct Workload;

impl Workload {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: struct.WorkloadBuilder.html
    pub fn builder<N: Into<Cow<'static, str>>>(name: N) -> WorkloadBuilder {
        WorkloadBuilder::new(name)
    }
}

/// Keeps information to create a workload.
///
/// A workload is a collection of systems. They will execute as much in parallel as possible.  
/// They are evaluated first to last when they can't be parallelized.  
/// The default workload will automatically be set to the first workload added.
#[allow(clippy::type_complexity)]
#[derive(Default)]
pub struct WorkloadBuilder {
    systems: Vec<(
        TypeId,
        &'static str,
        Range<usize>,
        bool,
        Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>,
    )>,
    borrow_info: Vec<TypeInfo>,
    name: Cow<'static, str>,
}

impl WorkloadBuilder {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: struct.WorkloadBuilder.html
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
            borrow_info: Vec::new(),
            name: name.into(),
        }
    }
}

impl WorkloadBuilder {
    /// Adds a system to the workload being created.  
    /// It is strongly recommended to use the [system] and [try_system] macros.  
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
    ///     .try_with_system((|world: &World| world.try_run(add), add))
    ///     .unwrap()
    ///     .try_with_system(system!(check))
    ///     .unwrap()
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    ///
    /// [system]: macro.system.html
    /// [try_system]: macro.try_system.html
    pub fn try_with_system<
        'a,
        B,
        R,
        F: System<'a, (), B, R>,
        S: Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static,
    >(
        &mut self,
        (system, _): (S, F),
    ) -> Result<&mut Self, error::InvalidSystem> {
        let old_len = self.borrow_info.len();
        F::borrow_infos(&mut self.borrow_info);

        let borrows = &self.borrow_info[old_len..];

        if borrows.contains(&TypeInfo {
            name: "",
            type_id: TypeId::of::<AllStorages>(),
            mutability: Mutability::Exclusive,
            is_send: true,
            is_sync: true,
        }) && borrows.len() > 1
        {
            return Err(error::InvalidSystem::AllStorages);
        }

        let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

        for a_type_info in &borrows[..mid] {
            for b_type_info in &borrows[mid..] {
                if a_type_info.type_id == b_type_info.type_id {
                    match (a_type_info.mutability, b_type_info.mutability) {
                        (Mutability::Exclusive, Mutability::Exclusive) => {
                            return Err(error::InvalidSystem::MultipleViewsMut)
                        }
                        (Mutability::Exclusive, Mutability::Shared)
                        | (Mutability::Shared, Mutability::Exclusive) => {
                            return Err(error::InvalidSystem::MultipleViews)
                        }
                        (Mutability::Shared, Mutability::Shared) => {}
                    }
                }
            }
        }

        let is_send_sync = F::is_send_sync();
        self.systems.push((
            TypeId::of::<S>(),
            type_name::<F>(),
            old_len..self.borrow_info.len(),
            is_send_sync,
            Box::new(system),
        ));

        Ok(self)
    }
    /// Adds a system to the workload being created.  
    /// It is strongly recommended to use the [system] and [try_system] macros.  
    /// If the two functions in the tuple don't match, the workload could fail to run every time.  
    /// Unwraps errors.
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
    ///     .with_system((|world: &World| world.try_run(add), add))
    ///     .with_system(system!(check))
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    ///
    /// [system]: macro.system.html
    /// [try_system]: macro.try_system.html
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn with_system<
        'a,
        B,
        R,
        F: System<'a, (), B, R>,
        S: Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static,
    >(
        &mut self,
        system: (S, F),
    ) -> &mut Self {
        match self.try_with_system(system) {
            Ok(s) => s,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Moves all systems of `other` into `Self`, leaving `other` empty.  
    /// This allows us to collect systems in different builders before joining them together.
    pub fn append(&mut self, other: &mut Self) -> &mut Self {
        let offset_ranges_by = self.borrow_info.len();
        self.borrow_info.extend(other.borrow_info.drain(..));
        self.systems.extend(other.systems.drain(..).map(
            |(type_id, type_name, mut borrow_info_range, is_send_sync, system_fn)| {
                borrow_info_range.start += offset_ranges_by;
                borrow_info_range.end += offset_ranges_by;
                (
                    type_id,
                    type_name,
                    borrow_info_range,
                    is_send_sync,
                    system_fn,
                )
            },
        ));

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
    /// [`World`]: struct.World.html
    pub fn add_to_world(&mut self, world: &World) -> Result<(), error::AddWorkload> {
        let mut scheduler = world
            .scheduler
            .try_borrow_mut()
            .map_err(|_| error::AddWorkload::Borrow)?;

        if self.systems.is_empty() {
            // if the workload doesn't have systems we just register it with no batch
            if scheduler.workloads.is_empty() {
                scheduler.default = self.name.clone();
                scheduler
                    .workloads
                    .insert(core::mem::take(&mut self.name), Vec::new());
            } else {
                match scheduler.workloads.entry(core::mem::take(&mut self.name)) {
                    hashbrown::hash_map::Entry::Occupied(_) => {
                        return Err(error::AddWorkload::AlreadyExists);
                    }
                    hashbrown::hash_map::Entry::Vacant(entry) => entry.insert(Vec::new()),
                };
            }
        } else if self.systems.len() == 1 {
            // with a single system there is just one batch configuration possible
            let Scheduler {
                systems,
                system_names,
                lookup_table,
                workloads,
                default,
            } = &mut *scheduler;

            let batches;

            if workloads.is_empty() {
                *default = self.name.clone();
                batches = workloads
                    .entry(core::mem::take(&mut self.name))
                    .insert(Vec::new())
                    .into_mut();
            } else {
                match workloads.entry(core::mem::take(&mut self.name)) {
                    hashbrown::hash_map::Entry::Occupied(_) => {
                        return Err(error::AddWorkload::AlreadyExists);
                    }
                    hashbrown::hash_map::Entry::Vacant(entry) => batches = entry.insert(Vec::new()),
                };
            }

            let (type_id, system_name, _, _, system) = self.systems.pop().unwrap();

            let system_index = *lookup_table.entry(type_id).or_insert_with(|| {
                systems.push(system);
                system_names.push(system_name);
                systems.len() - 1
            });

            batches.push(vec![system_index]);
        } else {
            // with multiple systems we have to create batches
            // a system can't be added to a batch older than the last one
            // systems borrowing !Send and !Sync types are currently always scheduled on their own to make them run on World's thread
            let Scheduler {
                systems,
                system_names,
                lookup_table,
                workloads,
                default,
            } = &mut *scheduler;

            let batches;

            if workloads.is_empty() {
                *default = self.name.clone();
                batches = workloads
                    .entry(core::mem::take(&mut self.name))
                    .insert(Vec::new())
                    .into_mut();
            } else {
                match workloads.entry(core::mem::take(&mut self.name)) {
                    hashbrown::hash_map::Entry::Occupied(_) => {
                        return Err(error::AddWorkload::AlreadyExists);
                    }
                    hashbrown::hash_map::Entry::Vacant(entry) => batches = entry.insert(Vec::new()),
                };
            }

            let mut batches_info: Vec<Vec<_>> = vec![];

            for (system_type_id, system_name, info_range, is_send_sync, system) in
                self.systems.drain(..)
            {
                let system_index = *lookup_table.entry(system_type_id).or_insert_with(|| {
                    systems.push(system);
                    system_names.push(system_name);
                    systems.len() - 1
                });

                if is_send_sync {
                    let mut conflict = false;

                    if let Some(last) = batches_info.last() {
                        'outer: for &(type_id, mutability) in last.iter().rev() {
                            for type_info in &self.borrow_info[info_range.clone()] {
                                match type_info.mutability {
                                    Mutability::Exclusive => {
                                        if type_info.type_id == type_id
                                            || type_info.type_id == TypeId::of::<AllStorages>()
                                            || type_id == TypeId::of::<AllStorages>()
                                        {
                                            conflict = true;
                                            break 'outer;
                                        }
                                    }
                                    Mutability::Shared => {
                                        if (type_info.type_id == type_id
                                            && mutability == Mutability::Exclusive)
                                            || type_info.type_id == TypeId::of::<AllStorages>()
                                            || type_id == TypeId::of::<AllStorages>()
                                        {
                                            conflict = true;
                                            break 'outer;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if conflict || batches.is_empty() {
                        batches.push(vec![system_index]);
                        batches_info.push(
                            self.borrow_info[info_range]
                                .iter()
                                .map(|type_info| (type_info.type_id, type_info.mutability))
                                .collect(),
                        );
                    } else {
                        batches.last_mut().unwrap().push(system_index);
                        batches_info.last_mut().unwrap().extend(
                            self.borrow_info[info_range]
                                .iter()
                                .map(|type_info| (type_info.type_id, type_info.mutability)),
                        );
                    }
                } else {
                    batches.push(vec![system_index]);
                    batches_info.push(vec![(TypeId::of::<AllStorages>(), Mutability::Exclusive)]);
                }
            }
        }

        Ok(())
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
    /// [`World`]: struct.World.html
    pub fn add_to_world_with_info(
        &mut self,
        world: &World,
    ) -> Result<WorkloadInfo, error::AddWorkload> {
        let mut scheduler = world
            .scheduler
            .try_borrow_mut()
            .map_err(|_| error::AddWorkload::Borrow)?;

        let mut workload_info;

        if self.systems.is_empty() {
            // if the workload doesn't have systems we just register it with no batch
            // and register the batch info
            if scheduler.workloads.is_empty() {
                scheduler.default = self.name.clone();
                scheduler
                    .workloads
                    .insert(core::mem::take(&mut self.name), Vec::new());
            } else {
                match scheduler.workloads.entry(core::mem::take(&mut self.name)) {
                    hashbrown::hash_map::Entry::Occupied(_) => {
                        return Err(error::AddWorkload::AlreadyExists);
                    }
                    hashbrown::hash_map::Entry::Vacant(entry) => entry.insert(Vec::new()),
                };
            }

            workload_info = WorkloadInfo {
                name: core::mem::take(&mut self.name),
                batch_info: Vec::new(),
            };
        } else if self.systems.len() == 1 {
            // with a single system there is just one batch configuration possible
            let Scheduler {
                systems,
                system_names,
                lookup_table,
                workloads,
                default,
            } = &mut *scheduler;

            let batches;

            if workloads.is_empty() {
                *default = self.name.clone();
                batches = workloads
                    .entry(core::mem::take(&mut self.name))
                    .insert(Vec::new())
                    .into_mut();
            } else {
                match workloads.entry(core::mem::take(&mut self.name)) {
                    hashbrown::hash_map::Entry::Occupied(_) => {
                        return Err(error::AddWorkload::AlreadyExists);
                    }
                    hashbrown::hash_map::Entry::Vacant(entry) => batches = entry.insert(Vec::new()),
                };
            }

            let (type_id, system_name, _, _, system) = self.systems.pop().unwrap();

            let system_index = *lookup_table.entry(type_id).or_insert_with(|| {
                systems.push(system);
                system_names.push(system_name);
                systems.len() - 1
            });

            batches.push(vec![system_index]);

            let batch_info = BatchInfo {
                systems: vec![SystemInfo {
                    name: system_name,
                    borrow: core::mem::take(&mut self.borrow_info),
                }],
                conflict: None,
            };

            workload_info = WorkloadInfo {
                name: core::mem::take(&mut self.name),
                batch_info: vec![batch_info],
            };
        } else {
            // with multiple systems we have to create batches
            // a system can't be added to a batch older than the last one
            // systems borrowing !Send and !Sync types are currently always scheduled on their own to make them run on World's thread
            let Scheduler {
                systems,
                system_names,
                lookup_table,
                workloads,
                default,
            } = &mut *scheduler;

            let batches;

            if workloads.is_empty() {
                *default = self.name.clone();
                batches = workloads
                    .entry(self.name.clone())
                    .insert(Vec::new())
                    .into_mut();
            } else {
                match workloads.entry(self.name.clone()) {
                    hashbrown::hash_map::Entry::Occupied(_) => {
                        return Err(error::AddWorkload::AlreadyExists);
                    }
                    hashbrown::hash_map::Entry::Vacant(entry) => batches = entry.insert(Vec::new()),
                };
            }

            workload_info = WorkloadInfo {
                name: self.name.clone(),
                batch_info: vec![],
            };

            for (system_type_id, system_name, info_range, is_send_sync, system) in
                self.systems.drain(..)
            {
                let system_index = *lookup_table.entry(system_type_id).or_insert_with(|| {
                    systems.push(system);
                    system_names.push(system_name);
                    systems.len() - 1
                });

                let system_info = SystemInfo {
                    name: system_name,
                    borrow: self.borrow_info[info_range.clone()].to_vec(),
                };

                if is_send_sync {
                    let mut conflict_info = None;

                    if let Some(batch_info) = workload_info.batch_info.last() {
                        'outer: for system_info in batch_info.systems.iter().rev() {
                            for type_info in &self.borrow_info[info_range.clone()] {
                                match type_info.mutability {
                                    Mutability::Exclusive => {
                                        for batch_type_info in &system_info.borrow {
                                            if type_info.type_id == batch_type_info.type_id
                                                || type_info.type_id == TypeId::of::<AllStorages>()
                                                || batch_type_info.type_id
                                                    == TypeId::of::<AllStorages>()
                                            {
                                                conflict_info = Some(Conflict::Borrow {
                                                    type_info: type_info.clone(),
                                                    system: SystemId {
                                                        name: system_info.name,
                                                        type_id: TypeId::of::<()>(),
                                                    },
                                                });

                                                break 'outer;
                                            }
                                        }
                                    }
                                    Mutability::Shared => {
                                        for batch_type_info in &system_info.borrow {
                                            if (type_info.type_id == batch_type_info.type_id
                                                && batch_type_info.mutability
                                                    == Mutability::Exclusive)
                                                || type_info.type_id == TypeId::of::<AllStorages>()
                                                || batch_type_info.type_id
                                                    == TypeId::of::<AllStorages>()
                                            {
                                                conflict_info = Some(Conflict::Borrow {
                                                    type_info: type_info.clone(),
                                                    system: SystemId {
                                                        name: system_info.name,
                                                        type_id: TypeId::of::<()>(),
                                                    },
                                                });

                                                break 'outer;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if conflict_info.is_some() || batches.is_empty() {
                        batches.push(vec![system_index]);
                        workload_info.batch_info.push(BatchInfo {
                            systems: vec![system_info],
                            conflict: conflict_info,
                        });
                    } else {
                        batches.last_mut().unwrap().push(system_index);
                        workload_info
                            .batch_info
                            .last_mut()
                            .unwrap()
                            .systems
                            .push(system_info);
                    }
                } else {
                    batches.push(vec![system_index]);
                    workload_info.batch_info.push(BatchInfo {
                        systems: vec![system_info],
                        conflict: Some(Conflict::NotSendSync),
                    });
                }
            }
        }

        Ok(workload_info)
    }
}

#[test]
fn single_immutable() {
    use crate::{View, World};

    fn system1(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("System1")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&vec![vec![0]]));
    assert_eq!(scheduler.default, "System1");
}

#[test]
fn single_mutable() {
    use crate::{ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("System1")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&vec![vec![0]]));
    assert_eq!(scheduler.default, "System1");
}

#[test]
fn multiple_immutable() {
    use crate::{View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&vec![vec![0, 1]]));
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn multiple_mutable() {
    use crate::{ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: ViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1]])
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
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn all_storages() {
    use crate::{AllStoragesViewMut, View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: AllStoragesViewMut<'_>) {}

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&vec![vec![0]]));
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Systems");

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Systems");
}

#[cfg(feature = "non_send")]
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

    Workload::builder("Test")
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Test");

    let world = World::new();

    Workload::builder("Test")
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys2), sys2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Test");

    let world = World::new();

    Workload::builder("Test")
        .try_with_system((|world: &World| world.try_run(sys2), sys2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Test");

    let world = World::new();

    Workload::builder("Test")
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys3), sys3))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Test");

    let world = World::new();

    Workload::builder("Test")
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys4), sys4))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Test"),
        Some(&vec![vec![0], vec![1]])
    );
    assert_eq!(scheduler.default, "Test");
}

#[test]
fn fake_borrow() {
    use crate::{FakeBorrow, View, World};

    fn system1(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((
            |world: &World| world.try_run(|| {}),
            |_: FakeBorrow<usize>| {},
        ))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 3);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0], vec![1], vec![2]])
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn unique_fake_borrow() {
    use crate::{FakeBorrow, Unique, UniqueView, View, World};

    fn system1(_: UniqueView<'_, usize>, _: View<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((
            |world: &World| world.try_run(|| {}),
            |_: FakeBorrow<Unique<usize>>| {},
        ))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 5);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(
        scheduler.workloads.get("Systems"),
        Some(&vec![vec![0, 1], vec![2, 3], vec![4]])
    );
    assert_eq!(scheduler.default, "Systems");
}

#[test]
fn unique_and_non_unique() {
    use crate::{UniqueViewMut, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: UniqueViewMut<'_, usize>) {}

    let world = World::new();

    Workload::builder("Systems")
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&vec![vec![0, 1]]));
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
    assert_eq!(scheduler.workloads.get("Systems"), Some(&vec![]));
    assert_eq!(scheduler.default, "Systems");
}
