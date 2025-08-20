use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::component::{Component, Unique};
use crate::scheduler::info::{
    BatchInfo, Conflict, DedupedLabels, SystemId, SystemInfo, TypeInfo, WorkloadInfo,
};
use crate::scheduler::label::{SystemLabel, WorkloadLabel};
use crate::scheduler::system::{ExtractWorkloadRunIf, WorkloadRunIfFn};
use crate::scheduler::{
    AsLabel, Batches, IntoWorkload, IntoWorkloadSystem, IntoWorkloadTrySystem, Label, Scheduler,
    WorkloadSystem,
};
use crate::storage::StorageId;
use crate::type_id::TypeId;
use crate::unique::UniqueStorage;
use crate::world::World;
use crate::{error, ShipHashMap};
use alloc::boxed::Box;
use alloc::format;
// macro not module
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
#[cfg(not(feature = "std"))]
use core::any::Any;
#[cfg(feature = "std")]
use std::error::Error;

/// Used to create a [`Workload`].
///
/// You can also use [`Workload::new`].
///
/// [`Workload`]: crate::Workload
/// [`Workload::new`]: crate::Workload::new()
#[allow(clippy::type_complexity)]
pub struct ScheduledWorkload {
    name: Box<dyn Label>,
    #[allow(clippy::type_complexity)]
    systems: Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: Vec<Box<dyn Label>>,
    #[allow(unused)]
    system_generators: Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    // system's `TypeId` to an index into both systems and system_names
    #[allow(unused)]
    lookup_table: ShipHashMap<TypeId, usize>,
    tracking_to_enable: Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    /// workload name to list of "batches"
    workloads: ShipHashMap<Box<dyn Label>, Batches>,
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
            &self.name,
        )
    }

    /// Apply tracking to all storages using it during this workload.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    /// - Systems' storage (exclusive) to enable tracking
    ///
    /// ### Errors
    ///
    /// - [`AllStorages`] borrow failed.
    /// - Storage borrow failed.
    pub fn apply_tracking(&self, world: &World) -> Result<(), error::GetStorage> {
        let all_storages = world
            .all_storages()
            .map_err(error::GetStorage::AllStoragesBorrow)?;

        for enable_tracking_fn in &self.tracking_to_enable {
            (enable_tracking_fn)(&all_storages)?;
        }

        Ok(())
    }
}

impl World {
    /// Creates a new workload and store it in the [`World`].
    pub fn add_workload<Views, R, W, F: FnOnce() -> W + 'static>(&self, workload: F)
    where
        W: IntoWorkload<Views, R>,
    {
        let mut w = workload().into_workload();

        w.tags.push(Box::new(WorkloadLabel {
            type_id: TypeId::of::<F>(),
            name: type_name::<F>().as_label(),
        }));

        w.name = if w.overwritten_name {
            w.tags.push(w.name.clone());

            w.name
        } else {
            Box::new(WorkloadLabel {
                type_id: TypeId::of::<F>(),
                name: type_name::<F>().as_label(),
            })
        };

        w.add_to_world(self).unwrap();
    }
}

