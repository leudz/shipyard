mod create_workload;

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
use crate::type_id::TypeId;
use crate::unique::UniqueStorage;
use crate::world::World;
use crate::{error, ShipHashMap};
use alloc::boxed::Box;
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

/// Keeps information to create a workload.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, Unique};
    use crate::{
        scheduler::Batches, AllStoragesViewMut, IntoWorkload, SystemModificator, UniqueView,
        UniqueViewMut, View, ViewMut, WorkloadModificator, World,
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn single_mutable() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_immutable() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_mutable() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn multiple_mixed() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn append_optimizes_batches() {
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
                parallel: vec![(None, vec![0]), (None, vec![1, 2])],
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1, 2],
                sequential_run_if: Vec::new(),
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn all_storages() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[cfg(feature = "thread_local")]
    #[test]
    fn non_send() {
        use crate::NonSend;

        #[allow(unused)]
        struct NotSend(*const ());
        unsafe impl Sync for NotSend {}
        impl Component for NotSend {
            type Tracking = crate::track::Untracked;
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
        assert!(
            scheduler.workloads_info[&label].batches_info[0].systems.1[0]
                .conflict
                .is_none()
        );

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
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
        assert!(
            scheduler.workloads_info[&label].batches_info[0].systems.1[0]
                .conflict
                .is_none()
        );

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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn unique_and_non_unique() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn empty_workload() {
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
                workload_run_if: None,
                systems_run_if: Vec::new(),
            })
        );
        assert_eq!(&scheduler.default, &label);
    }

    #[test]
    fn append_ensures_multiple_batches_can_be_optimized_over() {
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
                parallel: vec![(None, vec![0]), (None, vec![1, 2, 3])],
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1, 2, 3],
                sequential_run_if: Vec::new(),
                workload_run_if: None,
                systems_run_if: Vec::new(),
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
        fn d() {}

        let (workload, _) = Workload::new("")
            .with_system(d.after_all(b))
            .with_system(c.after_all(b))
            .with_system(b.after_all(a))
            .with_system(a)
            .build()
            .unwrap();

        let batches = &workload.batches;
        assert_eq!(batches.sequential, &[3, 2, 0, 1]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![3]), (None, vec![2]), (None, vec![0, 1])]
        );

        let (workload, _) = Workload::new("")
            .with_system(a)
            .with_system(b.after_all(a))
            .with_system(c.after_all(b))
            .build()
            .unwrap();

        let batches = &workload.batches;
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

        let batches = &workload.batches;
        assert_eq!(batches.sequential, &[1, 0, 2]);
        assert_eq!(
            batches.parallel,
            &[(None, vec![1]), (None, vec![0]), (None, vec![2])]
        );
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

        let batches = &workload.batches;
        assert_eq!(batches.sequential, &[0, 1]);
        assert_eq!(batches.parallel, &[(None, vec![0]), (None, vec![1])]);

        let (workload, _) = Workload::new("")
            .with_system(b.after_all(a))
            .with_system(a.before_all(b))
            .build()
            .unwrap();

        let batches = &workload.batches;
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

        let batches = &workload.batches;
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
        let batches = &workload.batches;
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

        let batches = &workload.batches;
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

        let batches = &workload.batches;
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

        let batches = &workload.batches;

        assert_eq!(
            batches,
            &Batches {
                parallel: vec![(None, vec![0]), (Some(1), vec![]), (None, vec![2])],
                parallel_run_if: Vec::new(),
                sequential: vec![0, 1, 2],
                sequential_run_if: Vec::new(),
                workload_run_if: None,
                systems_run_if: Vec::new()
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

        let batches = &workload.batches;
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

        let batches = &workload.batches;
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

        assert_eq!(workload.1.batches_info.len(), 3);

        let workload = Workload::new("")
            .with_barrier()
            .with_system(|| {})
            .with_system(|| {})
            .build()
            .unwrap();

        assert_eq!(workload.1.batches_info.len(), 1);

        let workload = Workload::new("")
            .with_system(|| {})
            .with_system(|| {})
            .with_barrier()
            .build()
            .unwrap();

        assert_eq!(workload.1.batches_info.len(), 1);
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

    /// Checks that cycles created using `after_all` are properly detected.
    #[test]
    fn cycle_detection_after_all() {
        fn sys_a() {}
        fn sys_b() {}
        fn workload_cycle() -> Workload {
            (sys_a.after_all(sys_b), sys_b.after_all(sys_a)).into_workload()
        }

        let result = workload_cycle().build();

        assert!(matches!(
            result,
            Err(error::AddWorkload::ImpossibleRequirements(
                error::ImpossibleRequirements::Cycle(_)
            ))
        ));
    }

    /// Checks that cycles created using `before_all` and `after_all` are properly detected.
    ///
    /// This also affects implicit ordering.\
    /// Here `sys_b` is implicitely after `sys_a` and `sys_c` tries to be both before and after them.
    #[test]
    fn cycle_detection_before_after_all() {
        fn sys_a() {}
        fn sys_b() {}
        fn sys_c() {}
        fn workload_cycle() -> Workload {
            (sys_a, sys_b, sys_c.before_all(sys_a).after_all(sys_b)).into_workload()
        }

        let result = workload_cycle().build();

        assert!(matches!(
            result,
            Err(error::AddWorkload::ImpossibleRequirements(_))
        ));
    }

    /// Checks that `before_all` cannot override `with_barrier`.
    #[test]
    fn cycle_barrier() {
        fn sys_a() {}
        fn sys_b() {}

        let result = Workload::new("")
            .with_system(sys_a)
            .with_barrier()
            .with_system(sys_b.before_all(sys_a))
            .build();

        assert!(matches!(
            result,
            Err(error::AddWorkload::ImpossibleRequirements(
                error::ImpossibleRequirements::Cycle(_)
            ))
        ));
    }

    /// Checks that `before_all`, `after_all`, `with_barrier` and implicit ordering all play well together.
    #[test]
    fn complex_ordering() {
        fn sys_a() {}
        fn sys_b() {}
        fn sys_c() {}
        fn sys_d() {}

        let (workload, _) = Workload::new("")
            .with_system(sys_a)
            .with_system(sys_b)
            .with_system(sys_c.after_all(sys_a).before_all(sys_b))
            .with_barrier()
            .with_system(sys_d)
            .build()
            .unwrap();

        let batches = &workload.batches;
        assert_eq!(
            batches.parallel,
            &[
                (None, vec![0]),
                (None, vec![2]),
                (None, vec![1]),
                (None, vec![3])
            ]
        );
        assert_eq!(batches.sequential, &[0, 2, 1, 3]);
    }

    #[test]
    fn require_in_workload() {
        fn sys_a() {}
        fn sys_b() {}
        fn sys_c() {}

        Workload::new("")
            .with_system(sys_a.require_in_workload(sys_b))
            .with_system(sys_b)
            .build()
            .unwrap();
        Workload::new("")
            .with_system(sys_a)
            .with_system(sys_b.require_in_workload(sys_a))
            .build()
            .unwrap();

        let result = Workload::new("")
            .with_system(sys_c)
            .with_system(sys_a.require_in_workload(sys_b))
            .with_system(sys_c)
            .build();

        assert_eq!(
            result.err(),
            Some(error::AddWorkload::MissingInWorkload(
                sys_a.as_label(),
                vec![sys_b.as_label()]
            ))
        );
    }

    #[test]
    fn require_before() {
        fn sys_a() {}
        fn sys_b() {}

        Workload::new("")
            .with_system(sys_a.after_all(sys_b).require_before(sys_b))
            .with_system(sys_b)
            .build()
            .unwrap();
        Workload::new("")
            .with_system(sys_a)
            .with_system(sys_b.after_all(sys_a).require_before(sys_a))
            .build()
            .unwrap();
        Workload::new("")
            .with_system(sys_a)
            .with_barrier()
            .with_system(sys_b.require_before(sys_a))
            .build()
            .unwrap();
        Workload::new("")
            .with_system((|_: AllStoragesViewMut<'_>| {}).tag("sys_a"))
            .with_system((|_: AllStoragesViewMut<'_>| {}).require_before("sys_a"))
            .build()
            .unwrap();

        // These systems don't conflict and would run in parallel
        let result = Workload::new("")
            .with_system(sys_a)
            .with_system(sys_b.require_before(sys_a))
            .build();

        assert_eq!(
            result.err(),
            Some(error::AddWorkload::MissingBefore(
                sys_b.as_label(),
                vec![sys_a.as_label()]
            ))
        );
    }

    #[test]
    fn require_after() {
        fn sys_a() {}
        fn sys_b() {}

        Workload::new("")
            .with_system(sys_a)
            .with_system(sys_b.before_all(sys_a).require_after(sys_a))
            .build()
            .unwrap();
        Workload::new("")
            .with_system(sys_a.before_all(sys_b).require_after(sys_b))
            .with_system(sys_b)
            .build()
            .unwrap();
        Workload::new("")
            .with_system(sys_a.require_after(sys_b))
            .with_barrier()
            .with_system(sys_b)
            .build()
            .unwrap();
        Workload::new("")
            .with_system((|_: AllStoragesViewMut<'_>| {}).require_after("sys_a"))
            .with_system((|_: AllStoragesViewMut<'_>| {}).tag("sys_a"))
            .build()
            .unwrap();

        // These systems don't conflict and would run in parallel
        let result = Workload::new("")
            .with_system(sys_a.require_after(sys_b))
            .with_system(sys_b)
            .build();

        assert_eq!(
            result.err(),
            Some(error::AddWorkload::MissingAfter(
                sys_a.as_label(),
                vec![sys_b.as_label()]
            ))
        );
    }
}

/// Tests related to `WorkloadInfo` and not system ordering.
#[cfg(test)]
mod info_tests {
    use super::*;
    use crate::borrow::Mutability;
    use crate::scheduler::info::{Conflict, SystemInfo};
    use crate::scheduler::system_modificator::SystemModificator;
    use crate::sparse_set::SparseSet;
    use crate::views::{View, ViewMut};
    use alloc::format;
    use std::string::ToString;

    #[allow(unused)]
    struct Usize(usize);
    impl Component for Usize {
        type Tracking = crate::track::Untracked;
    }
    #[allow(unused)]
    struct U32(u32);
    impl Component for U32 {
        type Tracking = crate::track::Untracked;
    }

    #[test]
    fn before_all() {
        fn sys_a() {}
        fn sys_b() {}

        let (_, info) = Workload::new("workload 1")
            .with_system(sys_a)
            .with_system(sys_b.before_all(sys_a))
            .build()
            .unwrap();

        assert_eq!(info.name, "workload 1");
        assert_eq!(info.batches_info.len(), 2);

        let mut systems0 = info.batches_info[0].systems();
        assert_eq!(
            systems0.next(),
            Some(&SystemInfo {
                name: "System(shipyard::scheduler::workload::info_tests::before_all::sys_b)"
                    .to_string(),
                borrow: vec![],
                conflict: None,
                after: vec![],
                after_all: vec![],
                before_all: vec![
                    "System(shipyard::scheduler::workload::info_tests::before_all::sys_a)"
                        .to_string()
                ],
                unique_id: 1
            })
        );
        assert_eq!(systems0.next(), None);

        let mut systems1 = info.batches_info[1].systems();
        assert_eq!(
            systems1.next(),
            Some(&SystemInfo {
                name: "System(shipyard::scheduler::workload::info_tests::before_all::sys_a)"
                    .to_string(),
                borrow: vec![],
                conflict: None,
                after: vec![1],
                after_all: vec![],
                before_all: vec![],
                unique_id: 0
            })
        );
        assert_eq!(systems1.next(), None);
    }

    #[test]
    fn after_all() {
        fn sys_a() {}
        fn sys_b() {}

        let (_, info) = Workload::new("workload 1")
            .with_system(sys_a.after_all(sys_b))
            .with_system(sys_b)
            .build()
            .unwrap();

        assert_eq!(info.name, "workload 1");
        assert_eq!(info.batches_info.len(), 2);

        let mut systems0 = info.batches_info[0].systems();
        assert_eq!(
            systems0.next(),
            Some(&SystemInfo {
                name: "System(shipyard::scheduler::workload::info_tests::after_all::sys_b)"
                    .to_string(),
                borrow: vec![],
                conflict: None,
                after: vec![],
                after_all: vec![],
                before_all: vec![],
                unique_id: 1
            })
        );
        assert_eq!(systems0.next(), None);

        let mut systems1 = info.batches_info[1].systems();
        assert_eq!(
            systems1.next(),
            Some(&SystemInfo {
                name: "System(shipyard::scheduler::workload::info_tests::after_all::sys_a)"
                    .to_string(),
                borrow: vec![],
                conflict: None,
                after: vec![1],
                after_all: vec![
                    "System(shipyard::scheduler::workload::info_tests::after_all::sys_b)"
                        .to_string()
                ],
                before_all: vec![],
                unique_id: 0
            })
        );
        assert_eq!(systems1.next(), None);
    }

    #[test]
    fn borrow() {
        fn sys_a(_: View<'_, Usize>, _: View<'_, U32>) {}
        fn sys_b(_: ViewMut<'_, Usize>, _: View<'_, U32>) {}
        fn sys_c(_: View<'_, Usize>) {}
        fn sys_d(_: View<'_, Usize>) {}

        let (_, info) = Workload::new("")
            .with_system(sys_a)
            .with_system(sys_b)
            .with_system(sys_c)
            .with_system(sys_d)
            .build()
            .unwrap();

        let mut systems0 = info.batches_info[0].systems();
        assert_eq!(
            systems0.next(),
            Some(&SystemInfo {
                name: format!("{:?}", sys_a.as_label()),
                borrow: vec![
                    TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Shared,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    },
                    TypeInfo {
                        name: type_name::<SparseSet::<U32>>().into(),
                        mutability: Mutability::Shared,
                        storage_id: StorageId::of::<SparseSet<U32>>(),
                        thread_safe: true
                    }
                ],
                conflict: None,
                after: vec![],
                after_all: vec![],
                before_all: vec![],
                unique_id: 0
            })
        );

        let mut systems1 = info.batches_info[1].systems();
        assert_eq!(
            systems1.next(),
            Some(&SystemInfo {
                name: format!("{:?}", sys_b.as_label()),
                borrow: vec![
                    TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Exclusive,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    },
                    TypeInfo {
                        name: type_name::<SparseSet::<U32>>().into(),
                        mutability: Mutability::Shared,
                        storage_id: StorageId::of::<SparseSet<U32>>(),
                        thread_safe: true
                    }
                ],
                conflict: Some(Conflict::Borrow {
                    type_info: Some(TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Exclusive,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    }),
                    other_system: 0,
                    other_type_info: TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Shared,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    }
                }),
                after: vec![0],
                after_all: vec![],
                before_all: vec![],
                unique_id: 1
            })
        );

        let mut systems2 = info.batches_info[2].systems();
        assert_eq!(
            systems2.next(),
            Some(&SystemInfo {
                name: format!("{:?}", sys_c.as_label()),
                borrow: vec![TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },],
                conflict: Some(Conflict::Borrow {
                    type_info: Some(TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Shared,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    }),
                    other_system: 1,
                    other_type_info: TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Exclusive,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    }
                }),
                after: vec![1],
                after_all: vec![],
                before_all: vec![],
                unique_id: 2
            })
        );
        assert_eq!(
            systems2.next(),
            Some(&SystemInfo {
                name: format!("{:?}", sys_d.as_label()),
                borrow: vec![TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },],
                conflict: Some(Conflict::Borrow {
                    type_info: Some(TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Shared,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    }),
                    other_system: 1,
                    other_type_info: TypeInfo {
                        name: type_name::<SparseSet::<Usize>>().into(),
                        mutability: Mutability::Exclusive,
                        storage_id: StorageId::of::<SparseSet<Usize>>(),
                        thread_safe: true
                    }
                }),
                after: vec![1],
                after_all: vec![],
                before_all: vec![],
                unique_id: 3
            })
        );
    }
}
