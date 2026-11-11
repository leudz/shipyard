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

/// Asserts that an `after_all` label no other system has doesn't opt a system out of implicit ordering,
/// so the writer stays between the two readers.
#[test]
fn after_all_absent() {
    fn read(_: View<'_, U32>) {}
    fn write(_: ViewMut<'_, U32>) {}

    let (workload, _) = (read, write.after_all("not present"), read)
        .into_workload()
        .rename("")
        .build()
        .unwrap();

    let batches = &workload.batches;
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![System(0)]),
            (None, vec![System(1)]),
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

/// Returns a workload with two independent systems split by a barrier: `[a] -> [b]`.
fn barrier_donor() -> Workload {
    fn a() {}
    fn b() {}

    Workload::new("donor")
        .with_system(a)
        .with_barrier()
        .with_system(b)
}

/// Asserts that `merge` keeps the barrier of the workload it takes systems from.
#[test]
fn merge_barrier() {
    let workload = Workload::new("").merge(barrier_donor());

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that `append` keeps the destination's own barrier:
/// `p` stays alone, everything after the barrier waits for it.
#[test]
fn append_destination_barrier() {
    fn p() {}
    fn q() {}
    fn a() {}
    fn b() {}

    let mut donor = Workload::new("donor").with_system(a).with_system(b);
    let workload = Workload::new("")
        .with_system(p)
        .with_barrier()
        .with_system(q)
        .append(&mut donor);

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["p"]), steps(&["q", "a", "b"])],
            sequential: steps(&["p", "q", "a", "b"]),
        }
    );
}

/// Asserts that appending an empty workload leaves the destination's schedule unchanged.
#[test]
fn append_empty_donor() {
    fn p() {}
    fn q() {}

    let workload = Workload::new("")
        .with_system(p)
        .with_barrier()
        .with_system(q)
        .append(&mut Workload::new("donor"));

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["p"]), steps(&["q"])],
            sequential: steps(&["p", "q"]),
        }
    );
}

/// Asserts that an empty first child of a sequential tuple doesn't break the order of the rest.
#[test]
fn sequential_empty_first() {
    fn a() {}
    fn b() {}

    let workload = (Workload::new("empty"), a, b).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that an empty last child of a sequential tuple doesn't break the order of the rest.
#[test]
fn sequential_empty_last() {
    fn a() {}
    fn b() {}

    let workload = (a, b, Workload::new("empty")).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that a sequential tuple of empty children builds into an empty schedule.
#[test]
fn sequential_empty_all() {
    let workload = (Workload::new("empty1"), Workload::new("empty2")).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![],
            sequential: vec![],
        }
    );
}

/// Asserts that two children of a sequential tuple with distinct names run one after the other,
/// the first child's systems staying in parallel.
#[test]
fn sequential_distinct_names() {
    fn a() {}
    fn b() {}
    fn c() {}

    let first = Workload::new("first").with_system(a).with_system(b);
    let second = Workload::new("second").with_system(c);
    let workload = (first, second).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
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

/// Asserts that `append` keeps the barrier of the workload it takes systems from.
#[test]
fn append_barrier() {
    let workload = Workload::new("").append(&mut barrier_donor());

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that a barrier `append` takes from its donor moves past the destination's systems:
/// `p` then `[a] | [b]` gives `[p, a] -> [b]`.
#[test]
fn append_barrier_after_prefix() {
    fn p() {}

    let workload = Workload::new("")
        .with_system(p)
        .append(&mut barrier_donor());

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["p", "a"]), steps(&["b"])],
            sequential: steps(&["p", "a", "b"]),
        }
    );
}

/// Asserts that `append` keeps a barrier the donor ends with,
/// so a system added afterwards waits for the donor's systems.
#[test]
fn append_trailing_barrier() {
    fn a() {}
    fn b() {}
    fn c() {}

    let mut donor = Workload::new("donor")
        .with_system(a)
        .with_system(b)
        .with_barrier();
    let workload = Workload::new("").append(&mut donor).with_system(c);

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that a workload emptied by `append` builds as an empty workload.
#[test]
fn append_drained_donor() {
    let mut donor = barrier_donor();
    let _ = Workload::new("").append(&mut donor);

    assert_eq!(
        Schedule::built(donor),
        Schedule {
            parallel: vec![],
            sequential: vec![],
        }
    );
}

/// Asserts that a workload emptied by `append` can be reused with fewer systems
/// than its old barrier position.
#[test]
fn append_reused_donor() {
    fn a() {}
    fn b() {}
    fn c() {}
    fn x() {}

    let mut donor = Workload::new("donor")
        .with_system(a)
        .with_system(b)
        .with_barrier()
        .with_system(c);
    let _ = Workload::new("").append(&mut donor);

    assert_eq!(
        Schedule::built(donor.with_system(x)),
        Schedule {
            parallel: vec![steps(&["x"])],
            sequential: steps(&["x"]),
        }
    );
}

/// Asserts that a workload emptied by `append` doesn't apply its old barrier
/// to the systems it's reused with.
#[test]
fn append_reused_donor_stale_barrier() {
    fn x() {}
    fn y() {}

    let mut donor = barrier_donor();
    let _ = Workload::new("").append(&mut donor);

    let workload = donor.with_system(x).with_system(y);

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["x", "y"])],
            sequential: steps(&["x", "y"]),
        }
    );
}