/// Keeps information to create a workload.
///
/// A workload is a collection of systems. They will execute as much in parallel as possible.  
/// They are evaluated first to last when they can't be parallelized.  
/// The default workload will automatically be set to the first workload added.
pub struct Workload {
    pub(super) name: Box<dyn Label>,
    pub(super) tags: Vec<Box<dyn Label>>,
    pub(super) systems: Vec<WorkloadSystem>,
    pub(super) run_if: Option<Box<dyn WorkloadRunIfFn>>,
    pub(super) before_all: DedupedLabels,
    pub(super) after_all: DedupedLabels,
    pub(super) overwritten_name: bool,
    pub(super) require_before: DedupedLabels,
    pub(super) require_after: DedupedLabels,
    pub(super) barriers: Vec<usize>,
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
    /// world.run_default_workload();
    /// ```
    pub fn new<T>(label: impl AsLabel<T>) -> Self {
        let label = label.as_label();

        Workload {
            systems: Vec::new(),
            name: label.clone(),
            run_if: None,
            tags: vec![label],
            before_all: DedupedLabels::new(),
            after_all: DedupedLabels::new(),
            overwritten_name: true,
            require_before: DedupedLabels::new(),
            require_after: DedupedLabels::new(),
            barriers: Vec::new(),
        }
    }
    /// Moves all systems of `other` into `Self`, leaving `other` empty.  
    /// This allows us to collect systems in different builders before joining them together.
    pub fn append(mut self, other: &mut Self) -> Self {
        self.systems.append(&mut other.systems);

        self
    }
    /// Propagates all information from `self` and `other` into their respective systems before merging their systems.  
    /// This includes `run_if`/`skip_if`, `tags`, `before`/`after` requirements.
    pub fn merge(mut self, mut other: Workload) -> Workload {
        self.propagate();
        other.propagate();

        let systems_len = self.systems.len();
        self.barriers.extend(
            other
                .barriers
                .drain(..)
                .map(|barrier| barrier + systems_len),
        );

        self.append(&mut other)
    }
    /// Propagates all information into the systems.  
    /// This includes `run_if`/`skip_if`, `tags`, `before`/`after` requirements.
    fn propagate(&mut self) {
        for system in &mut self.systems {
            system.run_if = match (system.run_if.take(), self.run_if.clone()) {
                (None, None) => None,
                (None, Some(run_if)) => Some(run_if.to_non_clone()),
                (Some(run_if), None) => Some(run_if),
                (Some(system_run_if), Some(workload_run_if)) => Some(Box::new(move |world| {
                    Ok(workload_run_if.clone().run(world)? && (system_run_if)(world)?)
                })),
            };

            system.tags.extend(self.tags.iter().cloned());

            system.before_all.extend(self.before_all.iter().cloned());
            system.after_all.extend(self.after_all.iter().cloned());
            system
                .require_before
                .extend(self.require_before.iter().cloned());
            system
                .require_after
                .extend(self.require_after.iter().cloned());
        }

        self.run_if = None;
        self.tags.clear();
        self.before_all.clear();
        self.after_all.clear();
        self.require_before.clear();
        self.require_after.clear();
    }
    /// Propagates all information from `self` and `other` into their respective systems before merging their systems.  
    /// This includes `run_if`/`skip_if`, `tags`, `before`/`after` requirements.
    pub fn with_workload(self, other: Workload) -> Workload {
        self.merge(other)
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
    /// world.run_default_workload();
    /// ```
    #[track_caller]
    pub fn with_system<B, R, S: IntoWorkloadSystem<B, R>>(mut self, system: S) -> Workload {
        self.systems.push(system.into_workload_system().unwrap());

        self
    }
    /// Adds a fallible system to the workload being created.  
    /// The workload's execution will stop if any error is encountered.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, Get, IntoIter, View, ViewMut, Workload, World};
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
    /// world.run_default_workload();
    /// ```
    #[track_caller]
    #[cfg(feature = "std")]
    pub fn with_try_system<
        B,
        Ok,
        Err: 'static + Into<Box<dyn Error + Send + Sync>>,
        R: Into<Result<Ok, Err>>,
        S: IntoWorkloadTrySystem<B, R>,
    >(
        mut self,
        system: S,
    ) -> Self {
        self.systems
            .push(system.into_workload_try_system::<Ok, Err>().unwrap());

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
    /// world.run_default_workload();
    /// ```
    #[track_caller]
    #[cfg(not(feature = "std"))]
    pub fn with_try_system<
        B,
        Ok,
        Err: 'static + Send + Any,
        R: Into<Result<Ok, Err>>,
        S: IntoWorkloadTrySystem<B, R>,
    >(
        mut self,
        system: S,
    ) -> Self {
        self.systems
            .push(system.into_workload_try_system::<Ok, Err>().unwrap().into());

        self
    }
    /// Finishes the workload creation and stores it in the [`World`].  
    /// Returns a struct with describing how the workload has been split in batches.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (exclusive)
    /// - [`AllStorages`] (shared)
    /// - Systems' storage (exclusive) to enable tracking
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload with an identical name already present.
    /// - Nested workload is not present in `world`.
    /// - [`AllStorages`] borrow failed.
    /// - Storage borrow failed.
    #[allow(clippy::blocks_in_conditions)]
    pub fn add_to_world(self, world: &World) -> Result<(), error::AddWorkload> {
        let Scheduler {
            systems,
            system_names,
            system_generators,
            lookup_table,
            workloads,
            workloads_info,
            default,
        } = &mut *world
            .scheduler
            .borrow_mut()
            .map_err(|_| error::AddWorkload::Borrow)?;

        let mut tracking_to_enable = Vec::new();

        let name = self.name.dyn_clone();

        let workload_info = create_workload(
            self,
            systems,
            system_names,
            system_generators,
            lookup_table,
            &mut tracking_to_enable,
            workloads,
            default,
        )?;

        let all_storages = world
            .all_storages()
            .map_err(|_| error::AddWorkload::TrackingAllStoragesBorrow)?;

        for enable_tracking_fn in &tracking_to_enable {
            (enable_tracking_fn)(&all_storages).map_err(|err| match err {
                error::GetStorage::StorageBorrow { name, id, borrow } => {
                    error::AddWorkload::TrackingStorageBorrow { name, id, borrow }
                }
                _ => unreachable!(),
            })?;
        }

        workloads_info.insert(name, workload_info);

        Ok(())
    }
    /// Returns the first [`Unique`] storage borrowed by this workload that is not present in `world`.\
    /// If the workload contains nested workloads they have to be present in the `World`.
    ///
    /// ### Borrows
    ///
    /// - AllStorages (shared)
    pub fn are_all_uniques_present_in_world(
        &self,
        world: &World,
    ) -> Result<(), error::UniquePresence> {
        struct ComponentType;

        impl Component for ComponentType {
            type Tracking = crate::track::Untracked;
        }
        impl Unique for ComponentType {}

        let all_storages = world
            .all_storages
            .borrow()
            .map_err(|_| error::UniquePresence::AllStorages)?;
        let storages = all_storages.storages.read();

        let unique_name = type_name::<UniqueStorage<ComponentType>>()
            .split_once('<')
            .unwrap()
            .0;

        for work_unit in &self.systems {
            if let Some(value) = check_uniques_in_systems(work_unit, unique_name, &storages) {
                return value;
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
            lookup_table: ShipHashMap::new(),
            tracking_to_enable: Vec::new(),
            workloads: ShipHashMap::new(),
        };

        let mut default: Box<dyn Label> = Box::new("");

        let workload_info = create_workload(
            self,
            &mut workload.systems,
            &mut workload.system_names,
            &mut workload.system_generators,
            &mut workload.lookup_table,
            &mut workload.tracking_to_enable,
            &mut workload.workloads,
            &mut default,
        )?;

        Ok((workload, workload_info))
    }
    /// Stop parallelism between systems before and after the barrier.
    pub fn with_barrier(mut self) -> Self {
        self.barriers.push(self.systems.len());

        self
    }
}

fn check_uniques_in_systems(
    system: &WorkloadSystem,
    unique_name: &str,
    storages: &ShipHashMap<StorageId, crate::storage::SBox>,
) -> Option<Result<(), error::UniquePresence>> {
    let WorkloadSystem {
        borrow_constraints, ..
    } = system;

    for type_info in borrow_constraints {
        if type_info.name.starts_with(unique_name) && !storages.contains_key(&type_info.storage_id)
        {
            return Some(Err(error::UniquePresence::Unique(type_info.clone())));
        }
    }

    None
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn create_workload(
    mut builder: Workload,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: &mut Vec<Box<dyn Label>>,
    system_generators: &mut Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    lookup_table: &mut ShipHashMap<TypeId, usize>,
    tracking_to_enable: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    workloads: &mut ShipHashMap<Box<dyn Label>, Batches>,
    default: &mut Box<dyn Label>,
) -> Result<WorkloadInfo, error::AddWorkload> {
    if workloads.contains_key(&*builder.name) {
        return Err(error::AddWorkload::AlreadyExists);
    }

    for index in builder.barriers.drain(..) {
        let tag = format!("__barrier__{}", index);

        for system in &mut builder.systems[..index] {
            system.tags.push(Box::new(tag.clone()));
        }

        for system in &mut builder.systems[index..] {
            system.after_all.add(tag.clone());
        }
    }

    let mut collected_systems: Vec<(usize, WorkloadSystem)> =
        Vec::with_capacity(builder.systems.len());

    for mut system in builder.systems.drain(..) {
        for tracking_to_enable_fn in system.tracking_to_enable.drain(..) {
            tracking_to_enable.push(tracking_to_enable_fn);
        }

        insert_system_in_scheduler(
            system,
            systems,
            lookup_table,
            &mut collected_systems,
            system_generators,
            system_names,
        );
    }

    if workloads.is_empty() {
        *default = builder.name.clone();
    }

    let batches = workloads.entry(builder.name.clone()).or_default();

    batches.run_if = builder.run_if;

    if collected_systems.len() == 1 {
        let (
            system_index,
            WorkloadSystem {
                type_id,
                display_name,
                borrow_constraints,
                run_if,
                ..
            },
        ) = collected_systems.pop().unwrap();

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
            batches.parallel_run_if.push((Some(0), Vec::new()));
        } else {
            batches.parallel.push((None, vec![system_index]));
            batches.parallel_run_if.push((None, vec![0]));
        }

        batches.sequential.push(system_index);
        batches.sequential_run_if.push(run_if);

        let batch_info = BatchInfo {
            systems: (
                Some(SystemInfo {
                    name: format!("{:?}", display_name),
                    type_id,
                    borrow: borrow_constraints,
                    conflict: None,
                    before: Vec::new(),
                    after: Vec::new(),
                }),
                Vec::new(),
            ),
        };

        return Ok(WorkloadInfo {
            name: format!("{:?}", builder.name),
            batch_info: vec![batch_info],
        });
    }

    let mut workload_info = WorkloadInfo {
        name: format!("{:?}", builder.name),
        batch_info: vec![],
    };

    // // Extract systems that have before/after requirements as they are not scheduled the same way
    // let mut before_or_after_collected_systems = Vec::new();
    // for i in (0..collected_systems.len()).rev() {
    //     if !collected_systems[i].1.before_all.is_empty()
    //         || !collected_systems[i].1.after_all.is_empty()
    //     {
    //         before_or_after_collected_systems.push(collected_systems.remove(i));
    //     }
    // }
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
    let mut memoize_before = ShipHashMap::new();
    let mut memoize_after = ShipHashMap::new();
    let mut collected_tags = Vec::new();
    let mut collected_require_in_workload = Vec::new();
    let mut collected_before = Vec::new();
    let mut collected_after = Vec::new();
    let mut collected_names = Vec::new();

    for (
        index,
        (
            _,
            WorkloadSystem {
                before_all,
                after_all,
                tags,
                require_in_workload,
                require_before,
                require_after,
                display_name,
                ..
            },
        ),
    ) in collected_systems.iter_mut().enumerate()
    {
        memoize_before.insert(index, before_all.clone());
        memoize_after.insert(index, after_all.clone());
        collected_tags.push(core::mem::take(tags));
        collected_require_in_workload.push(core::mem::take(require_in_workload));
        collected_before.push(core::mem::take(require_before));
        collected_after.push(core::mem::take(require_after));
        collected_names.push(display_name.clone());
    }

    // Remove before/after that are not present in the workload
    // This makes the systems with no before/after present scheduled like regular systems
    for (index, before) in &mut memoize_before {
        before.retain(|label| {
            collected_tags
                .iter()
                .enumerate()
                .flat_map(|(i, tags)| if i != *index { &**tags } else { &[] })
                .any(|tag| tag == label)
        });
    }
    for (index, after) in &mut memoize_after {
        after.retain(|label| {
            collected_tags
                .iter()
                .enumerate()
                .flat_map(|(i, tags)| if i != *index { &**tags } else { &[] })
                .any(|tag| tag == label)
        });
    }

    let mut new_requirements = true;
    while new_requirements {
        new_requirements = false;

        for index in 0..collected_systems.len() {
            dependencies(
                index,
                &collected_tags,
                &mut memoize_before,
                &mut new_requirements,
            )
            .map_err(error::AddWorkload::ImpossibleRequirements)?;

            dependencies(
                index,
                &collected_tags,
                &mut memoize_after,
                &mut new_requirements,
            )
            .map_err(error::AddWorkload::ImpossibleRequirements)?;

            let tags = &collected_tags[index];

            for (
                other_index,
                (
                    _,
                    WorkloadSystem {
                        type_id: other_type_id,
                        display_name,
                        ..
                    },
                ),
            ) in collected_systems.iter().enumerate()
            {
                if memoize_after
                    .get(&other_index)
                    .unwrap()
                    .iter()
                    .any(|requirement| tags.contains(requirement))
                    && memoize_before.get_mut(&index).unwrap().add(SystemLabel {
                        type_id: *other_type_id,
                        name: display_name.clone(),
                    })
                {
                    new_requirements = true;
                }

                if memoize_before
                    .get(&other_index)
                    .unwrap()
                    .iter()
                    .any(|requirement| tags.contains(requirement))
                    && memoize_after.get_mut(&index).unwrap().add(SystemLabel {
                        type_id: *other_type_id,
                        name: display_name.clone(),
                    })
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
                        collected_systems[*before].1.display_name.clone(),
                        before_requirement.clone(),
                    ),
                ));
            }
        }
    }

