use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::scheduler::info::{
    BatchInfo, Conflict, Requirements, SystemId, SystemInfo, TypeInfo, WorkloadInfo,
};
use crate::scheduler::{AsLabel, Batches, Label, Scheduler, WorkloadSystem};
use crate::type_id::TypeId;
use crate::world::World;
use crate::{
    error, track, AllStoragesView, Component, IntoWorkload, IntoWorkloadSystem, SparseSet, Unique,
    UniqueStorage,
};
// this is the macro, not the module
use crate::storage::StorageId;
use alloc::boxed::Box;
// macro not module
use alloc::vec;
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::any::Any;
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::error::Error;

/// Used to create a [`Workload`].
///
/// You can also use [`Workload::new`].
///
/// [`Workload`]: crate::Workload
/// [`Workload::new`]: crate::Workload::new()
pub struct ScheduledWorkload {
    name: Box<dyn Label>,
    #[allow(clippy::type_complexity)]
    systems: Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: Vec<&'static str>,
    #[allow(unused)]
    system_generators: Vec<fn(&mut Vec<TypeInfo>) -> TypeId>,
    // system's `TypeId` to an index into both systems and system_names
    #[allow(unused)]
    lookup_table: HashMap<TypeId, usize>,
    /// workload name to list of "batches"
    workloads: HashMap<Box<dyn Label>, Batches>,
}

impl ScheduledWorkload {
    /// Runs the workload.
    ///
    /// ### Borrows
    ///
    /// - Systems' borrow as they are executed
    ///
    /// ### Errors
    ///
    /// - Storage borrow failed.
    /// - User error returned by system.
    pub fn run_with_world(&self, world: &World) -> Result<(), error::RunWorkload> {
        world.run_batches(
            &self.systems,
            &self.system_names,
            &self.workloads[&self.name],
            #[cfg(feature = "tracing")]
            &self.name,
        )
    }
}

pub(super) enum WorkUnit {
    System(WorkloadSystem),
    WorkloadName(Box<dyn Label>),
}

impl From<WorkloadSystem> for WorkUnit {
    fn from(system: WorkloadSystem) -> Self {
        WorkUnit::System(system)
    }
}

impl From<Box<dyn Label>> for WorkUnit {
    fn from(workload: Box<dyn Label>) -> Self {
        WorkUnit::WorkloadName(workload)
    }
}

impl World {
    /// Creates a new workload and store it in the [`World`](crate::World).
    pub fn add_workload<Views, R, W, F: Fn() -> W + 'static>(&self, workload: F)
    where
        W: IntoWorkload<Views, R>,
    {
        let w = workload().into_workload();

        Workload {
            work_units: w.work_units,
            name: Box::new(TypeId::of::<F>()),
            skip_if: Vec::new(),
            before: w.before,
            after: w.after,
        }
        .add_to_world(self)
        .unwrap();
    }
}