/// Asserts that `merge` moves each of the donor's barriers once, past the destination's systems.
#[test]
fn merge_barrier_once() {
    fn p() {}

    let workload = Workload::new("").with_system(p).merge(barrier_donor());

    assert_eq!(workload.barriers, [2]);
}

/// Asserts that `into_sequential_workload` on a function returning a workload keeps that workload's
/// independent systems in one batch, like `into_workload` does.
#[test]
fn sequential_factory() {
    fn a() {}
    fn b() {}
    fn factory() -> Workload {
        (a, b).into_workload()
    }

    let workload = factory.into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that `into_sequential_workload` on a function returning a workload keeps the ordering
/// inside that workload: `b` follows `a`, `c` stays next to `a`.
#[test]
fn sequential_factory_constraints() {
    fn a() {}
    fn b() {}
    fn c() {}
    fn factory() -> Workload {
        (a, b.after_all(a), c).into_workload()
    }

    let workload = factory.into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "c"]), steps(&["b"])],
            sequential: steps(&["a", "c", "b"]),
        }
    );
}

/// Asserts that `into_sequential_workload` on a function returning a workload names the workload
/// after the function, like `into_workload` does.
#[test]
fn sequential_factory_name() {
    fn a() {}
    fn factory() -> Workload {
        a.into_workload()
    }

    let world = World::new_for_test();
    factory
        .into_sequential_workload()
        .add_to_world(&world)
        .unwrap();

    assert!(world.contains_workload(factory));
}

/// Asserts that `into_sequential_workload` calls a function returning a workload once.
#[test]
fn sequential_factory_once() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    fn a() {}
    fn factory() -> Workload {
        CALLS.fetch_add(1, Ordering::Relaxed);

        a.into_workload()
    }

    let _ = factory.into_sequential_workload();

    assert_eq!(CALLS.load(Ordering::Relaxed), 1);
}

/// Asserts that a function returning a workload, as a child of a sequential tuple,
/// keeps its systems in parallel and runs them before the next child.
#[test]
fn sequential_factory_child() {
    fn a() {}
    fn b() {}
    fn c() {}
    fn factory() -> Workload {
        (a, b).into_workload()
    }

    let workload = (factory, c).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that `into_sequential_workload` accepts a function returning a workload that lists
/// a system twice, like `into_workload` does: the workload's own systems aren't sequenced.
#[test]
fn sequential_factory_repeated() {
    fn a() {}
    fn factory() -> Workload {
        Workload::new("").with_system(a).with_system(a)
    }

    let workload = factory.into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "a"])],
            sequential: steps(&["a", "a"]),
        }
    );
}

/// Asserts that an empty child between two systems of a sequential tuple
/// doesn't break their order.
#[test]
fn sequential_empty_middle() {
    fn a() {}
    fn b() {}

    let workload = (a, Workload::new("empty"), b).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that several empty children between two systems of a sequential tuple
/// don't break their order.
#[test]
fn sequential_empty_several() {
    fn a() {}
    fn b() {}

    let workload =
        (a, Workload::new("empty1"), Workload::new("empty2"), b).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["b"])],
            sequential: steps(&["a", "b"]),
        }
    );
}

/// Asserts that two children of a sequential tuple sharing a name run one after the other,
/// the first child's systems staying in parallel.
#[test]
fn sequential_same_names() {
    fn a() {}
    fn b() {}
    fn c() {}

    let first = Workload::new("same").with_system(a).with_system(b);
    let second = Workload::new("same").with_system(c);
    let workload = (first, second).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that a child name reused at two nesting levels of sequential tuples
/// doesn't link the levels: `a`, then the inner `p`, then the inner `c`.
#[test]
fn sequential_same_names_nested() {
    fn a() {}
    fn p() {}
    fn c() {}

    let inner = (p, Workload::new("same").with_system(c)).into_sequential_workload();
    let workload = (Workload::new("same").with_system(a), inner).into_sequential_workload();

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a"]), steps(&["p"]), steps(&["c"])],
            sequential: steps(&["a", "p", "c"]),
        }
    );
}

/// Asserts that a sequential tuple still rejects a system listed in two of its children,
/// even when the children have different names.
#[test]
#[should_panic(expected = "appears twice in this workload")]
fn sequential_duplicate() {
    fn a() {}

    let first = Workload::new("first").with_system(a);
    let second = Workload::new("second").with_system(a);

    let _ = (first, second).into_sequential_workload();
}

/// Always returns a custom error.
fn fails() -> Result<(), error::SystemError> {
    Err(error::MissingComponent {
        id: crate::EntityId::dead(),
        name: "fails",
    }
    .into())
}

/// Asserts that a workload made from a tuple containing a fallible system stops with its error.
#[test]
fn tuple_fallible() {
    fn other() {}

    let (workload, _) = (other, fails).into_workload().build().unwrap();
    let world = World::new_for_test();

    let result = workload.run_with_world(&world);

    assert!(result.err().unwrap().custom_error().is_some());
}

/// Asserts that a sequential workload stops at a failing system, the system after it doesn't run.
#[test]
fn sequential_failure() {
    let ran = alloc::sync::Arc::new(core::sync::atomic::AtomicBool::new(false));
    let system_ran = ran.clone();
    let after = move || system_ran.store(true, core::sync::atomic::Ordering::SeqCst);
    let (workload, _) = (fails, after).into_sequential_workload().build().unwrap();
    let world = World::new_for_test();

    let result = workload.run_with_world(&world);

    assert!(result.err().unwrap().custom_error().is_some());
    assert!(!ran.load(core::sync::atomic::Ordering::SeqCst));
}
