use super::Scheduler;
use crate::error;
use crate::run::{Dispatch, Mutation, System, SystemData};
use crate::storage::AllStorages;
use std::any::TypeId;
use std::borrow::Cow;
use std::collections::hash_map::Entry;

pub trait IntoWorkload {
    fn try_into_workload(
        self,
        name: impl Into<Cow<'static, str>>,
        scheduler: &mut Scheduler,
        all_storages: &AllStorages,
    ) -> Result<(), error::AddWorkload>;
}

impl<T: for<'a> System<'a> + Send + Sync + 'static> IntoWorkload for T {
    #[allow(clippy::range_plus_one)]
    fn try_into_workload(
        self,
        name: impl Into<Cow<'static, str>>,
        scheduler: &mut Scheduler,
        _: &AllStorages,
    ) -> Result<(), error::AddWorkload> {
        let range = scheduler.batch.len()..(scheduler.batch.len() + 1);
        if scheduler.workloads.is_empty() {
            scheduler.default = range.clone();
        }
        scheduler.workloads.insert(name.into(), range);
        scheduler.batch.push(Box::new([scheduler.systems.len()]));
        if let Entry::Vacant(vacant) = scheduler.lookup_table.entry(TypeId::of::<T>()) {
            vacant.insert(scheduler.systems.len());
            scheduler
                .systems
                .push(Box::new(|world| T::try_dispatch(world)));
        }
        Ok(())
    }
}

impl<T: for<'a> System<'a> + Send + Sync + 'static> IntoWorkload for (T,) {
    fn try_into_workload(
        self,
        name: impl Into<Cow<'static, str>>,
        scheduler: &mut Scheduler,
        all_storages: &AllStorages,
    ) -> Result<(), error::AddWorkload> {
        self.0.try_into_workload(name, scheduler, all_storages)
    }
}

macro_rules! impl_scheduler {
    ($(($system: ident, $index: tt))+) => {
        impl<$($system: for<'a> System<'a> + Send + Sync + 'static),+> IntoWorkload for ($($system,)+) {
            fn try_into_workload(self, name: impl Into<Cow<'static, str>>, scheduler: &mut Scheduler, all_storages: &AllStorages) -> Result<(), error::AddWorkload> {
                let batch_start = scheduler.batch.len();
                // new batch added by this workload
                let mut new_batch = vec![Vec::new()];
                // what is borrowed by each new batch
                let mut batch_info = vec![Vec::new()];

                $(
                    // register the system or retrive it's index
                    let system_index = match scheduler.lookup_table.entry(TypeId::of::<$system>()) {
                        Entry::Vacant(vacant) => {
                            vacant.insert(scheduler.systems.len());
                            scheduler.systems.push(Box::new(|world| $system::try_dispatch(world)));
                            scheduler.systems.len() - 1
                        },
                        Entry::Occupied(occupied) => *occupied.get(),
                    };
                    // for now systems with `!Send` and `!Sync` storages are run sequentially
                    if $system::Data::is_send_sync(all_storages)? {
                        // what is borrowed by this system
                        let mut borrow_infos = Vec::new();
                        $system::Data::borrow_infos(&mut borrow_infos);
                        let mut batch_index = new_batch.len();
                        for batch in batch_info.iter().rev() {
                            let mut conflict = false;
                            for &(type_id, mutation) in &borrow_infos {
                                match mutation {
                                    Mutation::Shared => {
                                        for &(batch_type_id, mutation) in batch.iter() {
                                            #[cfg(feature = "parallel")]
                                            {
                                                if type_id == batch_type_id && mutation == Mutation::Unique
                                                || batch_type_id == TypeId::of::<AllStorages>() {
                                                    conflict = true;
                                                    break;
                                                }
                                            }
                                            #[cfg(not(feature = "parallel"))]
                                            {
                                                if type_id == batch_type_id && mutation == Mutation::Unique
                                                || batch_type_id == TypeId::of::<AllStorages>() {
                                                    conflict = true;
                                                    break;
                                                }
                                            }
                                        };
                                    },
                                    Mutation::Unique => {
                                        for &(batch_type_id, _) in batch.iter() {
                                            #[cfg(feature = "parallel")]
                                            {
                                                if type_id == batch_type_id
                                                    || type_id == TypeId::of::<AllStorages>()
                                                {
                                                    conflict = true;
                                                    break;
                                                }
                                            }
                                            #[cfg(not(feature = "parallel"))]
                                            {
                                                if type_id == batch_type_id
                                                    || type_id == TypeId::of::<AllStorages>()
                                                {
                                                    conflict = true;
                                                    break;
                                                }
                                            }
                                        };
                                    },
                                }
                            }

                            if conflict {
                                break;
                            } else {
                                batch_index -= 1;
                            }
                        }

                        // conflict at the very last new batch
                        if batch_index == batch_info.len() {
                            new_batch.push(vec![system_index]);
                            batch_info.push(borrow_infos);
                        } else {
                            new_batch[batch_index].push(system_index);
                            batch_info[batch_index].append(&mut borrow_infos);
                        }
                    } else {
                        let last = new_batch.last_mut().unwrap();
                        if last.is_empty() {
                            last.push(system_index);
                            new_batch.push(Vec::new());
                            batch_info.last_mut().unwrap().push((TypeId::of::<AllStorages>(), Mutation::Unique));
                            batch_info.push(Vec::new());
                        } else {
                            new_batch.push(vec![system_index]);
                            new_batch.push(Vec::new());
                            batch_info.push(vec![(TypeId::of::<AllStorages>(), Mutation::Unique)]);
                            batch_info.push(Vec::new());
                        }
                    }
                )+

                if new_batch.last().unwrap().is_empty() {
                    new_batch.pop();
                }

                scheduler.batch.extend(new_batch.into_iter().map(|batch| batch.into_boxed_slice()));

                if scheduler.workloads.is_empty() {
                    scheduler.default = batch_start..(scheduler.batch.len());
                }

                scheduler.workloads.insert(name.into(), batch_start..(scheduler.batch.len()));
                Ok(())
            }
        }
    }
}

