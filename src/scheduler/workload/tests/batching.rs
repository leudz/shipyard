use super::*;

#[test]
fn single_immutable() {
    fn system1(_: View<'_, Usize>) {}

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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
    use crate::borrow::NonSend;

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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

    let world = World::new_for_test();

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
    let world = World::new_for_test();

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

    let world = World::new_for_test();

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
fn with_system_return_type() {
    Workload::new("").with_system(|| 0usize).build().unwrap();
}

/// Asserts that a system borrowing `T` exclusively and one borrowing `NonSend<T>` exclusively
/// share a batch, since they are different storages.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_distinct_storage() {
    use crate::borrow::NonSend;

    fn sys1(_: ViewMut<'_, Usize>) {}
    fn sys2(_: NonSend<ViewMut<'_, Usize>>) {}

    let world = World::new_for_test();

    Workload::new("Test")
        .with_system(sys1)
        .with_system(sys2)
        .add_to_world(&world)
        .unwrap();

    let scheduler = world.scheduler.borrow_mut().unwrap();
    let label: Box<dyn Label> = Box::new("Test");
    assert_eq!(
        scheduler.workloads.get(&label),
        Some(&Batches {
            parallel: vec![(Some(1), vec![0])],
            parallel_run_if: Vec::new(),
            sequential: vec![0, 1],
            sequential_run_if: Vec::new(),
            workload_run_if: None,
            systems_run_if: Vec::new(),
        })
    );
}
