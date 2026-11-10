use super::pending::read_a;
use super::*;
use crate::error;
use crate::iter::IntoIter;
use crate::scheduler::info::WorkloadInfo;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

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

/// Returns a system adding one to `target`.
///
/// Every call returns the same closure type, only the captured counter differs.
fn counting(target: &Arc<AtomicUsize>) -> impl Fn() + Send + Sync + 'static {
    let target = target.clone();

    move || {
        target.fetch_add(1, Ordering::SeqCst);
    }
}

/// Borrows `Usize` with insertion tracking, `Usize` isn't tracked by default.
fn tracked(_: View<'_, Usize, crate::track::Insertion>) {}

/// Registers the workload `name` with `tracked` while `Usize` is borrowed,
/// so enabling its tracking fails.
fn add_while_borrowed(world: &World, name: &'static str) -> Result<(), error::AddWorkload> {
    let _usize = world.borrow::<View<'_, Usize>>().unwrap();

    Workload::new(name).with_system(tracked).add_to_world(world)
}

/// Asserts that registering a workload fails when the storage whose tracking it enables is borrowed.
#[test]
fn failed_tracking_error() {
    let world = World::new_for_test();

    let result = add_while_borrowed(&world, "tracked");

    assert!(matches!(
        result,
        Err(error::AddWorkload::TrackingStorageBorrow { .. })
    ));
}

/// Asserts that a failed registration keeps the existing default workload, which still runs.
#[test]
fn failed_tracking_keeps_default() {
    let world = World::new_for_test();
    let counter = Arc::new(AtomicUsize::new(0));
    Workload::new("valid")
        .with_system(counting(&counter))
        .add_to_world(&world)
        .unwrap();

    add_while_borrowed(&world, "tracked").unwrap_err();
    world.run_default_workload().unwrap();

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

/// Returns whether the workload `name` is in the scheduler's workloads and in its workloads' info.
fn published(world: &World, name: &'static str) -> (bool, bool) {
    let scheduler = world.scheduler.borrow().unwrap();
    let label: Box<dyn Label> = Box::new(name);

    (
        scheduler.workloads.contains_key(&label),
        scheduler.workloads_info.contains_key(&label),
    )
}

/// Asserts that a workload whose tracking couldn't be enabled is neither in the workloads nor in their info.
#[test]
fn failed_tracking_absent() {
    let world = World::new_for_test();

    add_while_borrowed(&world, "tracked").unwrap_err();

    assert_eq!(published(&world, "tracked"), (false, false));
}

/// Asserts that a workload whose tracking couldn't be enabled can be registered again
/// once the borrow is released, and is then in both the workloads and their info.
#[test]
fn failed_tracking_retry() {
    let world = World::new_for_test();
    add_while_borrowed(&world, "tracked").unwrap_err();

    let retry = Workload::new("tracked")
        .with_system(tracked)
        .add_to_world(&world);

    assert!(retry.is_ok(), "{retry:?}");
    assert_eq!(published(&world, "tracked"), (true, true));
}

/// Asserts that a workload whose tracking couldn't be enabled doesn't become the default workload,
/// the first workload registered successfully afterward does.
#[test]
fn failed_tracking_default() {
    let world = World::new_for_test();
    let counter = Arc::new(AtomicUsize::new(0));
    add_while_borrowed(&world, "tracked").unwrap_err();

    Workload::new("valid")
        .with_system(counting(&counter))
        .add_to_world(&world)
        .unwrap();
    world.run_default_workload().unwrap();

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

/// Ordered after `cycle_b`, which is ordered after it.
fn cycle_a() {}

/// Ordered after `cycle_a`, which is ordered after it.
fn cycle_b() {}

/// Asserts that a workload rejected for a cycle doesn't leave its `counting` system behind:
/// a later valid workload with another `counting` system runs its own.
#[test]
fn failed_cycle_capture() {
    let world = World::new_for_test();
    let rejected = Arc::new(AtomicUsize::new(0));
    let accepted = Arc::new(AtomicUsize::new(0));
    let result = Workload::new("rejected")
        .with_system(counting(&rejected))
        .with_system(cycle_a.after_all(cycle_b))
        .with_system(cycle_b.after_all(cycle_a))
        .add_to_world(&world);
    assert!(matches!(
        result,
        Err(error::AddWorkload::ImpossibleRequirements(_))
    ));

    Workload::new("accepted")
        .with_system(counting(&accepted))
        .add_to_world(&world)
        .unwrap();
    world.run_workload("accepted").unwrap();

    assert_eq!(
        (
            rejected.load(Ordering::SeqCst),
            accepted.load(Ordering::SeqCst)
        ),
        (0, 1)
    );
}

/// Asserts that a workload rejected for a missing requirement doesn't leave its `counting` system behind:
/// a later valid workload with another `counting` system runs its own.
#[test]
fn failed_requirement_capture() {
    let world = World::new_for_test();
    let rejected = Arc::new(AtomicUsize::new(0));
    let accepted = Arc::new(AtomicUsize::new(0));
    let result = Workload::new("rejected")
        .with_system(counting(&rejected).require_in_workload("missing"))
        .add_to_world(&world);
    assert!(matches!(
        result,
        Err(error::AddWorkload::MissingInWorkload(..))
    ));

    Workload::new("accepted")
        .with_system(counting(&accepted))
        .add_to_world(&world)
        .unwrap();
    world.run_workload("accepted").unwrap();

    assert_eq!(
        (
            rejected.load(Ordering::SeqCst),
            accepted.load(Ordering::SeqCst)
        ),
        (0, 1)
    );
}

/// Asserts that a workload whose tracking couldn't be enabled doesn't leave its `counting` system behind:
/// registering it again once the borrow is released, with another `counting` system, runs the new one.
#[test]
fn failed_tracking_capture() {
    let world = World::new_for_test();
    let rejected = Arc::new(AtomicUsize::new(0));
    let accepted = Arc::new(AtomicUsize::new(0));
    {
        let _usize = world.borrow::<View<'_, Usize>>().unwrap();
        Workload::new("tracked")
            .with_system(counting(&rejected))
            .with_system(tracked)
            .add_to_world(&world)
            .unwrap_err();
    }

    Workload::new("tracked")
        .with_system(counting(&accepted))
        .with_system(tracked)
        .add_to_world(&world)
        .unwrap();
    world.run_workload("tracked").unwrap();

    assert_eq!(
        (
            rejected.load(Ordering::SeqCst),
            accepted.load(Ordering::SeqCst)
        ),
        (0, 1)
    );
}

/// Returns the lengths of the scheduler's three callable vectors and the sorted indices its lookup table points to.
fn registry(world: &World) -> ([usize; 3], Vec<usize>) {
    let scheduler = world.scheduler.borrow().unwrap();
    let mut targets: Vec<usize> = scheduler.lookup_table.values().copied().collect();
    targets.sort_unstable();

    (
        [
            scheduler.systems.len(),
            scheduler.system_names.len(),
            scheduler.system_generators.len(),
        ],
        targets,
    )
}

/// Asserts that a workload whose tracking couldn't be enabled removes the callable it registered
/// and keeps the one registered by an earlier workload.
#[test]
fn failed_tracking_registry() {
    let world = World::new_for_test();
    Workload::new("prior")
        .with_system(read_a)
        .add_to_world(&world)
        .unwrap();

    add_while_borrowed(&world, "tracked").unwrap_err();

    assert_eq!(registry(&world), ([1, 1, 1], vec![0]));
}