    let mut seq_system_index_map = Vec::new();
    let mut par_system_index_map = Vec::new();

    let (collected_systems, before_after_collected_systems) = collected_systems
        .into_iter()
        .enumerate()
        .partition::<Vec<_>, _>(|(index, _)| {
            memoize_before[index].is_empty() && memoize_after[index].is_empty()
        });

    for (
        index,
        (
            system_index,
            WorkloadSystem {
                type_id,
                display_name,
                borrow_constraints,
                run_if,
                tags: _,
                ..
            },
        ),
    ) in collected_systems
    {
        insert_system(
            batches,
            &mut workload_info,
            index,
            system_index,
            type_id,
            display_name,
            borrow_constraints,
            run_if,
            &mut seq_system_index_map,
            &mut par_system_index_map,
        );
    }

    for (
        index,
        (
            system_index,
            WorkloadSystem {
                type_id,
                display_name,
                borrow_constraints,
                run_if,
                tags: _,
                ..
            },
        ),
    ) in before_after_collected_systems
    {
        insert_before_after_system(
            batches,
            &mut workload_info,
            &collected_tags,
            index,
            system_index,
            type_id,
            &display_name,
            borrow_constraints,
            run_if,
            &memoize_before,
            &memoize_after,
            &mut seq_system_index_map,
            &mut par_system_index_map,
        )?;
    }

