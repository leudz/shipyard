use super::pending::read_a;
use super::*;
use crate::iter::IntoIter;
use crate::scheduler::info::WorkloadInfo;

/// Returns a world with the AB group and an unevaluated entity in it.
fn ab_world() -> World {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    world.add_entity((A, B));

    world
}

/// Returns `true` if `info` shows a regroup.
fn has_regroups(info: &WorkloadInfo) -> bool {
    info.batches_info
        .iter()
        .any(|batch_info| !batch_info.regroups.is_empty())
}

/// Asserts that a workload added to a world regroups the sets its systems borrow when it runs.
#[test]
fn add_to_world() {
    let world = ab_world();
    Workload::new("")
        .with_system(read_a)
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
}

/// Asserts that `World::add_workload` adds a workload with its regroups.
#[test]
fn add_workload() {
    let world = ab_world();
    world.add_workload(|| Workload::new("").with_system(read_a));

    world.run_default_workload().unwrap();

    assert!(evaluated::<A>(&world));
}

/// Asserts that a workload nested in another one gets its regroups.
#[test]
fn nested() {
    let world = ab_world();
    Workload::new("outer")
        .with_workload(Workload::new("inner").with_system(read_a))
        .add_to_world(&world)
        .unwrap();

    world.run_workload("outer").unwrap();

    assert!(evaluated::<A>(&world));
}

/// Asserts that a workload merged into another one gets its regroups.
#[test]
fn merged() {
    let world = ab_world();
    Workload::new("")
        .merge(Workload::new("other").with_system(read_a))
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();

    assert!(evaluated::<A>(&world));
}

/// Asserts that the same workload gets regroups in a world grouping its storages,
/// and none in a world without groups.
#[test]
fn per_world() {
    let workload = || Workload::new("").with_system(read_a);
    let grouped = ab_world();
    let ungrouped = World::new_for_test();

    workload().add_to_world(&grouped).unwrap();
    workload().add_to_world(&ungrouped).unwrap();

    assert!(has_regroups(&grouped.workloads_info().0[""]));
    assert!(!has_regroups(&ungrouped.workloads_info().0[""]));
}

/// Asserts that a built workload has no regroups, and leaves the worlds it runs on unevaluated
/// while still iterating them correctly.
#[test]
fn standalone() {
    let (workload, info) = Workload::new("").with_system(read_a).build().unwrap();
    let worlds = [ab_world(), ab_world()];

    assert!(!has_regroups(&info));
    assert!(
        workload
            .batches
            .sequential
            .iter()
            .all(|step| matches!(step, System(_)))
    );
    for world in &worlds {
        workload.run_with_world(world).unwrap();

        assert!(!evaluated::<A>(world));
        let count = world.run(|a: View<'_, A>, b: View<'_, B>| (&a, &b).iter().count());
        assert_eq!(count, 1);
    }
}

/// Asserts that a workload added to a world's clone regroups the clone's copied components,
/// and leaves the source alone.
#[cfg(feature = "std")]
#[test]
fn clone() {
    use crate::sparse_set::SparseSet;

    let mut world = ab_world();
    world.register_clone::<SparseSet<A>>();
    world.register_clone::<SparseSet<B>>();

    let clone = world.clone();
    Workload::new("")
        .with_system(read_a)
        .add_to_world(&clone)
        .unwrap();
    clone.run_default_workload().unwrap();

    assert!(evaluated::<A>(&clone));
    assert!(evaluated::<B>(&clone));
    assert!(!evaluated::<A>(&world));
}
