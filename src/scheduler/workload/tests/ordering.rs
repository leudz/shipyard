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
    assert_eq!(
        batches.sequential,
        &[System(3), System(2), System(0), System(1)]
    );
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(3)]),
            (None, vec![System(2)]),
            (None, vec![System(0), System(1)])
        ]
    );

    let (workload, _) = Workload::new("")
        .with_system(a)
        .with_system(b.after_all(a))
        .with_system(c.after_all(b))
        .build()
        .unwrap();

    let batches = &workload.batches;
    assert_eq!(batches.sequential, &[System(0), System(1), System(2)]);
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(0)]),
            (None, vec![System(1)]),
            (None, vec![System(2)])
        ]
    );

    let (workload, _) = Workload::new("")
        .with_system(b.after_all(a))
        .with_system(a)
        .with_system(c.after_all(b))
        .build()
        .unwrap();

    let batches = &workload.batches;
    assert_eq!(batches.sequential, &[System(1), System(0), System(2)]);
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(1)]),
            (None, vec![System(0)]),
            (None, vec![System(2)])
        ]
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
    assert_eq!(batches.sequential, &[System(0), System(1)]);
    assert_eq!(
        batches.parallel,
        &[(None, vec![System(0)]), (None, vec![System(1)])]
    );

    let (workload, _) = Workload::new("")
        .with_system(b.after_all(a))
        .with_system(a.before_all(b))
        .build()
        .unwrap();

    let batches = &workload.batches;
    assert_eq!(batches.sequential, &[System(1), System(0)]);
    assert_eq!(
        batches.parallel,
        &[(None, vec![System(1)]), (None, vec![System(0)])]
    );
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
    assert_eq!(batches.sequential, &[System(0)]);
    assert_eq!(batches.parallel, &[(None, vec![System(0)])]);
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
    assert!(
        batches.sequential == [System(0), System(1)]
            || batches.sequential == [System(1), System(0)]
    );
    assert_eq!(batches.parallel, &[(None, vec![System(0), System(1)])]);
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
    assert_eq!(batches.sequential, &[System(2), System(1), System(0)]);
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(2)]),
            (None, vec![System(1)]),
            (None, vec![System(0)])
        ]
    );

    let (workload, _) = Workload::new("")
        .with_system(c.tag("c").after_all("b"))
        .with_system(b.tag("b").after_all("a"))
        .with_system(a.tag("a"))
        .build()
        .unwrap();

    let batches = &workload.batches;
    assert_eq!(batches.sequential, &[System(2), System(1), System(0)]);
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(2)]),
            (None, vec![System(1)]),
            (None, vec![System(0)])
        ]
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
            parallel: vec![
                (None, vec![System(0)]),
                (Some(System(1)), vec![]),
                (None, vec![System(2)])
            ],
            parallel_run_if: Vec::new(),
            parallel_occurrences: Vec::new(),
            sequential: vec![System(0), System(1), System(2)],
            sequential_run_if: Vec::new(),
            sequential_occurrences: Vec::new(),
            workload_run_if: None,
            systems_run_if: Vec::new(),
            regroups: Vec::new(),
            grouped_writes: Vec::new(),
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
    assert_eq!(batches.sequential, &[System(0), System(1), System(2)]);
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(0)]),
            (None, vec![System(1)]),
            (None, vec![System(2)])
        ]
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
        &[
            (None, vec![System(0)]),
            (Some(System(1)), Vec::new()),
            (None, vec![System(0)])
        ]
    );
    assert_eq!(batches.sequential, &[System(0), System(1), System(0)]);
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
            (None, vec![System(0)]),
            (None, vec![System(2)]),
            (None, vec![System(1)]),
            (None, vec![System(3)])
        ]
    );
    assert_eq!(
        batches.sequential,
        &[System(0), System(2), System(1), System(3)]
    );
}

/// Asserts that two independent systems added with `extend` build the same schedule
/// as when they're added with `with_system`.
#[test]
fn extend_two() {
    fn a() {}
    fn b() {}

    let expected = Schedule {
        parallel: vec![steps(&["a", "b"])],
        sequential: steps(&["a", "b"]),
    };

    let workload = Workload::new("").with_system(a).with_system(b);
    assert_eq!(Schedule::built(workload), expected);

    let mut workload = Workload::new("");
    workload.extend([
        a.into_workload_system().unwrap(),
        b.into_workload_system().unwrap(),
    ]);
    assert_eq!(Schedule::built(workload), expected);
}

/// Asserts that three independent systems added with `extend` schedule like `with_system`:
/// one batch, in declaration order.
#[test]
fn extend_three() {
    fn a() {}
    fn b() {}
    fn c() {}

    let expected = Schedule {
        parallel: vec![steps(&["a", "b", "c"])],
        sequential: steps(&["a", "b", "c"]),
    };

    let workload = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_system(c);
    assert_eq!(Schedule::built(workload), expected);

    let mut workload = Workload::new("");
    workload.extend([
        a.into_workload_system().unwrap(),
        b.into_workload_system().unwrap(),
        c.into_workload_system().unwrap(),
    ]);
    assert_eq!(Schedule::built(workload), expected);
}