    for (i, &index) in seq_system_index_map.iter().enumerate() {
        let mut require_in_workload = collected_require_in_workload[index].to_vec();
        let mut require_before = collected_before[index].to_vec();
        let mut require_after = collected_after[index].to_vec();

        for other_tag in seq_system_index_map[..i]
            .iter()
            .flat_map(|&other_index| &collected_tags[other_index])
        {
            require_in_workload.retain(|require| require != other_tag);
            require_before.retain(|require| require != other_tag);
        }

        for other_tag in seq_system_index_map[i..]
            .iter()
            .skip(1)
            .flat_map(|&other_index| &collected_tags[other_index])
        {
            require_in_workload.retain(|require| require != other_tag);
            require_after.retain(|require| require != other_tag);
        }

        if !require_in_workload.is_empty() {
            return Err(error::AddWorkload::MissingInWorkload(
                collected_names[index].clone(),
                require_in_workload,
            ));
        }
        if !require_before.is_empty() {
            return Err(error::AddWorkload::MissingBefore(
                collected_names[index].clone(),
                require_before,
            ));
        }
        if !require_after.is_empty() {
            return Err(error::AddWorkload::MissingAfter(
                collected_names[index].clone(),
                require_after,
            ));
        }
    }

    for (i, &index) in
        par_system_index_map
            .iter()
            .enumerate()
            .flat_map(|(i, (single_system, systems))| {
                single_system
                    .iter()
                    .chain(systems)
                    .map(move |index| (i, index))
            })
    {
        let mut require_in_workload = collected_require_in_workload[index].to_vec();
        let mut require_before = collected_before[index].to_vec();
        let mut require_after = collected_after[index].to_vec();

        for other_tag in par_system_index_map[..i]
            .iter()
            .flat_map(|(single_system, systems)| single_system.iter().chain(systems))
            .flat_map(|&other_index| &collected_tags[other_index])
        {
            require_in_workload.retain(|require| require != other_tag);
            require_before.retain(|require| require != other_tag);
        }

        for other_tag in par_system_index_map[i..]
            .iter()
            .skip(1)
            .flat_map(|(single_system, systems)| single_system.iter().chain(systems))
            .flat_map(|&other_index| &collected_tags[other_index])
        {
            require_in_workload.retain(|require| require != other_tag);
            require_after.retain(|require| require != other_tag);
        }

        if !require_in_workload.is_empty() {
            return Err(error::AddWorkload::MissingInWorkload(
                collected_names[index].clone(),
                require_in_workload,
            ));
        }
        if !require_before.is_empty() {
            return Err(error::AddWorkload::MissingBefore(
                collected_names[index].clone(),
                require_before,
            ));
        }
        if !require_after.is_empty() {
            return Err(error::AddWorkload::MissingAfter(
                collected_names[index].clone(),
                require_after,
            ));
        }
    }

    Ok(workload_info)
}