/// Keeps information to create a workload.
///
/// A workload is a collection of systems. They will execute as much in parallel as possible.  
/// They are evaluated first to last when they can't be parallelized.  
/// The default workload will automatically be set to the first workload added.
pub struct Workload {
    #[allow(unused)]
    pub(super) name: Box<dyn Label>,
    pub(super) work_units: Vec<WorkUnit>,
    #[allow(unused)]
    pub(super) skip_if: Vec<Box<dyn Fn(AllStoragesView<'_>) -> bool + Send + Sync + 'static>>,
    pub(super) before: Requirements,
    pub(super) after: Requirements,
}

impl Workload {
    /// Creates a new empty [`Workload`].
    ///
    /// [`Workload`]: crate::Workload
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, IntoIter, View, ViewMut, Workload, World};
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// fn add(mut usizes: ViewMut<USIZE>, u32s: View<U32>) {
    ///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///         x.0 += y.0 as usize;
    ///     }
    /// }
    ///
    /// fn check(usizes: View<USIZE>) {
    ///     let mut iter = usizes.iter();
    ///     assert_eq!(iter.next(), Some(&USIZE(1)));
    ///     assert_eq!(iter.next(), Some(&USIZE(5)));
    ///     assert_eq!(iter.next(), Some(&USIZE(9)));
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity((USIZE(0), U32(1)));
    /// world.add_entity((USIZE(2), U32(3)));
    /// world.add_entity((USIZE(4), U32(5)));
    ///
    /// Workload::new("Add & Check")
    ///     .with_system(add)
    ///     .with_system(check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    pub fn new<L: Label>(label: L) -> Self {
        Workload {
            work_units: Vec::new(),
            name: Box::new(label),
            skip_if: Vec::new(),
            before: Requirements::new(),
            after: Requirements::new(),
        }
    }
    /// Moves all systems of `other` into `Self`, leaving `other` empty.  
    /// This allows us to collect systems in different builders before joining them together.
    pub fn append(mut self, other: &mut Self) -> Self {
        self.work_units.append(&mut other.work_units);

        self
    }
    /// Nests a workload by adding all its systems.  
    /// This other workload must be present in the `World` by the time `add_to_world` is called.
    pub fn with_workload<W: Label>(mut self, workload: W) -> Self {
        let workload: Box<dyn Label> = Box::new(workload);

        self.work_units.push(workload.into());

        self
    }
    /// Adds a system to the workload being created.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, IntoIter, View, ViewMut, Workload, World};
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// fn add(mut usizes: ViewMut<USIZE>, u32s: View<U32>) {
    ///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///         x.0 += y.0 as usize;
    ///     }
    /// }
    ///
    /// fn check(usizes: View<USIZE>) {
    ///     let mut iter = usizes.iter();
    ///     assert_eq!(iter.next(), Some(&USIZE(1)));
    ///     assert_eq!(iter.next(), Some(&USIZE(5)));
    ///     assert_eq!(iter.next(), Some(&USIZE(9)));
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity((USIZE(0), U32(1)));
    /// world.add_entity((USIZE(2), U32(3)));
    /// world.add_entity((USIZE(4), U32(5)));
    ///
    /// Workload::new("Add & Check")
    ///     .with_system(add)
    ///     .with_system(check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    #[track_caller]
    pub fn with_system<B, R, S: IntoWorkloadSystem<B, R>>(mut self, system: S) -> Self {
        self.work_units
            .push(system.into_workload_system().unwrap().into());

        self
    }
    /// Adds a fallible system to the workload being created.  
    /// The workload's execution will stop if any error is encountered.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, Get, IntoIter, IntoWithId, View, ViewMut, Workload, World};
    /// use shipyard::error::MissingComponent;
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// fn add(mut usizes: ViewMut<USIZE>, u32s: View<U32>) {
    ///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///         x.0 += y.0 as usize;
    ///     }
    /// }
    ///
    /// fn check(usizes: View<USIZE>) -> Result<(), MissingComponent> {
    ///     for (id, i) in usizes.iter().with_id() {
    ///         assert!(usizes.get(id)? == i);
    ///     }
    ///
    ///     Ok(())
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity((USIZE(0), U32(1)));
    /// world.add_entity((USIZE(2), U32(3)));
    /// world.add_entity((USIZE(4), U32(5)));
    ///
    /// Workload::new("Add & Check")
    ///     .with_system(add)
    ///     .with_try_system(check)
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
        Err: 'static + Into<Box<dyn Error + Send + Sync>>,
        R: Into<Result<Ok, Err>>,
        S: IntoWorkloadSystem<B, R>,
    >(
        mut self,
        system: S,
    ) -> Self {
        self.work_units
            .push(system.into_workload_try_system::<Ok, Err>().unwrap().into());

        self
    }
    /// Adds a fallible system to the workload being created.  
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
    /// Workload::new("Add & Check")
    ///     .with_system(add)
    ///     .with_try_system(check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_default();
    /// ```
    #[track_caller]
    #[cfg(not(feature = "std"))]
    pub fn with_try_system<
        B,
        Ok,
        Err: 'static + Send + Any,
        R: Into<Result<Ok, Err>>,
        S: IntoWorkloadSystem<B, R>,
    >(
        mut self,
        system: S,
    ) -> Self {
        self.work_units
            .push(system.into_workload_try_system::<Ok, Err>().unwrap().into());

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
    pub fn add_to_world(self, world: &World) -> Result<WorkloadInfo, error::AddWorkload> {
        let Scheduler {
            systems,
            system_names,
            system_generators,
            system_labels,
            system_before,
            system_after,
            lookup_table,
            workloads,
            default,
        } = &mut *world
            .scheduler
            .borrow_mut()
            .map_err(|_| error::AddWorkload::Borrow)?;

        create_workload(
            self,
            systems,
            system_names,
            system_generators,
            system_labels,
            system_before,
            system_after,
            lookup_table,
            workloads,
            default,
        )
    }
    /// Returns the first [`Unique`] storage borrowed by this workload that is not present in `world`.\
    /// If the workload contains nested workloads they have to be present in the `World`.
    ///
    /// ### Borrows
    ///
    /// - AllStorages (shared)
    /// - Scheduler (shared)
    pub fn are_all_uniques_present_in_world(
        &self,
        world: &World,
    ) -> Result<(), error::UniquePresence> {
        struct ComponentType;

        impl Component for ComponentType {
            type Tracking = track::Untracked;
        }
        impl Unique for ComponentType {
            type Tracking = track::Untracked;
        }

        let all_storages = world
            .all_storages
            .borrow()
            .map_err(|_| error::UniquePresence::AllStorages)?;
        let storages = all_storages.storages.read();
        let scheduler = world
            .scheduler
            .borrow()
            .map_err(|_| error::UniquePresence::Scheduler)?;

        let unique_name = core::any::type_name::<UniqueStorage<ComponentType>>()
            .split_once('<')
            .unwrap()
            .0;
        let mut type_infos = Vec::new();

        for work_unit in &self.work_units {
            if let Some(value) = check_uniques_in_work_unit(
                work_unit,
                unique_name,
                &storages,
                &scheduler,
                &mut type_infos,
            ) {
                return value;
            }
        }

        for type_info in type_infos {
            if type_info.name.starts_with(unique_name)
                && !storages.contains_key(&type_info.storage_id)
            {
                return Err(error::UniquePresence::Unique(type_info));
            }
        }

        Ok(())
    }
    /// Build the [`Workload`](super::Workload) from the [`Workload`].
    pub fn build(self) -> Result<(ScheduledWorkload, WorkloadInfo), error::AddWorkload> {
        let mut workload = ScheduledWorkload {
            name: self.name.clone(),
            systems: Vec::new(),
            system_names: Vec::new(),
            system_generators: Vec::new(),
            lookup_table: HashMap::new(),
            workloads: HashMap::new(),
        };

        let mut default: Box<dyn Label> = Box::new("");

        let workload_info = create_workload(
            self,
            &mut workload.systems,
            &mut workload.system_names,
            &mut workload.system_generators,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut workload.lookup_table,
            &mut workload.workloads,
            &mut default,
        )?;

        Ok((workload, workload_info))
    }
    /// Do not run the workload if the function evaluates to `true`.
    pub fn skip_if<F>(mut self, should_skip: F) -> Self
    where
        F: Fn(AllStoragesView<'_>) -> bool + Send + Sync + 'static,
    {
        self.skip_if.push(Box::new(should_skip));
        self
    }
    /// Do not run the workload if the `T` storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    pub fn skip_if_storage_empty<T: Component>(self) -> Self {
        let storage_id = StorageId::of::<SparseSet<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    /// Do not run the workload if the `T` unique storage is not present in the `World`.
    pub fn skip_if_missing_unique<T: Unique>(self) -> Self {
        let storage_id = StorageId::of::<UniqueStorage<T>>();
        self.skip_if_storage_empty_by_id(storage_id)
    }
    /// Do not run the workload if the storage is empty.
    ///
    /// If the storage is not present it is considered empty.
    /// If the storage is already borrowed, assume it's not empty.
    pub fn skip_if_storage_empty_by_id(self, storage_id: StorageId) -> Self {
        use crate::all_storages::CustomStorageAccess;

        let should_skip = move |all_storages: AllStoragesView<'_>| match all_storages
            .custom_storage_by_id(storage_id)
        {
            Ok(storage) => storage.is_empty(),
            Err(error::GetStorage::MissingStorage { .. }) => true,
            Err(_) => false,
        };

        self.skip_if(should_skip)
    }
}

fn check_uniques_in_work_unit(
    work_unit: &WorkUnit,
    unique_name: &str,
    storages: &HashMap<StorageId, crate::storage::SBox>,
    scheduler: &Scheduler,
    type_infos: &mut Vec<TypeInfo>,
) -> Option<Result<(), error::UniquePresence>> {
    match work_unit {
        WorkUnit::System(WorkloadSystem::System {
            borrow_constraints, ..
        }) => {
            for type_info in borrow_constraints {
                if type_info.name.starts_with(unique_name)
                    && !storages.contains_key(&type_info.storage_id)
                {
                    return Some(Err(error::UniquePresence::Unique(type_info.clone())));
                }
            }
        }
        WorkUnit::WorkloadName(workload) => {
            if let Some(workload) = scheduler.workloads.get(workload) {
                for system_index in &workload.sequential {
                    scheduler.system_generators[*system_index](type_infos);
                }
            } else {
                return Some(Err(error::UniquePresence::Workload(workload.clone())));
            }
        }
        WorkUnit::System(WorkloadSystem::Workload(workload)) => {
            for wu in &workload.work_units {
                let check =
                    check_uniques_in_work_unit(wu, unique_name, storages, scheduler, type_infos);

                if check.is_some() {
                    return check;
                }
            }
        }
    }

    None
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn create_workload(
    mut builder: Workload,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: &mut Vec<&'static str>,
    system_generators: &mut Vec<fn(&mut Vec<TypeInfo>) -> TypeId>,
    system_labels: &mut HashMap<(Box<dyn Label>, usize), Box<dyn Label>>,
    system_before: &mut HashMap<(Box<dyn Label>, usize), Requirements>,
    system_after: &mut HashMap<(Box<dyn Label>, usize), Requirements>,
    lookup_table: &mut HashMap<TypeId, usize>,
    workloads: &mut HashMap<Box<dyn Label>, Batches>,
    default: &mut Box<dyn Label>,
) -> Result<WorkloadInfo, error::AddWorkload> {
    if workloads.contains_key(&*builder.name) {
        return Err(error::AddWorkload::AlreadyExists);
    }

    for work_unit in &builder.work_units {
        if let WorkUnit::WorkloadName(workload) = work_unit {
            if !workloads.contains_key(&**workload) {
                return Err(error::AddWorkload::UnknownWorkload(
                    builder.name,
                    workload.clone(),
                ));
            }
        }
    }

    let mut collected_systems: Vec<(
        TypeId,
        &'static str,
        usize,
        Vec<TypeInfo>,
        Requirements,
        Requirements,
    )> = Vec::with_capacity(builder.work_units.len());

    for work_unit in builder.work_units.drain(..) {
        flatten_work_unit(
            work_unit,
            systems,
            lookup_table,
            &mut collected_systems,
            workloads,
            system_generators,
            system_names,
            system_before,
            system_after,
        );
    }

    if workloads.is_empty() {
        *default = builder.name.clone();
    }

    let batches = workloads.entry(builder.name.clone()).or_default();

    batches.skip_if = builder.skip_if;

    if collected_systems.len() == 1 {
        let (system_type_id, system_type_name, system_index, borrow_constraints, before, after) =
            collected_systems.pop().unwrap();

        let mut all_storages = None;
        let mut non_send_sync = None;

        for type_info in &borrow_constraints {
            if type_info.storage_id == TypeId::of::<AllStorages>() {
                all_storages = Some(type_info);
                break;
            } else if !type_info.thread_safe {
                non_send_sync = Some(type_info);
                break;
            }
        }

        if all_storages.is_some() || non_send_sync.is_some() {
            batches.parallel.push((Some(system_index), Vec::new()));
        } else {
            batches.parallel.push((None, vec![system_index]));
        }

        batches.sequential.push(system_index);

        if !before.is_empty() {
            system_before.insert((builder.name.clone(), 0), before);
        }

        if !after.is_empty() {
            system_after.insert((builder.name.clone(), 0), after);
        }

        let batch_info = BatchInfo {
            systems: (
                Some(SystemInfo {
                    name: system_type_name,
                    type_id: system_type_id,
                    borrow: borrow_constraints,
                    conflict: None,
                }),
                Vec::new(),
            ),
        };

        return Ok(WorkloadInfo {
            name: builder.name,
            batch_info: vec![batch_info],
        });
    }
    let mut workload_info = WorkloadInfo {
        name: builder.name,
        batch_info: vec![],
    };

    // Extract systems that have before/after requirements as they are not scheduled the same way
    let mut before_or_after_collected_systems = Vec::new();
    for i in (0..collected_systems.len()).rev() {
        if !collected_systems[i].4.is_empty() || !collected_systems[i].5.is_empty() {
            before_or_after_collected_systems.push(collected_systems.remove(i));
        }
    }

    'systems: for (
        system_type_id,
        system_type_name,
        system_index,
        borrow_constraints,
        before,
        after,
    ) in collected_systems
    {
        batches.sequential.push(system_index);

        if !before.is_empty() {
            system_before.insert(
                (workload_info.name.clone(), batches.sequential.len() - 1),
                before,
            );
        }

        if !after.is_empty() {
            system_after.insert(
                (workload_info.name.clone(), batches.sequential.len() - 1),
                after,
            );
        }

        let mut valid = batches.parallel.len();

        let mut all_storages = None;
        let mut non_send_sync = None;

        for type_info in &borrow_constraints {
            if type_info.storage_id == TypeId::of::<AllStorages>() {
                all_storages = Some(type_info.clone());
                break;
            } else if !type_info.thread_safe {
                non_send_sync = Some(type_info.clone());
                break;
            }
        }

        if let Some(all_storages_type_info) = all_storages {
            for (i, batch_info) in workload_info.batch_info.iter().enumerate().rev() {
                match (
                    &batch_info.systems.0,
                    batch_info
                        .systems
                        .1
                        .iter()
                        .rev()
                        .find(|other_system_info| !other_system_info.borrow.is_empty()),
                ) {
                    (None, None) => valid = i,
                    (Some(other_system_info), None)
                    | (None, Some(other_system_info))
                    | (Some(other_system_info), Some(_)) => {
                        let system_info = SystemInfo {
                            name: system_type_name,
                            type_id: system_type_id,
                            borrow: borrow_constraints,
                            conflict: Some(Conflict::Borrow {
                                type_info: Some(all_storages_type_info.clone()),
                                other_system: SystemId {
                                    name: other_system_info.name,
                                    type_id: other_system_info.type_id,
                                },
                                other_type_info: other_system_info.borrow.last().unwrap().clone(),
                            }),
                        };

                        if valid < batches.parallel.len() {
                            batches.parallel[valid].0 = Some(system_index);
                            workload_info.batch_info[valid].systems.0 = Some(system_info);
                        } else {
                            batches.parallel.push((Some(system_index), Vec::new()));
                            workload_info.batch_info.push(BatchInfo {
                                systems: (Some(system_info), Vec::new()),
                            });
                        }

                        continue 'systems;
                    }
                }
            }

            let system_info = SystemInfo {
                name: system_type_name,
                type_id: system_type_id,
                borrow: borrow_constraints,
                conflict: None,
            };

            if valid < batches.parallel.len() {
                batches.parallel[valid].0 = Some(system_index);
                workload_info.batch_info[valid].systems.0 = Some(system_info);
            } else {
                batches.parallel.push((Some(system_index), Vec::new()));
                workload_info.batch_info.push(BatchInfo {
                    systems: (Some(system_info), Vec::new()),
                });
            }
        } else {
            let mut conflict = None;

            'batch: for (i, batch_info) in workload_info.batch_info.iter().enumerate().rev() {
                if let (Some(non_send_sync_type_info), Some(other_system_info)) =
                    (&non_send_sync, &batch_info.systems.0)
                {
                    let system_info = SystemInfo {
                        name: system_type_name,
                        type_id: system_type_id,
                        borrow: borrow_constraints,
                        conflict: Some(Conflict::Borrow {
                            type_info: Some(non_send_sync_type_info.clone()),
                            other_system: SystemId {
                                name: other_system_info.name,
                                type_id: other_system_info.type_id,
                            },
                            other_type_info: other_system_info.borrow.last().unwrap().clone(),
                        }),
                    };

                    if valid < batches.parallel.len() {
                        batches.parallel[valid].0 = Some(system_index);
                        workload_info.batch_info[valid].systems.0 = Some(system_info);
                    } else {
                        batches.parallel.push((Some(system_index), Vec::new()));
                        workload_info.batch_info.push(BatchInfo {
                            systems: (Some(system_info), Vec::new()),
                        });
                    }

                    continue 'systems;
                } else {
                    for other_system in batch_info
                        .systems
                        .0
                        .iter()
                        .chain(batch_info.systems.1.iter())
                    {
                        for other_type_info in &other_system.borrow {
                            for type_info in &borrow_constraints {
                                match type_info.mutability {
                                    Mutability::Exclusive => {
                                        if !type_info.thread_safe && !other_type_info.thread_safe {
                                            conflict = Some(Conflict::OtherNotSendSync {
                                                system: SystemId {
                                                    name: other_system.name,
                                                    type_id: other_system.type_id,
                                                },
                                                type_info: other_type_info.clone(),
                                            });

                                            break 'batch;
                                        }

                                        if type_info.storage_id == other_type_info.storage_id
                                            || type_info.storage_id == TypeId::of::<AllStorages>()
                                            || other_type_info.storage_id
                                                == TypeId::of::<AllStorages>()
                                        {
                                            conflict = Some(Conflict::Borrow {
                                                type_info: Some(type_info.clone()),
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
                                        if !type_info.thread_safe && !other_type_info.thread_safe {
                                            conflict = Some(Conflict::OtherNotSendSync {
                                                system: SystemId {
                                                    name: other_system.name,
                                                    type_id: other_system.type_id,
                                                },
                                                type_info: other_type_info.clone(),
                                            });

                                            break 'batch;
                                        }

                                        if (type_info.storage_id == other_type_info.storage_id
                                            && other_type_info.mutability == Mutability::Exclusive)
                                            || type_info.storage_id == TypeId::of::<AllStorages>()
                                            || other_type_info.storage_id
                                                == TypeId::of::<AllStorages>()
                                        {
                                            conflict = Some(Conflict::Borrow {
                                                type_info: Some(type_info.clone()),
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
            }

            let system_info = SystemInfo {
                name: system_type_name,
                type_id: system_type_id,
                borrow: borrow_constraints,
                conflict,
            };

            if valid < batches.parallel.len() {
                if non_send_sync.is_some() {
                    batches.parallel[valid].0 = Some(system_index);
                    workload_info.batch_info[valid].systems.0 = Some(system_info);
                } else {
                    batches.parallel[valid].1.push(system_index);
                    workload_info.batch_info[valid].systems.1.push(system_info);
                }
            } else if non_send_sync.is_some() {
                batches.parallel.push((Some(system_index), Vec::new()));
                workload_info.batch_info.push(BatchInfo {
                    systems: (Some(system_info), Vec::new()),
                });
            } else {
                batches.parallel.push((None, vec![system_index]));
                workload_info.batch_info.push(BatchInfo {
                    systems: (None, vec![system_info]),
                });
            }
        }
    }

    // Flatten requirements
    // Example:
    // workload: a, b, c, d
    // c after a
    // d after b and before c
    //
    // c needs to inherit d's requirements
    // to end up with a, b, d, c and not a, c, b (d can't be placed)
    //
    // This also needs to work with a c with requirements and one without any
    // workload: a, b, c1, c2, d
    // c1 after a
    // d after b and before c
    // a, b, d, c1, c2
    //
    // And sometimes there isn't any solid fondation, everything has requirements
    // workload: a, b, c
    // a before c
    // b after a
    // c after b
    let mut memoize_before = HashMap::new();
    let mut memoize_after = HashMap::new();
    for (index, (_, _, _, _, before, after)) in before_or_after_collected_systems.iter().enumerate()
    {
        memoize_before.insert(index, before.clone());
        memoize_after.insert(index, after.clone());
    }
    let mut new_requirements = true;
    while new_requirements {
        new_requirements = false;

        for index in 0..before_or_after_collected_systems.len() {
            dependencies(
                index,
                &before_or_after_collected_systems,
                &mut memoize_before,
                &mut new_requirements,
            )
            .map_err(error::AddWorkload::ImpossibleRequirements)?;

            dependencies(
                index,
                &before_or_after_collected_systems,
                &mut memoize_after,
                &mut new_requirements,
            )
            .map_err(error::AddWorkload::ImpossibleRequirements)?;

            let (system_type_id, _, _, _, _, _) = &before_or_after_collected_systems[index];

            for (other_index, (other_system_type_id, _, _, _, _, _)) in
                before_or_after_collected_systems.iter().enumerate()
            {
                let system = system_type_id.as_label();

                if memoize_after
                    .get(&other_index)
                    .unwrap()
                    .iter()
                    .any(|requirement| requirement == &system)
                    && memoize_before
                        .get_mut(&index)
                        .unwrap()
                        .add(other_system_type_id.as_label())
                {
                    new_requirements = true;
                }

                if memoize_before
                    .get(&other_index)
                    .unwrap()
                    .iter()
                    .any(|requirement| requirement == &system)
                    && memoize_after
                        .get_mut(&index)
                        .unwrap()
                        .add(other_system_type_id.as_label())
                {
                    new_requirements = true;
                }
            }
        }
    }

    for (before, before_requirements) in &memoize_before {
        let after_requirements = memoize_after.get(before).unwrap();

        for before_requirement in before_requirements {
            if after_requirements
                .iter()
                .any(|after_requirement| after_requirement == before_requirement)
            {
                return Err(error::AddWorkload::ImpossibleRequirements(
                    error::ImpossibleRequirements::BeforeAndAfter(
                        before_or_after_collected_systems[*before].1.as_label(),
                        before_requirement.clone(),
                    ),
                ));
            }
        }
    }

    for (index, (_, name, system_index, borrow_constraints, _, _)) in
        before_or_after_collected_systems.iter().enumerate()
    {
        let sequential_position = valid_sequential(
            index,
            &memoize_before,
            &memoize_after,
            &batches.sequential,
            system_labels,
            &workload_info.name,
            name,
            system_generators,
        )
        .map_err(error::AddWorkload::ImpossibleRequirements)?;

        let parallel_position = valid_parallel(
            index,
            &memoize_before,
            &memoize_after,
            &batches.parallel,
            system_labels,
            &workload_info.name,
            name,
            system_generators,
        )
        .map_err(error::AddWorkload::ImpossibleRequirements)?;

        // TODO: Move `system_labels`, `system_before` and `system_after` since we are shifting all systems in the sequential order

        batches
            .sequential
            .insert(sequential_position, *system_index);

        let mut other_borrow_constraints = Vec::new();
        system_generators[parallel_position](&mut other_borrow_constraints);

        let single_system = borrow_constraints.iter().any(|type_info| {
            type_info.storage_id == StorageId::of::<AllStorages>() || !type_info.thread_safe
        });

        batches.parallel.insert(
            parallel_position,
            if single_system {
                (Some(*system_index), Vec::new())
            } else {
                (None, vec![*system_index])
            },
        );
    }

    Ok(workload_info)
}

#[allow(clippy::type_complexity)]
fn dependencies(
    index: usize,
    before_or_after_collected_systems: &[(
        TypeId,
        &str,
        usize,
        Vec<TypeInfo>,
        Requirements,
        Requirements,
    )],
    memoize: &mut HashMap<usize, Requirements>,
    new_requirements: &mut bool,
) -> Result<(), error::ImpossibleRequirements> {
    let mut new = memoize.get(&index).unwrap().clone();

    for system in memoize.get(&index).unwrap() {
        for (other_index, (other_system_type_id, _, _, _, _, _)) in
            before_or_after_collected_systems.iter().enumerate()
        {
            if system == &other_system_type_id.as_label() {
                let other = memoize.get(&other_index).unwrap().clone();

                new.extend(other.iter());
            }
        }
    }

    if memoize.get(&index).unwrap().len() < new.len() {
        *new_requirements = true;
    }

    *memoize.get_mut(&index).unwrap() = new;

    Ok(())
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn valid_sequential(
    index: usize,
    memoize_before: &HashMap<usize, Requirements>,
    memoize_after: &HashMap<usize, Requirements>,
    sequential: &[usize],
    system_labels: &HashMap<(Box<dyn Label>, usize), Box<dyn Label>>,
    workload_name: &dyn Label,
    system_name: &'static str,
    system_generators: &[fn(&mut Vec<TypeInfo>) -> TypeId],
) -> Result<usize, error::ImpossibleRequirements> {
    let mut valid_start = sequential.len();
    let mut valid_end = 0;

    let before = &memoize_before[&index];
    let after = &memoize_after[&index];

    for other_index in 0..sequential.len() {
        let other_system = system_labels
            .get(&(workload_name.dyn_clone(), other_index))
            .cloned()
            .unwrap_or_else(|| {
                system_generators[sequential[other_index]](&mut Vec::new()).as_label()
            });

        if before.iter().any(|system| system == &other_system) {
            break;
        } else {
            valid_end += 1;
        }
    }
    for other_index in (0..sequential.len()).rev() {
        let other_system = system_labels
            .get(&(workload_name.dyn_clone(), other_index))
            .cloned()
            .unwrap_or_else(|| {
                system_generators[sequential[other_index]](&mut Vec::new()).as_label()
            });

        if after.iter().any(|system| system == &other_system) {
            break;
        } else {
            valid_start -= 1;
        }
    }

    if valid_start > valid_end {
        return Err(error::ImpossibleRequirements::ImpossibleConstraints(
            system_name.as_label(),
            before.iter().cloned().collect(),
            after.iter().cloned().collect(),
        ));
    }

    Ok(valid_start)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn valid_parallel(
    index: usize,
    memoize_before: &HashMap<usize, Requirements>,
    memoize_after: &HashMap<usize, Requirements>,
    parallel: &[(Option<usize>, Vec<usize>)],
    system_labels: &HashMap<(Box<dyn Label>, usize), Box<dyn Label>>,
    workload_name: &dyn Label,
    system_name: &'static str,
    system_generators: &[fn(&mut Vec<TypeInfo>) -> TypeId],
) -> Result<usize, error::ImpossibleRequirements> {
    let mut valid_start = parallel.len();
    let mut valid_end = 0;

    let before = &memoize_before[&index];
    let after = &memoize_after[&index];

    'outer: for (single_system, systems) in parallel {
        if let &Some(other_system) = single_system {
            let other_system = system_labels
                .get(&(workload_name.dyn_clone(), other_system))
                .cloned()
                .unwrap_or_else(|| system_generators[other_system](&mut Vec::new()).as_label());

            if before.iter().any(|system| system == &other_system) {
                break;
            }
        }

        for &other_system in systems {
            let other_system = system_labels
                .get(&(workload_name.dyn_clone(), other_system))
                .cloned()
                .unwrap_or_else(|| system_generators[other_system](&mut Vec::new()).as_label());

            if before.iter().any(|system| system == &other_system) {
                break 'outer;
            }
        }

        valid_end += 1;
    }

    'outer: for (single_system, systems) in parallel.iter().rev() {
        if let &Some(other_system) = single_system {
            let other_system = system_labels
                .get(&(workload_name.dyn_clone(), other_system))
                .cloned()
                .unwrap_or_else(|| system_generators[other_system](&mut Vec::new()).as_label());

            if after.iter().any(|system| system == &other_system) {
                break;
            }
        }

        for &other_system in systems {
            let other_system = system_labels
                .get(&(workload_name.dyn_clone(), other_system))
                .cloned()
                .unwrap_or_else(|| system_generators[other_system](&mut Vec::new()).as_label());

            if after.iter().any(|system| system == &other_system) {
                break 'outer;
            }
        }

        valid_start -= 1;
    }

    if valid_start > valid_end {
        return Err(error::ImpossibleRequirements::ImpossibleConstraints(
            system_name.as_label(),
            before.iter().cloned().collect(),
            after.iter().cloned().collect(),
        ));
    }

    Ok(valid_start)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn flatten_work_unit(
    work_unit: WorkUnit,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync>>,
    lookup_table: &mut HashMap<TypeId, usize>,
    collected_systems: &mut Vec<(
        TypeId,
        &str,
        usize,
        Vec<TypeInfo>,
        Requirements,
        Requirements,
    )>,
    workloads: &HashMap<Box<dyn Label>, Batches>,
    system_generators: &mut Vec<fn(&mut Vec<TypeInfo>) -> TypeId>,
    system_names: &mut Vec<&'static str>,
    system_before: &HashMap<(Box<dyn Label>, usize), Requirements>,
    system_after: &HashMap<(Box<dyn Label>, usize), Requirements>,
) {
    match work_unit {
        WorkUnit::System(WorkloadSystem::System {
            mut borrow_constraints,
            system_type_name,
            system_type_id,
            generator,
            system_fn,
        }) => {
            let borrow_constraints = core::mem::take(&mut borrow_constraints);
            let system_type_name = system_type_name;
            let system_type_id = system_type_id;

            let system_index = *lookup_table.entry(system_type_id).or_insert_with(|| {
                systems.push(system_fn);
                system_names.push(system_type_name);
                system_generators.push(generator);
                systems.len() - 1
            });

            collected_systems.push((
                system_type_id,
                system_type_name,
                system_index,
                borrow_constraints,
                Requirements::new(),
                Requirements::new(),
            ));
        }
        WorkUnit::WorkloadName(workload) => {
            for (system_sequential_index, &system_index) in
                workloads[&workload].sequential.iter().enumerate()
            {
                let mut borrow = Vec::new();
                let mut before = Requirements::new();
                let mut after = Requirements::new();

                if let Some(systems_before) =
                    system_before.get(&(workload.clone(), system_sequential_index))
                {
                    for system_before in systems_before {
                        before.add(system_before.clone());
                    }
                }

                if let Some(systems_after) =
                    system_after.get(&(workload.clone(), system_sequential_index))
                {
                    for system_after in systems_after {
                        after.add(system_after.clone());
                    }
                }

                collected_systems.push((
                    system_generators[system_index](&mut borrow),
                    system_names[system_index],
                    system_index,
                    borrow,
                    before,
                    after,
                ));
            }
        }
        WorkUnit::System(WorkloadSystem::Workload(workload)) => {
            let start = collected_systems.len();

            for wu in workload.work_units {
                flatten_work_unit(
                    wu,
                    systems,
                    lookup_table,
                    collected_systems,
                    workloads,
                    system_generators,
                    system_names,
                    system_before,
                    system_after,
                )
            }

            for (_, _, _, _, before, after) in &mut collected_systems[start..] {
                for workload_before in &workload.before {
                    before.add(workload_before.clone());
                }

                for workload_after in &workload.after {
                    after.add(workload_after.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Unique};
    use crate::{track, IntoWorkload};

    struct Usize(usize);
    struct U32(u32);
    struct U16(u16);

    impl Component for Usize {
        type Tracking = track::Untracked;
    }
    impl Component for U32 {
        type Tracking = track::Untracked;
    }
    impl Component for U16 {
        type Tracking = track::Untracked;
    }
    impl Unique for Usize {
        type Tracking = track::Untracked;
    }
    impl Unique for U32 {
        type Tracking = track::Untracked;
    }
    impl Unique for U16 {
        type Tracking = track::Untracked;
    }

    #[test]
    fn single_immutable() {
        use crate::{View, World};

        fn system1(_: View<'_, Usize>) {}

        let world = World::new();

        Workload::new("System1")
            .with_system(system1)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("System1");
        assert_eq!(scheduler.systems.len(), 1);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0])],
                sequential: vec![0],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn single_mutable() {
        use crate::{ViewMut, World};

        fn system1(_: ViewMut<'_, Usize>) {}

        let world = World::new();

        Workload::new("System1")
            .with_system(system1)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("System1");
        assert_eq!(scheduler.systems.len(), 1);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0])],
                sequential: vec![0],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_immutable() {
        use crate::{IntoWorkloadSystem, View, World};

        fn system1(_: View<'_, Usize>) {}
        fn system2(_: View<'_, Usize>) {}

        let world = World::new();

        Workload::new("Systems")
            .with_system(system1)
            .with_system(system2.into_workload_system().unwrap())
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_mutable() {
        use crate::{ViewMut, World};

        fn system1(_: ViewMut<'_, Usize>) {}
        fn system2(_: ViewMut<'_, Usize>) {}

        let world = World::new();

        Workload::new("Systems")
            .with_system(system1)
            .with_system(system2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0]), (None, vec![1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_mixed() {
        use crate::{View, ViewMut, World};

        fn system1(_: ViewMut<'_, Usize>) {}
        fn system2(_: View<'_, Usize>) {}

        let world = World::new();

        Workload::new("Systems")
            .with_system(system1)
            .with_system(system2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0]), (None, vec![1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        Workload::new("Systems")
            .with_system(system2)
            .with_system(system1)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0]), (None, vec![1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn append_optimizes_batches() {
        use crate::{View, ViewMut, World};

        fn system_a1(_: View<'_, Usize>, _: ViewMut<'_, U32>) {}
        fn system_a2(_: View<'_, Usize>, _: ViewMut<'_, U32>) {}
        fn system_b1(_: View<'_, Usize>) {}

        let world = World::new();

        let mut group_a = Workload::new("Group A")
            .with_system(system_a1)
            .with_system(system_a2);

        let mut group_b = Workload::new("Group B").with_system(system_b1);

        Workload::new("Combined")
            .append(&mut group_a)
            .append(&mut group_b)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Combined");
        assert_eq!(scheduler.systems.len(), 3);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 2]), (None, vec![1])],
                sequential: vec![0, 1, 2],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn all_storages() {
        use crate::{AllStoragesViewMut, View, World};

        fn system1(_: View<'_, Usize>) {}
        fn system2(_: AllStoragesViewMut<'_>) {}

        let world = World::new();

        Workload::new("Systems")
            .with_system(system2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 1);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(Some(0), Vec::new())],
                sequential: vec![0],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        Workload::new("Systems")
            .with_system(system2)
            .with_system(system2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 1);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(Some(0), Vec::new()), (Some(0), Vec::new())],
                sequential: vec![0, 0],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        Workload::new("Systems")
            .with_system(system1)
            .with_system(system2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0]), (Some(1), Vec::new())],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        Workload::new("Systems")
            .with_system(system2)
            .with_system(system1)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(Some(0), Vec::new()), (None, vec![1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[cfg(feature = "thread_local")]
    #[test]
    fn non_send() {
        use crate::{NonSend, View, ViewMut, World};

        struct NotSend(*const ());
        unsafe impl Sync for NotSend {}
        impl Component for NotSend {
            type Tracking = track::Untracked;
        }

        fn sys1(_: NonSend<View<'_, NotSend>>) {}
        fn sys2(_: NonSend<ViewMut<'_, NotSend>>) {}
        fn sys3(_: View<'_, Usize>) {}
        fn sys4(_: ViewMut<'_, Usize>) {}

        let world = World::new();

        let info = Workload::new("Test")
            .with_system(sys1)
            .with_system(sys1)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Test");
        assert_eq!(scheduler.systems.len(), 1);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 0])],
                sequential: vec![0, 0],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
        assert!(info.batch_info[0].systems.1[0].conflict.is_none());

        let world = World::new();

        Workload::new("Test")
            .with_system(sys1)
            .with_system(sys2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0]), (Some(1), Vec::new())],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        Workload::new("Test")
            .with_system(sys2)
            .with_system(sys1)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(Some(0), Vec::new()), (None, vec![1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        let info = Workload::new("Test")
            .with_system(sys1)
            .with_system(sys3)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
        assert!(info.batch_info[0].systems.1[0].conflict.is_none());

        let world = World::new();

        Workload::new("Test")
            .with_system(sys1)
            .with_system(sys4)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn unique_and_non_unique() {
        use crate::{UniqueViewMut, ViewMut, World};

        fn system1(_: ViewMut<'_, Usize>) {}
        fn system2(_: UniqueViewMut<'_, Usize>) {}

        let world = World::new();

        Workload::new("Systems")
            .with_system(system1)
            .with_system(system2)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 2);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 1])],
                sequential: vec![0, 1],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn empty_workload() {
        use crate::World;

        let world = World::new();

        Workload::new("Systems").add_to_world(&world).unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Systems");
        assert_eq!(scheduler.systems.len(), 0);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![],
                sequential: vec![],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn append_ensures_multiple_batches_can_be_optimized_over() {
        use crate::{View, ViewMut, World};

        fn sys_a1(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}
        fn sys_a2(_: View<'_, Usize>, _: ViewMut<'_, U32>) {}
        fn sys_b1(_: View<'_, Usize>) {}
        fn sys_c1(_: View<'_, U16>) {}

        let world = World::new();

        let mut group_a = Workload::new("Group A")
            .with_system(sys_a1)
            .with_system(sys_a2);
        let mut group_b = Workload::new("Group B").with_system(sys_b1);
        let mut group_c = Workload::new("Group C").with_system(sys_c1);

        Workload::new("Combined")
            .append(&mut group_a)
            .append(&mut group_b)
            .append(&mut group_c)
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        let label: Box<dyn Label> = Box::new("Combined");
        assert_eq!(scheduler.systems.len(), 4);
        assert_eq!(scheduler.workloads.len(), 1);
        assert_eq!(
            scheduler.workloads.get(&label),
            Some(&Batches {
                parallel: vec![(None, vec![0, 3]), (None, vec![1, 2])],
                sequential: vec![0, 1, 2, 3],
                skip_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn workload_flattening() {
        use crate::{View, ViewMut, World};

        fn sys1(_: View<'_, U32>) {}
        fn sys2(_: ViewMut<'_, U32>) {}

        let world = World::new();

        Workload::new("1")
            .with_system(sys1)
            .with_system(sys2)
            .with_system(sys1)
            .add_to_world(&world)
            .unwrap();

        let debug_info = Workload::new("2")
            .with_workload("1")
            .with_system(sys1)
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

        Workload::new("1").add_to_world(&world).unwrap();

        let debug_info = Workload::new("2")
            .with_workload("1")
            .add_to_world(&world)
            .unwrap();

        let scheduler = world.scheduler.borrow_mut().unwrap();
        assert_eq!(scheduler.systems.len(), 0);
        assert_eq!(debug_info.batch_info.len(), 0);
    }

    #[test]
    fn skip_if_missing_storage() {
        let world = World::new();

        Workload::new("test")
            .skip_if_storage_empty::<Usize>()
            .with_system(|| panic!())
            .build()
            .unwrap()
            .0
            .run_with_world(&world)
            .unwrap();

        Workload::new("test")
            .skip_if_storage_empty::<Usize>()
            .with_system(|| panic!())
            .add_to_world(&world)
            .unwrap();

        world.run_default().unwrap();
    }

    #[test]
    fn skip_if_empty_storage() {
        let mut world = World::new();

        let eid = world.add_entity((Usize(0),));
        world.remove::<(Usize,)>(eid);

        Workload::new("test")
            .skip_if_storage_empty::<Usize>()
            .with_system(|| panic!())
            .build()
            .unwrap()
            .0
            .run_with_world(&world)
            .unwrap();

        Workload::new("test")
            .skip_if_storage_empty::<Usize>()
            .with_system(|| panic!())
            .add_to_world(&world)
            .unwrap();

        world.run_default().unwrap();
    }

    #[test]
    fn before_after() {
        fn a() {}
        fn b() {}
        fn c() {}

        let (workload, _) = Workload::new("")
            .with_system(c.after_all(b))
            .with_system(b.after_all(a))
            .with_system(a)
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[2, 1, 0]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![2]), (None, vec![1]), (None, vec![0])]
        );

        let (workload, _) = Workload::new("")
            .with_system(a)
            .with_system(b.after_all(a))
            .with_system(c.after_all(b))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[0, 1, 2]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![0]), (None, vec![1]), (None, vec![2])]
        );

        let (workload, _) = Workload::new("")
            .with_system(b.after_all(a))
            .with_system(a)
            .with_system(c.after_all(b))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[1, 0, 2]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![1]), (None, vec![0]), (None, vec![2])]
        );
    }

    #[test]
    fn before_after_loop() {
        fn a() {}
        fn b() {}

        let result = Workload::new("")
            .with_system(a.after_all(b))
            .with_system(b.after_all(a))
            .build();

        // HashMap makes this error random
        assert!(result.is_err());
    }

    #[test]
    fn before_after_no_anchor() {
        fn a() {}
        fn b() {}

        let (workload, _) = Workload::new("")
            .with_system(a.before_all(b))
            .with_system(b.after_all(a))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[0, 1]);
        assert_eq!(batches.parallel, &[(None, vec![0]), (None, vec![1])]);

        let (workload, _) = Workload::new("")
            .with_system(b.after_all(a))
            .with_system(a.before_all(b))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[1, 0]);
        assert_eq!(batches.parallel, &[(None, vec![1]), (None, vec![0])]);
    }

    #[test]
    fn before_after_missing_system() {
        fn a() {}
        fn b() {}

        let (workload, _) = Workload::new("")
            .with_system(a.before_all(b))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[0]);
        assert_eq!(batches.parallel, &[(None, vec![0])]);
    }

    #[test]
    fn before_after_absent_system() {
        fn a() {}
        fn b() {}
        fn c() {}

        let (workload, _) = Workload::new("")
            .with_system(a)
            .with_system(c.after_all(b))
            .build()
            .unwrap();

        // HashMap makes this error random between a and c
        let batches = &workload.workloads[&"".as_label()];
        assert!(batches.sequential == &[0, 1] || batches.sequential == &[1, 0]);
        assert!(
            batches.parallel == &[(None, vec![0]), (None, vec![1])]
                || batches.parallel == &[(None, vec![1]), (None, vec![0])]
        );
    }
}
