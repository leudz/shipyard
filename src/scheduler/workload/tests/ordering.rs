use super::*;

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
    fn type_id_of<T: 'static>(_: &T) -> TypeId {
        TypeId::of::<T>()
    }

    fn w() -> Workload {
        (|| {}).into_workload()
    }
    let world = World::new_for_test();
    world.add_workload(w);
    assert!(world.contains_workload(WorkloadLabel {
        type_id: type_id_of(&w),
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