#[allow(clippy::needless_range_loop)]
fn dependencies(
    index: usize,
    collected_tags: &[Vec<Box<dyn Label>>],
    memoize: &mut ShipHashMap<usize, DedupedLabels>,
    new_requirements: &mut bool,
) -> Result<(), error::ImpossibleRequirements> {
    let mut new = memoize.get(&index).unwrap().clone();

    for system in memoize.get(&index).unwrap() {
        for other_index in 0..collected_tags.len() {
            if other_index != index && collected_tags[other_index].contains(system) {
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
fn insert_system(
    batches: &mut Batches,
    workload_info: &mut WorkloadInfo,
    index: usize,
    system_index: usize,
    type_id: TypeId,
    display_name: Box<dyn Label>,
    borrow_constraints: Vec<TypeInfo>,
    run_if: Option<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync>>,
    seq_system_index_map: &mut Vec<usize>,
    par_system_index_map: &mut Vec<(Option<usize>, Vec<usize>)>,
) {
    batches.sequential.push(system_index);
    batches.sequential_run_if.push(run_if);
    seq_system_index_map.push(index);

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
                        name: format!("{:?}", display_name),
                        type_id,
                        borrow: borrow_constraints,
                        conflict: Some(Conflict::Borrow {
                            type_info: Some(all_storages_type_info),
                            other_system: SystemId {
                                name: other_system_info.name.clone(),
                                type_id: other_system_info.type_id,
                            },
                            other_type_info: other_system_info.borrow.last().unwrap().clone(),
                        }),
                        before: Vec::new(),
                        after: Vec::new(),
                    };

                    if valid < batches.parallel.len() {
                        batches.parallel[valid].0 = Some(system_index);
                        batches.parallel_run_if[valid].0 =
                            Some(batches.sequential_run_if.len() - 1);
                        workload_info.batch_info[valid].systems.0 = Some(system_info);
                        par_system_index_map[valid].0 = Some(index);
                    } else {
                        batches.parallel.push((Some(system_index), Vec::new()));
                        batches
                            .parallel_run_if
                            .push((Some(batches.sequential_run_if.len() - 1), Vec::new()));
                        workload_info.batch_info.push(BatchInfo {
                            systems: (Some(system_info), Vec::new()),
                        });
                        par_system_index_map.push((Some(index), Vec::new()));
                    }

                    return;
                }
            }
        }

        let system_info = SystemInfo {
            name: format!("{:?}", display_name),
            type_id,
            borrow: borrow_constraints,
            conflict: None,
            before: Vec::new(),
            after: Vec::new(),
        };

        if valid < batches.parallel.len() {
            batches.parallel[valid].0 = Some(system_index);
            batches.parallel_run_if[valid].0 = Some(batches.sequential_run_if.len() - 1);
            workload_info.batch_info[valid].systems.0 = Some(system_info);
            par_system_index_map[valid].0 = Some(index);
        } else {
            batches.parallel.push((Some(system_index), Vec::new()));
            batches
                .parallel_run_if
                .push((Some(batches.sequential_run_if.len() - 1), Vec::new()));
            workload_info.batch_info.push(BatchInfo {
                systems: (Some(system_info), Vec::new()),
            });
            par_system_index_map.push((Some(index), Vec::new()));
        }
    } else {
        let mut conflict = None;

        'batch: for (i, batch_info) in workload_info.batch_info.iter().enumerate().rev() {
            if let (Some(non_send_sync_type_info), Some(other_system_info)) =
                (&non_send_sync, &batch_info.systems.0)
            {
                let system_info = SystemInfo {
                    name: format!("{:?}", display_name),
                    type_id,
                    borrow: borrow_constraints,
                    conflict: Some(Conflict::Borrow {
                        type_info: Some(non_send_sync_type_info.clone()),
                        other_system: SystemId {
                            name: other_system_info.name.clone(),
                            type_id: other_system_info.type_id,
                        },
                        other_type_info: other_system_info.borrow.last().unwrap().clone(),
                    }),
                    before: Vec::new(),
                    after: Vec::new(),
                };

                if valid < batches.parallel.len() {
                    batches.parallel[valid].0 = Some(system_index);
                    batches.parallel_run_if[valid].0 = Some(batches.sequential_run_if.len() - 1);
                    workload_info.batch_info[valid].systems.0 = Some(system_info);
                    par_system_index_map[valid].0 = Some(index);
                } else {
                    batches.parallel.push((Some(system_index), Vec::new()));
                    batches
                        .parallel_run_if
                        .push((Some(batches.sequential_run_if.len() - 1), Vec::new()));
                    workload_info.batch_info.push(BatchInfo {
                        systems: (Some(system_info), Vec::new()),
                    });
                    par_system_index_map.push((Some(index), Vec::new()));
                }

                return;
            } else {
                for other_system in batch_info
                    .systems
                    .0
                    .iter()
                    .chain(batch_info.systems.1.iter())
                {
                    check_conflict(other_system, &borrow_constraints, &mut conflict);

                    if conflict.is_some() {
                        break 'batch;
                    }
                }

                valid = i;
            }
        }

        let system_info = SystemInfo {
            name: format!("{:?}", display_name),
            type_id,
            borrow: borrow_constraints,
            conflict,
            before: Vec::new(),
            after: Vec::new(),
        };

        if valid < batches.parallel.len() {
            if non_send_sync.is_some() {
                batches.parallel[valid].0 = Some(system_index);
                batches.parallel_run_if[valid].0 = Some(batches.sequential_run_if.len() - 1);
                workload_info.batch_info[valid].systems.0 = Some(system_info);
                par_system_index_map[valid].0 = Some(index);
            } else {
                batches.parallel[valid].1.push(system_index);
                batches.parallel_run_if[valid]
                    .1
                    .push(batches.sequential_run_if.len() - 1);
                workload_info.batch_info[valid].systems.1.push(system_info);
                par_system_index_map[valid].1.push(index);
            }
        } else if non_send_sync.is_some() {
            batches.parallel.push((Some(system_index), Vec::new()));
            batches
                .parallel_run_if
                .push((Some(batches.sequential_run_if.len() - 1), Vec::new()));
            workload_info.batch_info.push(BatchInfo {
                systems: (Some(system_info), Vec::new()),
            });
            par_system_index_map.push((Some(index), Vec::new()));
        } else {
            batches.parallel.push((None, vec![system_index]));
            batches
                .parallel_run_if
                .push((None, vec![batches.sequential_run_if.len() - 1]));
            workload_info.batch_info.push(BatchInfo {
                systems: (None, vec![system_info]),
            });
            par_system_index_map.push((None, vec![index]));
        }
    }
}