macro_rules! scheduler {
    ($(($system: ident, $index: tt))*;($system1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_scheduler![$(($system, $index))*];
        scheduler![$(($system, $index))* ($system1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($system: ident, $index: tt))*;) => {
        impl_scheduler![$(($system, $index))*];
    }
}

scheduler![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];

#[test]
fn single_immutable() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    System1
        .try_into_workload("System1", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
}
#[test]
fn single_mutable() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    System1
        .try_into_workload("System1", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("System1"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
}
#[test]
fn multiple_immutable() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    (System1, System2)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0, 1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);
}
#[test]
fn multiple_mutable() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    (System1, System2)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
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
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    (System1, System2)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut scheduler = Scheduler::default();
    (System2, System1)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
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
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (AllStorages,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    System2
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 1);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..1)));
    assert_eq!(scheduler.default, 0..1);

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    (System2, System2)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut all_storages = AllStorages::default();
    all_storages.register::<usize>();
    let mut scheduler = Scheduler::default();
    (System1, System2)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut scheduler = Scheduler::default();
    (System2, System1)
        .try_into_workload("Systems", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Systems"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);
}

#[test]
fn non_send() {
    struct NonSend(*const ());
    unsafe impl Sync for NonSend {}

    struct Sys1;
    impl<'a> System<'a> for Sys1 {
        type Data = (&'a NonSend,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }
    struct Sys2;
    impl<'a> System<'a> for Sys2 {
        type Data = (&'a mut NonSend,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    struct Sys3;
    impl<'a> System<'a> for Sys3 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    struct Sys4;
    impl<'a> System<'a> for Sys4 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData<'_>>::View) {}
    }

    let mut all_storages = AllStorages::default();
    all_storages.register_non_send::<NonSend>();
    let mut scheduler = Scheduler::default();
    (Sys1, Sys1)
        .try_into_workload("Test", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 1);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[0]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut scheduler = Scheduler::default();
    (Sys1, Sys2)
        .try_into_workload("Test", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut scheduler = Scheduler::default();
    (Sys2, Sys1)
        .try_into_workload("Test", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut scheduler = Scheduler::default();
    (Sys1, Sys2)
        .try_into_workload("Test", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);

    let mut scheduler = Scheduler::default();
    (Sys1, Sys2)
        .try_into_workload("Test", &mut scheduler, &all_storages)
        .unwrap();
    assert_eq!(scheduler.systems.len(), 2);
    assert_eq!(scheduler.batch.len(), 2);
    assert_eq!(&*scheduler.batch[0], &[0]);
    assert_eq!(&*scheduler.batch[1], &[1]);
    assert_eq!(scheduler.workloads.len(), 1);
    assert_eq!(scheduler.workloads.get("Test"), Some(&(0..2)));
    assert_eq!(scheduler.default, 0..2);
}
