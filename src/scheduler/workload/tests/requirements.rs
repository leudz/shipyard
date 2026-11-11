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

/// Asserts that without requirements, `c` lands after both `a` and `b`.
///
/// `c` writes both storages, so it conflicts with `a` (X) and `b` (Y).
#[test]
fn implicit_unconstrained() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let workload = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_system(c);

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that `c` requiring its nearest conflicting predecessor `b` before itself is accepted,
/// and that the requirement adds no edge.
#[test]
fn implicit_nearest_before() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let workload = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_system(c.require_before(b));

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that `b` requiring its nearest conflicting successor `c` after itself is accepted,
/// and that the requirement adds no edge.
#[test]
fn implicit_nearest_after() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let workload = Workload::new("")
        .with_system(a)
        .with_system(b.require_after(c))
        .with_system(c);

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that `c` requiring an unrelated earlier system `d` before itself is rejected.
///
/// `d` borrows a storage `c` doesn't touch, so only declaration order puts it first.
/// That's no guarantee `d` runs before `c`.
#[test]
fn implicit_unrelated_before() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn d(_: ViewMut<'_, U16>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let result = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_system(d)
        .with_system(c.require_before(d))
        .build();

    assert_eq!(
        result.err(),
        Some(error::AddWorkload::MissingBefore(
            c.as_label(),
            vec![d.as_label()]
        ))
    );
}

/// Asserts that an unrelated earlier system `d` requiring `c` after itself is rejected.
///
/// `d` borrows a storage `c` doesn't touch, so only declaration order puts `c` later.
/// That's no guarantee `c` runs after `d`.
#[test]
fn implicit_unrelated_after() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn d(_: ViewMut<'_, U16>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let result = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_system(d.require_after(c))
        .with_system(c)
        .build();

    assert_eq!(
        result.err(),
        Some(error::AddWorkload::MissingAfter(
            d.as_label(),
            vec![c.as_label()]
        ))
    );
}

/// Asserts that `c` requiring `a` before itself is accepted
/// when implicit ordering already puts `a` first, and that the requirement adds no edge.
///
/// `a` isn't `c`'s nearest conflicting predecessor, `b` is.
#[test]
fn implicit_require_before() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let workload = Workload::new("")
        .with_system(a)
        .with_system(b)
        .with_system(c.require_before(a));

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}

/// Asserts that `a` requiring `c` after itself is accepted
/// when implicit ordering already puts `c` later, and that the requirement adds no edge.
///
/// `a` isn't `c`'s nearest conflicting predecessor, `b` is.
#[test]
fn implicit_require_after() {
    fn a(_: ViewMut<'_, Usize>) {}
    fn b(_: ViewMut<'_, U32>) {}
    fn c(_: ViewMut<'_, Usize>, _: ViewMut<'_, U32>) {}

    let workload = Workload::new("")
        .with_system(a.require_after(c))
        .with_system(b)
        .with_system(c);

    assert_eq!(
        Schedule::built(workload),
        Schedule {
            parallel: vec![steps(&["a", "b"]), steps(&["c"])],
            sequential: steps(&["a", "b", "c"]),
        }
    );
}
