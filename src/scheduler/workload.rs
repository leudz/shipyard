mod create_workload;
mod prepare;
#[cfg(test)]
mod tests;
// Rustdoc doesn't set `test` when it collects doc tests, these checks need their own gate.
#[cfg(doctest)]
#[path = "workload/tests/compile_fail.rs"]
mod compile_fail_tests;

use crate::all_storages::{AllStorages, StorageMaps};
use crate::component::{Component, Unique};
use crate::scheduler::info::{DedupedLabels, TypeInfo, WorkloadInfo};
use crate::scheduler::label::WorkloadLabel;
use crate::scheduler::system::{ExtractWorkloadRunIf, WorkloadRunIf};
use crate::scheduler::{
    AsLabel, Batches, IntoWorkload, IntoWorkloadSystem, Label, Scheduler, WorkloadSystem,
};
use crate::unique::UniqueStorage;
use crate::world::World;
use crate::{ShipHashMap, error};
use alloc::boxed::Box;
use core::any::TypeId;
// macro not module
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;

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
    batches: Batches,
}

impl ScheduledWorkload {
    /// Runs the workload.
    ///
    /// It doesn't regroup, see [`Workload::build`].
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
        world.run_batches(&self.systems, &self.system_names, &self.batches, &self.name)
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
    ///
    /// Like [`Workload::add_to_world`], the workload regroups the world's groups it uses,
    /// see [`WorldBuilder::with_group`](crate::world::WorldBuilder::with_group).
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

/// Holds information to schedule a group of systems.
///
/// A workload is a collection of systems. They will execute as much in parallel as possible.\
/// They are evaluated first to last when they can't be parallelized.\
/// The default workload will automatically be set to the first workload added.
pub struct Workload {
    pub(super) name: Box<dyn Label>,
    pub(super) tags: Vec<Box<dyn Label>>,
    pub(super) systems: Vec<WorkloadSystem>,
    pub(super) run_if: Option<WorkloadRunIf>,
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
    ///
    /// `other`'s barriers move along with its systems.\
    /// Its conditions, tags and ordering requirements stay in `other`, [`merge`](Workload::merge) applies them to its systems first.
    pub fn append(mut self, other: &mut Self) -> Self {
        // Barriers are positions in the systems list, `other`'s ones now start after our systems.
        let systems_len = self.systems.len();
        self.barriers.extend(
            other
                .barriers
                .drain(..)
                .map(|barrier| barrier + systems_len),
        );

        for system in &mut other.systems {
            system.unique_id += systems_len;
        }

        self.systems.append(&mut other.systems);

        self
    }
    /// Propagates all information from `self` and `other` into their respective systems before merging their systems.
    /// This includes `run_if`/`skip_if`, `tags`, `before`/`after` requirements.
    pub fn merge(mut self, mut other: Workload) -> Workload {
        self.propagate();
        other.propagate();

        self.append(&mut other)
    }
    /// Propagates all information into the systems.
    /// This includes `run_if`/`skip_if`, `tags`, `before`/`after` requirements.
    fn propagate(&mut self) {
        for system in &mut self.systems {
            if let Some(workload_run_if) = &self.run_if {
                // Each system gets its own copy of the condition, cloning gives it its own tracking cursor.
                // We keep that copy across runs, so it sees the changes since this system's last evaluation.
                let workload_fn = workload_run_if.system_fn.clone();
                system.run_if = match system.run_if.take() {
                    None => Some(workload_fn.to_non_clone()),
                    Some(system_run_if) => Some(Box::new(move |world| {
                        Ok(workload_fn.run(world)? && (system_run_if)(world)?)
                    })),
                };
                system
                    .run_if_borrow_constraints
                    .extend(workload_run_if.borrow_constraints.iter().cloned());
                system
                    .tracking_to_enable
                    .extend(workload_run_if.tracking_to_enable.iter().copied());
            }

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
    ///
    /// ### Return value
    ///
    /// A system can return any type, the value is dropped.\
    /// The exception is exactly `Result<(), SystemError>`, or an alias of it: an `Err` stops the workload
    /// and is returned as [`RunWorkload::Run`](error::RunWorkload::Run).\
    /// Any other type is dropped, including other `Result`s, use `?` to convert their error into a [`SystemError`](error::SystemError).
    ///
    /// ```
    /// use shipyard::error::SystemError;
    /// use shipyard::{Component, IntoIter, View, Workload, World};
    ///
    /// #[derive(Component)]
    /// struct Health(u32);
    ///
    /// fn check(healths: View<Health>) -> Result<(), SystemError> {
    ///     if healths.iter().any(|health| health.0 == 0) {
    ///         return Err("a health reached zero".into());
    ///     }
    ///
    ///     Ok(())
    /// }
    ///
    /// let mut world = World::new();
    ///
    /// world.add_entity(Health(0));
    ///
    /// Workload::new("Check")
    ///     .with_system(check)
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// let error = world.run_workload("Check").unwrap_err();
    ///
    /// assert_eq!(
    ///     error.custom_error().unwrap().to_string(),
    ///     "a health reached zero"
    /// );
    /// ```
    ///
    /// ### Shared instance
    ///
    /// A world registers each function type once, the first time a workload containing it is added.\
    /// Every later occurrence of that type runs this first instance.\
    /// This matters for closures: different captures of the same closure type all run the first one.
    ///
    /// ```
    /// use shipyard::{Workload, World};
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicU32, Ordering};
    ///
    /// // Every call returns the same closure type, only the captured counter differs.
    /// fn counting(count: &Arc<AtomicU32>) -> impl Fn() + Send + Sync + 'static {
    ///     let count = count.clone();
    ///
    ///     move || {
    ///         count.fetch_add(1, Ordering::Relaxed);
    ///     }
    /// }
    ///
    /// let world = World::new();
    /// let first = Arc::new(AtomicU32::new(0));
    /// let second = Arc::new(AtomicU32::new(0));
    ///
    /// Workload::new("first")
    ///     .with_system(counting(&first))
    ///     .add_to_world(&world)
    ///     .unwrap();
    /// Workload::new("second")
    ///     .with_system(counting(&second))
    ///     .add_to_world(&world)
    ///     .unwrap();
    ///
    /// world.run_workload("second").unwrap();
    ///
    /// // "second" ran the closure registered by "first".
    /// assert_eq!(first.load(Ordering::Relaxed), 1);
    /// assert_eq!(second.load(Ordering::Relaxed), 0);
    /// ```
    ///
    /// To give each occurrence its own data, store it in the [`World`], in a unique for example.
    #[track_caller]
    pub fn with_system<B, R, S: IntoWorkloadSystem<B, R>>(mut self, system: S) -> Workload {
        let mut system = system.into_workload_system().unwrap();
        system.unique_id += self.systems.len();

        self.systems.push(system);

        self
    }
    /// Finishes the workload creation and stores it in the [`World`].
    /// Returns a struct with describing how the workload has been split in batches.
    ///
    /// The workload regroups the world's groups it uses, the regroups are planned here.\
    /// See [`WorldBuilder::with_group`](crate::world::WorldBuilder::with_group) for when they run.
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

        // We borrow it before adding the workload, it can't be added without its regroups.
        let all_storages = world
            .all_storages()
            .map_err(|_| error::AddWorkload::TrackingAllStoragesBorrow)?;

        if workloads.contains_key(&self.name) {
            return Err(error::AddWorkload::AlreadyExists);
        }

        let mut tracking_to_enable = Vec::new();

        let name = self.name.dyn_clone();

        let registered = systems.len();

        // We only publish the workload once every step that can fail succeeded,
        // a rejected workload doesn't show up in `workloads`, `default` or `workloads_info`.
        let built = create_workload::create_workload(
            self,
            systems,
            system_names,
            system_generators,
            lookup_table,
            &mut tracking_to_enable,
        )
        .and_then(|(mut batches, mut workload_info, dependencies)| {
            prepare::prepare_regroups(
                &mut batches,
                &mut workload_info.batches_info,
                &all_storages.grouping,
                &dependencies,
            );

            for enable_tracking_fn in &tracking_to_enable {
                (enable_tracking_fn)(&all_storages).map_err(|err| match err {
                    error::GetStorage::StorageBorrow { name, id, borrow } => {
                        error::AddWorkload::TrackingStorageBorrow { name, id, borrow }
                    }
                    _ => unreachable!(),
                })?;
            }

            Ok((batches, workload_info))
        });

        let (batches, workload_info) = match built {
            Ok(built) => built,
            Err(err) => {
                // The rejected workload's callables can't stay registered, a later workload would reuse them.
                // Callables are only ever appended and we hold the scheduler the whole time,
                // so the ones from earlier workloads are exactly those below `registered`.
                // Tracking enabled by the callbacks that succeeded stays enabled, that's harmless.
                systems.truncate(registered);
                system_names.truncate(registered);
                system_generators.truncate(registered);
                lookup_table.retain(|_, index| *index < registered);

                return Err(err);
            }
        };

        if workloads.is_empty() {
            *default = name.clone();
        }
        workloads.insert(name.clone(), batches);
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
    ///
    /// Unlike [`Workload::add_to_world`], the built workload doesn't regroup, it isn't tied to a world's groups.\
    /// Iteration stays correct, but components added since the last regroup are slower to iterate.\
    /// Call [`World::regroup`] or [`World::regroup_set`] when it matters.
    pub fn build(self) -> Result<(ScheduledWorkload, WorkloadInfo), error::AddWorkload> {
        let (workload, workload_info, _) = self.build_unprepared()?;

        Ok((workload, workload_info))
    }

    /// Builds the workload like [`Workload::build`], with what
    /// [`prepare_regroups`](prepare::prepare_regroups) needs to add regroups to it.
    fn build_unprepared(
        self,
    ) -> Result<
        (
            ScheduledWorkload,
            WorkloadInfo,
            create_workload::Dependencies,
        ),
        error::AddWorkload,
    > {
        let mut workload = ScheduledWorkload {
            name: self.name.clone(),
            systems: Vec::new(),
            system_names: Vec::new(),
            system_generators: Vec::new(),
            lookup_table: ShipHashMap::new(),
            tracking_to_enable: Vec::new(),
            batches: Batches::default(),
        };

        let (batches, workload_info, dependencies) = create_workload::create_workload(
            self,
            &mut workload.systems,
            &mut workload.system_names,
            &mut workload.system_generators,
            &mut workload.lookup_table,
            &mut workload.tracking_to_enable,
        )?;

        workload.batches = batches;

        Ok((workload, workload_info, dependencies))
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
    storages: &StorageMaps,
) -> Option<Result<(), error::UniquePresence>> {
    let WorkloadSystem {
        borrow_constraints, ..
    } = system;

    for type_info in borrow_constraints {
        if type_info.name.starts_with(unique_name)
            && storages.unique(&type_info.storage_id).is_none()
        {
            return Some(Err(error::UniquePresence::Unique(type_info.clone())));
        }
    }

    None
}
