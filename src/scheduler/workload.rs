mod create_workload;
#[cfg(test)]
mod tests;

use crate::all_storages::AllStorages;
use crate::component::{Component, Unique};
use crate::scheduler::info::{DedupedLabels, TypeInfo, WorkloadInfo};
use crate::scheduler::label::WorkloadLabel;
use crate::scheduler::system::{ExtractWorkloadRunIf, WorkloadRunIfFn};
use crate::scheduler::{
    AsLabel, Batches, IntoWorkload, IntoWorkloadSystem, IntoWorkloadTrySystem, Label, Scheduler,
    WorkloadSystem,
};
use crate::storage::StorageId;
use crate::unique::UniqueStorage;
use crate::world::World;
use crate::{error, ShipHashMap};
use alloc::boxed::Box;
use core::any::TypeId;
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
    batches: Batches,
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
        for system in &mut other.systems {
            system.unique_id += self.systems.len();
        }

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
        let mut system = system.into_workload_system().unwrap();
        system.unique_id += self.systems.len();

        self.systems.push(system);

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
        let mut system = system.into_workload_try_system::<Ok, Err>().unwrap();
        system.unique_id += self.systems.len();

        self.systems.push(system);

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
        let mut system = system.into_workload_try_system::<Ok, Err>().unwrap();
        system.unique_id += self.systems.len();

        self.systems.push(system);

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

        let workload_info = create_workload::create_workload(
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
            batches: Batches::default(),
        };

        let workload_name = self.name.clone();
        let mut default: Box<dyn Label> = Box::new("");

        let mut workloads = ShipHashMap::new();

        let workload_info = create_workload::create_workload(
            self,
            &mut workload.systems,
            &mut workload.system_names,
            &mut workload.system_generators,
            &mut workload.lookup_table,
            &mut workload.tracking_to_enable,
            &mut workloads,
            &mut default,
        )?;

        workload.batches = workloads.remove(&workload_name).unwrap();

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
