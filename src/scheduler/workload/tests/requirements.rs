use super::*;

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