fn check_conflict(
    other_system: &SystemInfo,
    borrow_constraints: &[TypeInfo],
    conflict: &mut Option<Conflict>,
) {
    for other_type_info in &other_system.borrow {
        for type_info in borrow_constraints {
            match type_info.mutability {
                Mutability::Exclusive => {
                    if !type_info.thread_safe && !other_type_info.thread_safe {
                        *conflict = Some(Conflict::OtherNotSendSync {
                            system: SystemId {
                                name: other_system.name.clone(),
                                type_id: other_system.type_id,
                            },
                            type_info: other_type_info.clone(),
                        });

                        return;
                    }

                    if type_info.storage_id == other_type_info.storage_id
                        || type_info.storage_id == TypeId::of::<AllStorages>()
                        || other_type_info.storage_id == TypeId::of::<AllStorages>()
                    {
                        *conflict = Some(Conflict::Borrow {
                            type_info: Some(type_info.clone()),
                            other_system: SystemId {
                                name: other_system.name.clone(),
                                type_id: other_system.type_id,
                            },
                            other_type_info: other_type_info.clone(),
                        });

                        return;
                    }
                }
                Mutability::Shared => {
                    if !type_info.thread_safe && !other_type_info.thread_safe {
                        *conflict = Some(Conflict::OtherNotSendSync {
                            system: SystemId {
                                name: other_system.name.clone(),
                                type_id: other_system.type_id,
                            },
                            type_info: other_type_info.clone(),
                        });

                        return;
                    }

                    if (type_info.storage_id == other_type_info.storage_id
                        && other_type_info.mutability == Mutability::Exclusive)
                        || type_info.storage_id == TypeId::of::<AllStorages>()
                        || other_type_info.storage_id == TypeId::of::<AllStorages>()
                    {
                        *conflict = Some(Conflict::Borrow {
                            type_info: Some(type_info.clone()),
                            other_system: SystemId {
                                name: other_system.name.clone(),
                                type_id: other_system.type_id,
                            },
                            other_type_info: other_type_info.clone(),
                        });

                        return;
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn insert_before_after_system(
    batches: &mut Batches,
    workload_info: &mut WorkloadInfo,
    collected_tags: &[Vec<Box<dyn Label>>],
    index: usize,
    system_index: usize,
    type_id: TypeId,
    display_name: &dyn Label,
    borrow_constraints: Vec<TypeInfo>,
    run_if: Option<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync>>,
    memoize_before: &ShipHashMap<usize, DedupedLabels>,
    memoize_after: &ShipHashMap<usize, DedupedLabels>,
    seq_system_index_map: &mut Vec<usize>,
    par_system_index_map: &mut Vec<(Option<usize>, Vec<usize>)>,
) -> Result<(), error::AddWorkload> {
    let sequential_position = valid_sequential(
        index,
        memoize_before,
        memoize_after,
        batches.sequential.len(),
        collected_tags,
        seq_system_index_map,
        display_name,
    )
    .map_err(error::AddWorkload::ImpossibleRequirements)?;

    let (parallel_position, can_go_in) = valid_parallel(
        index,
        memoize_before,
        memoize_after,
        &batches.parallel,
        collected_tags,
        par_system_index_map,
        display_name,
    )
    .map_err(error::AddWorkload::ImpossibleRequirements)?;

    batches.sequential.insert(sequential_position, system_index);
    batches
        .sequential_run_if
        .insert(sequential_position, run_if);
    seq_system_index_map.insert(sequential_position, index);

    for (single_run_if, run_if_indices) in &mut batches.parallel_run_if {
        if let Some(single_run_if) = single_run_if {
            if *single_run_if >= sequential_position {
                *single_run_if += 1;
            }
        }

        for run_if_index in run_if_indices {
            if *run_if_index >= sequential_position {
                *run_if_index += 1;
            }
        }
    }

    let single_system = borrow_constraints.iter().any(|type_info| {
        type_info.storage_id == StorageId::of::<AllStorages>() || !type_info.thread_safe
    });

    let mut conflict = None;
    if can_go_in {
        if single_system {
            if let Some(other_system) = &workload_info.batch_info[parallel_position].systems.0 {
                conflict = Some(Conflict::OtherNotSendSync {
                    system: SystemId {
                        name: format!("{:?}", display_name),
                        type_id,
                    },
                    type_info: other_system.borrow[0].clone(),
                })
            }

            for other_system in &workload_info.batch_info[parallel_position].systems.1 {
                check_conflict(other_system, &borrow_constraints, &mut conflict);

                if conflict.is_some() {
                    break;
                }
            }
        } else {
            for other_system in workload_info.batch_info[parallel_position]
                .systems
                .0
                .as_ref()
                .into_iter()
                .chain(&workload_info.batch_info[parallel_position].systems.1)
            {
                check_conflict(other_system, &borrow_constraints, &mut conflict);

                if conflict.is_some() {
                    break;
                }
            }
        }
    }

    let system_info = SystemInfo {
        name: format!("{:?}", display_name),
        type_id,
        borrow: borrow_constraints,
        conflict,
        before: memoize_before[&index].to_string_vec(),
        after: memoize_after[&index].to_string_vec(),
    };

    if !can_go_in || system_info.conflict.is_some() {
        batches.parallel.insert(
            parallel_position,
            if single_system {
                (Some(system_index), Vec::new())
            } else {
                (None, vec![system_index])
            },
        );
        batches.parallel_run_if.insert(
            parallel_position,
            if single_system {
                (Some(batches.sequential_run_if.len() - 1), Vec::new())
            } else {
                (None, vec![batches.sequential_run_if.len() - 1])
            },
        );
        par_system_index_map.insert(
            parallel_position,
            if single_system {
                (Some(index), Vec::new())
            } else {
                (None, vec![index])
            },
        );

        workload_info.batch_info.insert(
            parallel_position,
            BatchInfo {
                systems: if single_system {
                    (Some(system_info), Vec::new())
                } else {
                    (None, vec![system_info])
                },
            },
        );
    } else if single_system {
        batches.parallel[parallel_position].0 = Some(system_index);
        batches.parallel_run_if[parallel_position].0 = Some(batches.sequential_run_if.len() - 1);
        par_system_index_map[parallel_position].0 = Some(index);
        workload_info.batch_info[parallel_position].systems.0 = Some(system_info);
    } else {
        batches.parallel[parallel_position].1.push(system_index);
        batches.parallel_run_if[parallel_position]
            .1
            .push(batches.sequential_run_if.len() - 1);
        par_system_index_map[parallel_position].1.push(index);
        workload_info.batch_info[parallel_position]
            .systems
            .1
            .push(system_info);
    }

    Ok(())
}

fn valid_sequential(
    index: usize,
    memoize_before: &ShipHashMap<usize, DedupedLabels>,
    memoize_after: &ShipHashMap<usize, DedupedLabels>,
    sequential_len: usize,
    collected_tags: &[Vec<Box<dyn Label>>],
    system_index_map: &[usize],
    display_name: &dyn Label,
) -> Result<usize, error::ImpossibleRequirements> {
    let mut valid_start = 0;
    let mut valid_end = sequential_len;

    let before = &memoize_before[&index];
    let after = &memoize_after[&index];

    for other_index in 0..sequential_len {
        let other_tags = &collected_tags[system_index_map[other_index]];

        if before.iter().any(|system| other_tags.contains(system)) {
            break;
        } else {
            valid_start += 1;
        }
    }
    for other_index in (0..sequential_len).rev() {
        let other_tags = &collected_tags[system_index_map[other_index]];

        if after.iter().any(|system| other_tags.contains(system)) {
            break;
        } else {
            valid_end -= 1;
        }
    }

    if valid_start < valid_end {
        return Err(error::ImpossibleRequirements::ImpossibleConstraints(
            display_name.dyn_clone(),
            before.iter().cloned().collect(),
            after.iter().cloned().collect(),
        ));
    }

    Ok(valid_start)
}

fn valid_parallel(
    index: usize,
    memoize_before: &ShipHashMap<usize, DedupedLabels>,
    memoize_after: &ShipHashMap<usize, DedupedLabels>,
    parallel: &[(Option<usize>, Vec<usize>)],
    collected_tags: &[Vec<Box<dyn Label>>],
    system_index_map: &[(Option<usize>, Vec<usize>)],
    display_name: &dyn Label,
) -> Result<(usize, bool), error::ImpossibleRequirements> {
    let mut valid_start = 0;
    let mut valid_end = parallel.len();

    let before = &memoize_before[&index];
    let after = &memoize_after[&index];

    'outer_before: for (i, (other_single, other_indices)) in parallel.iter().enumerate() {
        if other_single.is_some() {
            let other_tags = &collected_tags[system_index_map[i].0.unwrap()];

            if before
                .iter()
                .any(|before_requirement| other_tags.contains(before_requirement))
            {
                break 'outer_before;
            }
        }

        for other_index in 0..other_indices.len() {
            let other_tags = &collected_tags[system_index_map[i].1[other_index]];

            if before
                .iter()
                .any(|before_requirement| other_tags.contains(before_requirement))
            {
                break 'outer_before;
            }
        }

        valid_start += 1;
    }

    'outer_after: for (i, (other_single, other_indices)) in parallel.iter().enumerate().rev() {
        if other_single.is_some() {
            let other_tags = &collected_tags[system_index_map[i].0.unwrap()];

            if after
                .iter()
                .any(|after_requirement| other_tags.contains(after_requirement))
            {
                break 'outer_after;
            }
        }

        for other_index in 0..other_indices.len() {
            let other_tags = &collected_tags[system_index_map[i].1[other_index]];

            if after
                .iter()
                .any(|after_requirement| other_tags.contains(after_requirement))
            {
                break 'outer_after;
            }
        }

        valid_end -= 1;
    }

    if valid_start < valid_end {
        return Err(error::ImpossibleRequirements::ImpossibleConstraints(
            display_name.dyn_clone(),
            before.iter().cloned().collect(),
            after.iter().cloned().collect(),
        ));
    }

    if valid_start == valid_end {
        Ok((valid_start, false))
    } else {
        Ok((valid_end, true))
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn insert_system_in_scheduler(
    mut system: WorkloadSystem,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync>>,
    lookup_table: &mut ShipHashMap<TypeId, usize>,
    collected_systems: &mut Vec<(usize, WorkloadSystem)>,
    system_generators: &mut Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync>>,
    system_names: &mut Vec<Box<dyn Label>>,
) {
    let system_index = *lookup_table.entry(system.type_id).or_insert_with(|| {
        let system_fn = core::mem::replace(&mut system.system_fn, Box::new(|_| Ok(())));
        let generator = core::mem::replace(&mut system.generator, Box::new(|_| TypeId::of::<()>()));

        systems.push(system_fn);
        system_names.push(system.display_name.clone());
        system_generators.push(generator);
        systems.len() - 1
    });

    collected_systems.push((system_index, system));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Unique};
    use crate::{
        AllStoragesViewMut, IntoWorkload, SystemModificator, UniqueView, UniqueViewMut, View,
        WorkloadModificator,
    };

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
                parallel_run_if: Vec::new(),
                sequential: vec![0],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0],
                sequential_run_if: Vec::new(),
                run_if: None,
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_immutable() {
        use crate::{scheduler::IntoWorkloadSystem, View, World};

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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1, 2],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 0],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[cfg(feature = "thread_local")]
    #[test]
    fn non_send() {
        use crate::{track, NonSend, View, ViewMut, World};

        #[allow(unused)]
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

        Workload::new("Test")
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 0],
                sequential_run_if: Vec::new(),
                run_if: None,
            })
        );
        assert_eq!(&scheduler.default, &label);
        assert!(scheduler.workloads_info[&label].batch_info[0].systems.1[0]
            .conflict
            .is_none());

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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
            })
        );
        assert_eq!(&scheduler.default, &label);

        let world = World::new();

        Workload::new("Test")
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
            })
        );
        assert_eq!(&scheduler.default, &label);
        assert!(scheduler.workloads_info[&label].batch_info[0].systems.1[0]
            .conflict
            .is_none());

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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![],
                sequential_run_if: Vec::new(),
                run_if: None,
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
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1, 2, 3],
                sequential_run_if: Vec::new(),
                run_if: None,
            })
        );
        assert_eq!(&scheduler.default, &label);
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

        world.run_default_workload().unwrap();
    }

    #[test]
    fn system_skip_if_missing_storage() {
        let world = World::new();

        Workload::new("test")
            .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
            .build()
            .unwrap()
            .0
            .run_with_world(&world)
            .unwrap();

        Workload::new("test")
            .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
            .add_to_world(&world)
            .unwrap();

        world.run_default_workload().unwrap();
    }

    #[test]
    fn skip_if_empty_storage() {
        let mut world = World::new();

        let eid = world.add_entity((Usize(0),));
        world.remove::<(Usize,)>(eid);

        Workload::new("test")
            .skip_if_storage_empty::<Usize>()
            .with_system(|| -> () { panic!() })
            .build()
            .unwrap()
            .0
            .run_with_world(&world)
            .unwrap();

        Workload::new("test")
            .skip_if_storage_empty::<Usize>()
            .with_system(|| -> () { panic!() })
            .add_to_world(&world)
            .unwrap();

        world.run_default_workload().unwrap();
    }

    #[test]
    fn system_skip_if_empty_storage() {
        let mut world = World::new();

        let eid = world.add_entity((Usize(0),));
        world.remove::<(Usize,)>(eid);

        Workload::new("test")
            .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
            .build()
            .unwrap()
            .0
            .run_with_world(&world)
            .unwrap();

        Workload::new("test")
            .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
            .add_to_world(&world)
            .unwrap();

        world.run_default_workload().unwrap();
    }

    #[test]
    fn workload_merge_skip_if() {
        let world = World::new();

        world.add_unique(Usize(0));

        world.add_workload(|| {
            (
                (|| -> () { panic!() })
                    .into_workload()
                    .skip_if_missing_unique::<U32>(),
                (|mut u: UniqueViewMut<'_, Usize>| u.0 += 1).into_workload(),
            )
        });

        world.run_default_workload().unwrap();

        assert_eq!(world.borrow::<UniqueView<'_, Usize>>().unwrap().0, 1);
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
        assert!(batches.sequential == [0, 1] || batches.sequential == [1, 0]);
        assert_eq!(batches.parallel, &[(None, vec![0, 1])]);
    }

    #[test]
    fn before_after_system_label() {
        fn a() {}
        fn b() {}
        fn c() {}

        let (workload, _) = Workload::new("")
            .with_system(a.tag("a"))
            .with_system(
                b.tag("b")
                    .before_all("a")
                    .require_after("a")
                    .require_before("c"),
            )
            .with_system(c.tag("c").before_all("b").require_after("b"))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[2, 1, 0]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![2]), (None, vec![1]), (None, vec![0])]
        );

        let (workload, _) = Workload::new("")
            .with_system(c.tag("c").after_all("b"))
            .with_system(b.tag("b").after_all("a"))
            .with_system(a.tag("a"))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[2, 1, 0]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![2]), (None, vec![1]), (None, vec![0])]
        );
    }

    #[test]
    fn after_all_single_system() {
        let (workload, _) = Workload::new("")
            .with_system((|| {}).tag("this"))
            .with_system((|_: AllStoragesViewMut<'_>| {}).after_all("this"))
            .with_system((|_: View<'_, Usize>| {}).after_all("this"))
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];

        assert_eq!(
            batches,
            &Batches {
                parallel: vec![(None, vec![0]), (None, vec![2]), (Some(1), vec![])],
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1, 2],
                sequential_run_if: Vec::new(),
                run_if: None,
            }
        );
    }

    #[test]
    fn sequential_workload() {
        fn sys0() {}
        fn sys1() {}
        fn sys2() {}
        fn workload1() -> Workload {
            (sys0, sys1, sys2).into_sequential_workload()
        }

        let (workload, _) = workload1().rename("").build().unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(batches.sequential, &[0, 1, 2]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![0]), (None, vec![1]), (None, vec![2])]
        );
    }

    #[test]
    fn before_after_borrow_conflict() {
        fn sys0(_: View<'_, U32>) {}
        fn sys1(_: AllStoragesViewMut<'_>) {}

        let (workload, _) = (sys0, sys1.before_all("not present"), sys0)
            .into_workload()
            .rename("")
            .build()
            .unwrap();

        let batches = &workload.workloads[&"".as_label()];
        assert_eq!(
            batches.parallel,
            &[(None, vec![0]), (Some(1), Vec::new()), (None, vec![0])]
        );
        assert_eq!(batches.sequential, &[0, 1, 0]);
    }

    #[test]
    fn contains() {
        fn type_name_of<T: 'static>(_: &T) -> &'static str {
            type_name::<T>()
        }

        fn w() -> Workload {
            (|| {}).into_workload()
        }
        let world = World::builder()
            .with_custom_lock::<parking_lot::RawRwLock>()
            .build();
        world.add_workload(w);
        assert!(world.contains_workload(WorkloadLabel {
            type_id: TypeId::of_val(&w),
            name: type_name_of(&w).as_label()
        }));
        assert!(world.contains_workload(w));
        world.run_workload(w).unwrap();
    }

    #[test]
    fn barrier() {
        let workload = Workload::new("")
            .with_system(|| {})
            .with_system(|| {})
            .with_barrier()
            .with_system(|| {})
            .with_system(|| {})
            .with_barrier()
            .with_system(|| {})
            .with_system(|| {})
            .build()
            .unwrap();

        assert_eq!(workload.1.batch_info.len(), 3);

        let workload = Workload::new("")
            .with_barrier()
            .with_system(|| {})
            .with_system(|| {})
            .build()
            .unwrap();

        assert_eq!(workload.1.batch_info.len(), 1);

        let workload = Workload::new("")
            .with_system(|| {})
            .with_system(|| {})
            .with_barrier()
            .build()
            .unwrap();

        assert_eq!(workload.1.batch_info.len(), 1);
    }

    #[test]
    fn with_system_return_type() {
        Workload::new("").with_system(|| 0usize).build().unwrap();
    }

    #[test]
    fn try_system_run_if() {
        fn try_sys() -> Result<(), error::MissingComponent> {
            Err(error::MissingComponent {
                id: crate::EntityId::dead(),
                name: "",
            })
        }

        let (workload, _) = Workload::new("")
            .with_try_system(try_sys.into_workload_try_system().unwrap().run_if(|| true))
            .build()
            .unwrap();

        let world = World::new();

        assert!(workload
            .run_with_world(&world)
            .err()
            .unwrap()
            .custom_error()
            .is_some());
    }
}