/// Asserts that three writers of the same storage added with `extend` each get their own
/// batch, in declaration order.
#[test]
fn extend_writers() {
    fn w1(_: ViewMut<'_, Usize>) {}
    fn w2(_: ViewMut<'_, Usize>) {}
    fn w3(_: ViewMut<'_, Usize>) {}

    let expected = Schedule {
        parallel: vec![steps(&["w1"]), steps(&["w2"]), steps(&["w3"])],
        sequential: steps(&["w1", "w2", "w3"]),
    };

    let workload = Workload::new("")
        .with_system(w1)
        .with_system(w2)
        .with_system(w3);
    assert_eq!(Schedule::built(workload), expected);

    let mut workload = Workload::new("");
    workload.extend([
        w1.into_workload_system().unwrap(),
        w2.into_workload_system().unwrap(),
        w3.into_workload_system().unwrap(),
    ]);
    assert_eq!(Schedule::built(workload), expected);
}

/// Asserts that two systems added with `extend` after an existing system keep their order.
#[test]
fn extend_after_prefix() {
    fn p() {}
    fn a() {}
    fn b() {}

    let expected = Schedule {
        parallel: vec![steps(&["p", "a", "b"])],
        sequential: steps(&["p", "a", "b"]),
    };

    let workload = Workload::new("")
        .with_system(p)
        .with_system(a)
        .with_system(b);
    assert_eq!(Schedule::built(workload), expected);

    let mut workload = Workload::new("").with_system(p);
    workload.extend([
        a.into_workload_system().unwrap(),
        b.into_workload_system().unwrap(),
    ]);
    assert_eq!(Schedule::built(workload), expected);
}

/// Asserts that a barrier after systems added with `extend` makes the next system wait
/// for all of them.
#[test]
fn extend_barrier() {
    fn a() {}
    fn b() {}
    fn c() {}

    let expected = Schedule {
        parallel: vec![steps(&["a", "b"]), steps(&["c"])],
        sequential: steps(&["a", "b", "c"]),
    };

    let workload = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_barrier()
        .with_system(c);
    assert_eq!(Schedule::built(workload), expected);

    let mut workload = Workload::new("");
    workload.extend([
        a.into_workload_system().unwrap(),
        b.into_workload_system().unwrap(),
    ]);
    let workload = workload.with_barrier().with_system(c);
    assert_eq!(Schedule::built(workload), expected);
}

/// Asserts that a system added three times with `extend` is scheduled and runs three times.
#[test]
fn extend_repeated() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static RUNS: AtomicUsize = AtomicUsize::new(0);

    fn a() {
        RUNS.fetch_add(1, Ordering::Relaxed);
    }

    let expected = Schedule {
        parallel: vec![steps(&["a", "a", "a"])],
        sequential: steps(&["a", "a", "a"]),
    };

    let workload = Workload::new("")
        .with_system(a)
        .with_system(a)
        .with_system(a);
    assert_eq!(Schedule::built(workload), expected);

    let mut workload = Workload::new("Repeated");
    workload.extend([
        a.into_workload_system().unwrap(),
        a.into_workload_system().unwrap(),
        a.into_workload_system().unwrap(),
    ]);
    let world = World::new_for_test();
    workload.add_to_world(&world).unwrap();
    assert_eq!(Schedule::registered(&world, "Repeated"), expected);

    world.run_workload("Repeated").unwrap();
    assert_eq!(RUNS.load(Ordering::Relaxed), 3);
}

/// Asserts that `d.after_all(a)` keeps `d` in a later batch than `a`
/// when an unrelated system was added with `extend` in between.
///
/// `a` waits on `b`, so it lands in the second batch. The extended `c` runs in the first one.
#[test]
fn extend_explicit() {
    fn a() {}
    fn b() {}
    fn c() {}
    fn d() {}

    let mut workload = Workload::new("")
        .with_system(a.after_all(b))
        .with_system(b)
        .with_system(c)
        .with_system(d.after_all(a));
    let schedule = Schedule::built(workload);
    assert!(schedule.batch("b") < schedule.batch("a"), "{schedule:?}");
    assert!(schedule.batch("a") < schedule.batch("d"), "{schedule:?}");

    workload = Workload::new("").with_system(a.after_all(b)).with_system(b);
    workload.extend([c.into_workload_system().unwrap()]);
    let workload = workload.with_system(d.after_all(a));
    let schedule = Schedule::built(workload);
    assert!(schedule.batch("b") < schedule.batch("a"), "{schedule:?}");
    assert!(schedule.batch("a") < schedule.batch("d"), "{schedule:?}");
}

/// Asserts that conditions of systems added with `extend` stay with their own system,
/// even when the sequential order differs from the declaration order.
///
/// `a` is declared first but follows `b`, `b`'s condition refuses to run it.
#[test]
fn extend_conditions() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static A_RUNS: AtomicUsize = AtomicUsize::new(0);
    static B_RUNS: AtomicUsize = AtomicUsize::new(0);
    static C_RUNS: AtomicUsize = AtomicUsize::new(0);

    fn a() {
        A_RUNS.fetch_add(1, Ordering::Relaxed);
    }
    fn b() {
        B_RUNS.fetch_add(1, Ordering::Relaxed);
    }
    fn c() {
        C_RUNS.fetch_add(1, Ordering::Relaxed);
    }

    let mut workload = Workload::new("Conditions");
    workload.extend([
        a.after_all(b).run_if(|| true),
        b.run_if(|| false),
        c.into_workload_system().unwrap(),
    ]);
    let world = World::new_for_test();
    workload.add_to_world(&world).unwrap();
    assert_eq!(
        Schedule::registered(&world, "Conditions"),
        Schedule {
            parallel: vec![steps(&["b", "c"]), steps(&["a"])],
            sequential: steps(&["b", "c", "a"]),
        }
    );

    world.run_workload("Conditions").unwrap();

    let runs = [&A_RUNS, &B_RUNS, &C_RUNS].map(|runs| runs.load(Ordering::Relaxed));
    assert_eq!(runs, [1, 0, 1]);
}
