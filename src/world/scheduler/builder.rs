use super::Scheduler;
use crate::atomic_refcell::RefMut;
use crate::borrow::Mutation;
use crate::error;
use crate::storage::AllStorages;
use crate::system::System;
use crate::world::World;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::any::type_name;
use core::any::TypeId;
use core::ops::Range;
use hashbrown::hash_map::Entry;

/// Keeps information to create a workload.
#[allow(clippy::type_complexity)]
pub struct WorkloadBuilder<'a> {
    scheduler: RefMut<'a, Scheduler>,
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

impl<'a> WorkloadBuilder<'a> {
    pub(crate) fn new(scheduler: RefMut<'a, Scheduler>, name: Cow<'static, str>) -> Self {
        WorkloadBuilder {
            scheduler,
            systems: Vec::new(),
            borrow_info: Vec::new(),
            name,
        }
    }
}

impl<'a> WorkloadBuilder<'a> {
    /// Adds a system to the workload been created.  
    /// It is strongly recommanded to use the [system] and [try_system] macros.  
    /// If the two functions in the tuple don't match, the workload could fail to run every time.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{system, EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, World};
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
    /// world
    ///     .add_workload("Add & Check")
    ///     .with_system((|world: &World| world.try_run(add), add))
    ///     .with_system(system!(check))
    ///     .build();
    ///
    /// world.run_default();
    /// ```
    ///
    /// [system]: macro.system.html
    /// [try_system]: macro.try_system.html
    pub fn try_with_system<
        B,
        R,
        F: System<'a, B, R>,
        S: Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static,
    >(
        &mut self,
        (system, _): (S, F),
    ) -> Result<&mut WorkloadBuilder<'a>, error::InvalidSystem> {
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
            core::any::TypeId::of::<S>(),
            type_name::<F>(),
            old_len..self.borrow_info.len(),
            is_send_sync,
            Box::new(system),
        ));
        Ok(self)
    }
    #[cfg(feature = "panic")]
    pub fn with_system<
        B,
        R,
        F: System<'a, B, R>,
        S: Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static,
    >(
        &mut self,
        system: (S, F),
    ) -> &mut WorkloadBuilder<'a> {
        self.try_with_system(system).unwrap()
    }
    /// Finishes the workload creation and store it in the `World`.
    pub fn build(&mut self) {
        if self.systems.len() == 1 {
            let (type_id, system_name, _, _, system) = self.systems.pop().unwrap();
            let mut name = "".into();
            core::mem::swap(&mut name, &mut self.name);
            let range = self.scheduler.batch.len()..(self.scheduler.batch.len() + 1);
            if self.scheduler.workloads.is_empty() {
                self.scheduler.default = range.clone();
            }
            self.scheduler.workloads.insert(name, range);
            let len = self.scheduler.systems.len();
            self.scheduler.batch.push(Box::new([len]));

            if let Entry::Vacant(vacant) = self.scheduler.lookup_table.entry(type_id) {
                vacant.insert(len);
                self.scheduler.system_names.push(system_name);
                self.scheduler.systems.push(system);
            }
        } else {
            let batch_start = self.scheduler.batch.len();
            let mut new_batch = vec![Vec::new()];
            let mut batch_info = vec![Vec::new()];

            for (type_id, name, info_range, is_send_sync, system) in self.systems.drain(..) {
                let len = self.scheduler.systems.len();
                let system_index = match self.scheduler.lookup_table.entry(type_id) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(len);
                        self.scheduler.systems.push(system);
                        self.scheduler.system_names.push(name);
                        self.scheduler.systems.len() - 1
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

            self.scheduler
                .batch
                .extend(new_batch.into_iter().map(Vec::into_boxed_slice));

            if self.scheduler.workloads.is_empty() {
                self.scheduler.default = batch_start..(self.scheduler.batch.len());
            }

            let mut name = "".into();
            core::mem::swap(&mut name, &mut self.name);
            let len = self.scheduler.batch.len();
            self.scheduler.workloads.insert(name, batch_start..len);
        }
    }
}

#[test]
fn single_immutable() {
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{View, World};

    fn system1(_: View<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "System1".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
}
#[test]
fn single_mutable() {
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "System1".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
}
#[test]
fn multiple_immutable() {
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0, 1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
}
#[test]
fn multiple_mutable() {
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: ViewMut<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
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
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{View, ViewMut, World};

    fn system1(_: ViewMut<'_, usize>) {}
    fn system2(_: View<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
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
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{AllStoragesViewMut, View, World};

    fn system1(_: View<'_, usize>) {}
    fn system2(_: AllStoragesViewMut<'_>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system2), system2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
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
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{NonSend, View, ViewMut, World};

    struct NotSend(*const ());
    unsafe impl Sync for NotSend {}

    fn sys1(_: NonSend<View<'_, NotSend>>) {}
    fn sys2(_: NonSend<ViewMut<'_, NotSend>>) {}
    fn sys3(_: View<'_, usize>) {}
    fn sys4(_: ViewMut<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Test".into())
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Test".into())
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys2), sys2))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Test".into())
        .try_with_system((|world: &World| world.try_run(sys2), sys2))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Test".into())
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys3), sys3))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Test".into())
        .try_with_system((|world: &World| world.try_run(sys1), sys1))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(sys4), sys4))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
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
    use crate::atomic_refcell::AtomicRefCell;
    use crate::{FakeBorrow, View, World};

    fn system1(_: View<'_, usize>) {}

    let scheduler = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(Scheduler::default(), None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(Scheduler::default())
        }
    };
    WorkloadBuilder::new(scheduler.try_borrow_mut().unwrap(), "Systems".into())
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .try_with_system((
            |world: &World| world.try_run(|_: FakeBorrow<usize>| {}),
            |_: FakeBorrow<usize>| {},
        ))
        .unwrap()
        .try_with_system((|world: &World| world.try_run(system1), system1))
        .unwrap()
        .build();

    let scheduler = scheduler.try_borrow_mut().unwrap();
    assert_eq!(scheduler.systems.len(), 3);
    assert_eq!(scheduler.batch.len(), 3);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(&*scheduler.batch[2], &[2]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..3)));
    assert_eq!(scheduler.default, 0..3);
}
