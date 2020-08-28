use crate::borrow::Mutation;
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
use hashbrown::hash_map::Entry;

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
    borrow_info: Vec<(TypeId, Mutation)>,
    name: Cow<'static, str>,
}

impl WorkloadBuilder {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: struct.WorkloadBuilder.html
    ///
    /// ### Example
    /// ```
    /// use shipyard::{system, EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, Workload, World};
    ///
    /// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
    ///     for (x, &y) in (&mut usizes, &u32s).iter() {
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
    /// Adds a system to the workload been created.  
    /// It is strongly recommanded to use the [system] and [try_system] macros.  
    /// If the two functions in the tuple don't match, the workload could fail to run every time.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{system, EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, Workload, World};
    ///
    /// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
    ///     for (x, &y) in (&mut usizes, &u32s).iter() {
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

        if borrows.contains(&(TypeId::of::<AllStorages>(), Mutation::Unique)) && borrows.len() > 1 {
            return Err(error::InvalidSystem::AllStorages);
        }

        let mid = borrows.len() / 2 + (borrows.len() % 2 != 0) as usize;

        for (a_type_id, a_borrow) in &borrows[..mid] {
            for (b_type_id, b_borrow) in &borrows[mid..] {
                if a_type_id == b_type_id {
                    match (a_borrow, b_borrow) {
                        (Mutation::Unique, Mutation::Unique) => {
                            return Err(error::InvalidSystem::MultipleViewsMut)
                        }
                        (Mutation::Unique, Mutation::Shared)
                        | (Mutation::Shared, Mutation::Unique) => {
                            return Err(error::InvalidSystem::MultipleViews)
                        }
                        (Mutation::Shared, Mutation::Shared) => {}
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
    /// Adds a system to the workload been created.  
    /// It is strongly recommanded to use the [system] and [try_system] macros.  
    /// If the two functions in the tuple don't match, the workload could fail to run every time.  
    /// Unwraps errors.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{system, EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, Workload, World};
    ///
    /// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
    ///     for (x, &y) in (&mut usizes, &u32s).iter() {
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
    /// Finishes the workload creation and store it in the [`World`].
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

        if self.systems.len() == 1 {
            let (type_id, system_name, _, _, system) = self.systems.pop().unwrap();

            let mut name = "".into();
            core::mem::swap(&mut name, &mut self.name);

            let range = scheduler.batch.len()..(scheduler.batch.len() + 1);

            match scheduler.workloads.entry(name) {
                hashbrown::hash_map::Entry::Occupied(_) => {
                    return Err(error::AddWorkload::AlreadyExists);
                }
                hashbrown::hash_map::Entry::Vacant(entry) => {
                    entry.insert(range.clone());
                }
            }

            if scheduler.workloads.len() == 1 {
                scheduler.default = range;
            }

            let len = scheduler.systems.len();
            let system_index = match scheduler.lookup_table.entry(type_id) {
                Entry::Vacant(vacant) => {
                    vacant.insert(len);
                    scheduler.systems.push(system);
                    scheduler.system_names.push(system_name);
                    scheduler.systems.len() - 1
                }
                Entry::Occupied(occupied) => *occupied.get(),
            };

            scheduler.batch.push(Box::new([system_index]));
        } else {
            if scheduler.workloads.contains_key(&self.name) {
                return Err(error::AddWorkload::AlreadyExists);
            }

            let batch_start = scheduler.batch.len();
            let mut new_batch = vec![Vec::new()];
            let mut batch_info = vec![Vec::new()];

            for (type_id, name, info_range, is_send_sync, system) in self.systems.drain(..) {
                let len = scheduler.systems.len();
                let system_index = match scheduler.lookup_table.entry(type_id) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(len);
                        scheduler.systems.push(system);
                        scheduler.system_names.push(name);
                        scheduler.systems.len() - 1
                    }
                    Entry::Occupied(occupied) => *occupied.get(),
                };

                if is_send_sync {
                    let mut batch_index = new_batch.len();
                    for batch in batch_info.iter().rev() {
                        let mut conflict = false;
                        for &(type_id, mutation) in &self.borrow_info[info_range.clone()] {
                            match mutation {
                                Mutation::Shared => {
                                    for &(batch_type_id, mutation) in batch.iter() {
                                        #[cfg(feature = "parallel")]
                                        {
                                            if type_id == batch_type_id
                                                && mutation == Mutation::Unique
                                                || type_id == TypeId::of::<AllStorages>()
                                                || batch_type_id == TypeId::of::<AllStorages>()
                                            {
                                                conflict = true;
                                                break;
                                            }
                                        }
                                        #[cfg(not(feature = "parallel"))]
                                        {
                                            if type_id == batch_type_id
                                                && mutation == Mutation::Unique
                                                || type_id == TypeId::of::<AllStorages>()
                                                || batch_type_id == TypeId::of::<AllStorages>()
                                            {
                                                conflict = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                Mutation::Unique => {
                                    for &(batch_type_id, _) in batch.iter() {
                                        #[cfg(feature = "parallel")]
                                        {
                                            if type_id == batch_type_id
                                                || type_id == TypeId::of::<AllStorages>()
                                                || batch_type_id == TypeId::of::<AllStorages>()
                                            {
                                                conflict = true;
                                                break;
                                            }
                                        }
                                        #[cfg(not(feature = "parallel"))]
                                        {
                                            if type_id == batch_type_id
                                                || type_id == TypeId::of::<AllStorages>()
                                                || batch_type_id == TypeId::of::<AllStorages>()
                                            {
                                                conflict = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if conflict {
                            break;
                        } else {
                            batch_index -= 1;
                        }
                    }

                    if batch_index == batch_info.len() {
                        new_batch.push(vec![system_index]);
                        batch_info.push(self.borrow_info[info_range].to_vec());
                    } else {
                        new_batch[batch_index].push(system_index);
                        batch_info[batch_index].extend_from_slice(&self.borrow_info[info_range]);
                    }
                } else {
                    let last = new_batch.last_mut().unwrap();
                    if last.is_empty() {
                        last.push(system_index);
                        new_batch.push(Vec::new());
                        batch_info
                            .last_mut()
                            .unwrap()
                            .push((TypeId::of::<AllStorages>(), Mutation::Unique));
                        batch_info.push(Vec::new());
                    } else {
                        new_batch.push(vec![system_index]);
                        new_batch.push(Vec::new());
                        batch_info.push(vec![(TypeId::of::<AllStorages>(), Mutation::Unique)]);
                        batch_info.push(Vec::new());
                    }
                }
            }

            if new_batch.last().unwrap().is_empty() {
                new_batch.pop();
            }

            scheduler
                .batch
                .extend(new_batch.into_iter().map(Vec::into_boxed_slice));

            if scheduler.workloads.is_empty() {
                scheduler.default = batch_start..(scheduler.batch.len());
            }

            let mut name = "".into();
            core::mem::swap(&mut name, &mut self.name);
            let len = scheduler.batch.len();
            scheduler.workloads.insert(name, batch_start..len);
        }

        Ok(())
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
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
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
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
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
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0, 1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);
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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);
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
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);
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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

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
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);
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
            |world: &World| world.try_run(|_: FakeBorrow<usize>| {}),
            |_: FakeBorrow<usize>| {},
        ))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 3);
    assert_eq!(scheduler.batch.len(), 3);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(&*scheduler.batch[2], &[2]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..3)));
    assert_eq!(scheduler.default, 0..3);
}
