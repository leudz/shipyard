use super::pending::read_a;
use super::*;
use crate::error;
use crate::iter::IntoIter;
use crate::scheduler::IntoWorkloadSystem;
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

/// Always fails with a `MissingComponent`, its workload stops with it.
fn fails() -> Result<(), error::SystemError> {
    Err(error::MissingComponent {
        id: crate::EntityId::dead(),
        name: "fails",
    }
    .into())
}

/// Returns `true` when running the workload `name` fails with a system's custom error.
fn custom_error(world: &World, name: &'static str) -> bool {
    match world.run_workload(name) {
        Ok(()) => false,
        Err(error) => error.custom_error().is_some(),
    }
}

/// Returns the label of the system that stopped a workload and the error it returned.
///
/// ### Panics
///
/// When the workload didn't stop because of a system's error.
fn system_failure(
    result: Result<(), error::RunWorkload>,
) -> (Box<dyn Label>, Box<dyn core::error::Error + Send + Sync>) {
    match result {
        Err(error::RunWorkload::Run((label, error::Run::Custom(returned)))) => {
            (label, returned.into_inner())
        }
        other => panic!("a system should have returned an error: {other:?}"),
    }
}

/// Returns the name of the `MissingComponent` held by `returned`, `None` if it holds another error.
fn missing_name(
    returned: &(dyn core::error::Error + Send + Sync + 'static),
) -> Option<&'static str> {
    returned
        .downcast_ref::<error::MissingComponent>()
        .map(|missing| missing.name)
}

/// Asserts that a function without views returning an error stops its registered workload,
/// the error names `fails` and holds what it returned.
#[test]
fn return_error() {
    let world = World::new_for_test();
    Workload::new("fails")
        .with_system(fails)
        .add_to_world(&world)
        .unwrap();

    let (label, returned) = system_failure(world.run_workload("fails"));

    assert!(*label == *fails.as_label());
    assert_eq!(missing_name(&*returned), Some("fails"));
}

/// Asserts that a function without views returning an error stops its built workload.
#[test]
fn return_error_built() {
    let world = World::new_for_test();
    let (workload, _) = Workload::new("fails").with_system(fails).build().unwrap();

    let (label, returned) = system_failure(workload.run_with_world(&world));

    assert!(*label == *fails.as_label());
    assert_eq!(missing_name(&*returned), Some("fails"));
}

/// Fails with a `MissingComponent` while borrowing `Usize`.
fn fails_with_view(_: View<'_, Usize>) -> Result<(), error::SystemError> {
    Err(error::MissingComponent {
        id: crate::EntityId::dead(),
        name: "fails_with_view",
    }
    .into())
}

/// Asserts that a function with views returning an error stops its registered workload.
#[test]
fn return_error_views() {
    let world = World::new_for_test();
    Workload::new("fails")
        .with_system(fails_with_view)
        .add_to_world(&world)
        .unwrap();

    let (label, returned) = system_failure(world.run_workload("fails"));

    assert!(*label == *fails_with_view.as_label());
    assert_eq!(missing_name(&*returned), Some("fails_with_view"));
}

/// Asserts that a function with views returning an error stops its built workload.
#[test]
fn return_error_views_built() {
    let world = World::new_for_test();
    let (workload, _) = Workload::new("fails")
        .with_system(fails_with_view)
        .build()
        .unwrap();

    let (label, returned) = system_failure(workload.run_with_world(&world));

    assert!(*label == *fails_with_view.as_label());
    assert_eq!(missing_name(&*returned), Some("fails_with_view"));
}

/// Asserts that functions returning `Ok(())`, with and without views, let their workload succeed.
#[test]
fn return_ok() {
    fn succeeds() -> Result<(), error::SystemError> {
        Ok(())
    }

    fn succeeds_with_view(_: View<'_, Usize>) -> Result<(), error::SystemError> {
        Ok(())
    }

    let world = World::new_for_test();
    Workload::new("succeeds")
        .with_system(succeeds)
        .with_system(succeeds_with_view)
        .add_to_world(&world)
        .unwrap();

    assert!(world.run_workload("succeeds").is_ok());
}

/// Asserts that a function returning an alias of `Result<(), SystemError>` stops its workload.
#[test]
fn return_alias() {
    type Fallible = Result<(), error::SystemError>;

    fn aliased() -> Fallible {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "aliased",
        }
        .into())
    }

    let world = World::new_for_test();
    Workload::new("aliased")
        .with_system(aliased)
        .add_to_world(&world)
        .unwrap();

    assert!(custom_error(&world, "aliased"));
}

/// Asserts that a `Result` with another error type is dropped, its workload succeeds.
#[test]
fn return_other_error() {
    fn other_error() -> Result<(), error::MissingComponent> {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "other_error",
        })
    }

    let world = World::new_for_test();
    Workload::new("other")
        .with_system(other_error)
        .add_to_world(&world)
        .unwrap();

    assert!(world.run_workload("other").is_ok());
}

/// Asserts that a `Result` with a `SystemError` but another success type is dropped,
/// its workload succeeds.
#[test]
fn return_non_unit() {
    fn non_unit() -> Result<u32, error::SystemError> {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "non_unit",
        }
        .into())
    }

    let world = World::new_for_test();
    Workload::new("non_unit")
        .with_system(non_unit)
        .add_to_world(&world)
        .unwrap();

    assert!(world.run_workload("non_unit").is_ok());
}

/// Wraps a `Result<(), SystemError>`, it converts into one but isn't one.
struct Wrapper(Result<(), error::SystemError>);

impl From<Wrapper> for Result<(), error::SystemError> {
    fn from(wrapper: Wrapper) -> Self {
        wrapper.0
    }
}

/// Asserts that a type converting into `Result<(), SystemError>` is dropped, its workload succeeds.
#[test]
fn return_wrapper() {
    fn wrapper() -> Wrapper {
        Wrapper(fails())
    }

    let world = World::new_for_test();
    Workload::new("wrapper")
        .with_system(wrapper)
        .add_to_world(&world)
        .unwrap();

    assert!(world.run_workload("wrapper").is_ok());
}

/// Adds one to its counter when dropped.
struct Dropped(Arc<AtomicUsize>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Asserts that a returned value is dropped once per run.
#[test]
fn return_dropped() {
    let drops = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    let system_drops = drops.clone();
    Workload::new("dropped")
        .with_system(move || Dropped(system_drops.clone()))
        .add_to_world(&world)
        .unwrap();

    world.run_workload("dropped").unwrap();
    world.run_workload("dropped").unwrap();

    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

/// Asserts that a fallible system whose views can't be borrowed doesn't run,
/// the workload returns the borrow error.
#[test]
fn return_borrow_failure() {
    let ran = Arc::new(AtomicUsize::new(0));
    let world = World::new_for_test();
    world.add_unique(Usize(0));
    let system_ran = ran.clone();
    Workload::new("borrow")
        .with_system(
            move |_: UniqueView<'_, Usize>| -> Result<(), error::SystemError> {
                system_ran.fetch_add(1, Ordering::SeqCst);

                Ok(())
            },
        )
        .add_to_world(&world)
        .unwrap();

    let result = {
        let _usize = world.borrow::<UniqueViewMut<'_, Usize>>().unwrap();

        world.run_workload("borrow")
    };

    assert!(matches!(
        result,
        Err(error::RunWorkload::Run((_, error::Run::GetStorage(_))))
    ));
    assert_eq!(ran.load(Ordering::SeqCst), 0);
}

/// Asserts that `fails` stops two workloads using it through different builder paths,
/// the plain function first.
#[test]
fn fallible_across() {
    let world = World::new_for_test();
    Workload::new("first")
        .with_system(fails)
        .add_to_world(&world)
        .unwrap();
    Workload::new("second")
        .with_system(fails.tag("second"))
        .add_to_world(&world)
        .unwrap();

    assert!(custom_error(&world, "first"));
    assert!(custom_error(&world, "second"));
}

/// Asserts that `fails` stops two workloads using it through different builder paths,
/// the converted system first.
#[test]
fn fallible_across_converted_first() {
    let world = World::new_for_test();
    Workload::new("first")
        .with_system(fails.into_workload_system().unwrap())
        .add_to_world(&world)
        .unwrap();
    Workload::new("second")
        .with_system(fails)
        .add_to_world(&world)
        .unwrap();

    assert!(custom_error(&world, "first"));
    assert!(custom_error(&world, "second"));
}

/// Asserts that a workload stops with `fails`' error when its first occurrence is skipped.
#[test]
fn fallible_together_skipped_first() {
    let world = World::new_for_test();
    Workload::new("both")
        .with_system(fails.run_if(|| false))
        .with_system(fails)
        .add_to_world(&world)
        .unwrap();

    assert!(custom_error(&world, "both"));
}

/// Asserts that a workload stops with `fails`' error when its last occurrence is skipped.
#[test]
fn fallible_together_skipped_last() {
    let world = World::new_for_test();
    Workload::new("both")
        .with_system(fails)
        .with_system(fails.run_if(|| false))
        .add_to_world(&world)
        .unwrap();

    assert!(custom_error(&world, "both"));
}

/// Asserts that each occurrence of `fails` keeps its own condition:
/// with both skipped, the workload succeeds.
#[test]
fn fallible_together_skipped() {
    let world = World::new_for_test();
    Workload::new("both")
        .with_system(fails.run_if(|| false))
        .with_system(fails.skip_if(|| true))
        .add_to_world(&world)
        .unwrap();

    assert!(world.run_workload("both").is_ok());
}

/// Ordered after `fails` in `fallible_label_order`.
fn later() {}

/// Asserts that ordering after `fails` applies to all its occurrences, whatever their builder path.
#[test]
fn fallible_label_order() {
    let workload = Workload::new("order")
        .with_system(later.after_all(fails))
        .with_system(fails)
        .with_system(fails.into_workload_system().unwrap());

    assert_eq!(
        Schedule::built(workload).parallel,
        vec![steps(&["fails", "fails"]), steps(&["later"])]
    );
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

/// Asserts that two captures of the same closure type in one workload both run the first capture.
///
/// A world keeps one instance per callable type and adapter, the first one registered.
#[test]
fn capture_first_within() {
    let world = World::new_for_test();
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    Workload::new("")
        .with_system(counting(&first))
        .with_system(counting(&second))
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();

    assert_eq!(first.load(Ordering::SeqCst), 2);
    assert_eq!(second.load(Ordering::SeqCst), 0);
}

/// Asserts that a workload registering another capture of a closure type
/// already registered by an earlier workload runs the earlier capture.
#[test]
fn capture_first_across() {
    let world = World::new_for_test();
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    Workload::new("first")
        .with_system(counting(&first))
        .add_to_world(&world)
        .unwrap();
    Workload::new("second")
        .with_system(counting(&second))
        .add_to_world(&world)
        .unwrap();

    world.run_workload("second").unwrap();

    assert_eq!(first.load(Ordering::SeqCst), 1);
    assert_eq!(second.load(Ordering::SeqCst), 0);
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
