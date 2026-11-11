use super::*;
use crate::error;
use crate::scheduler::info::RegroupTrigger;
use crate::scheduler::workload::prepare::prepare_regroups;
use crate::views::EntitiesViewMut;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(super) fn read_a(_: View<'_, A>) {}
/// Only updates values, so it doesn't change the group.
pub(super) fn write_a(_: ViewMut<'_, A>) {}

/// Adds an `(A, B)` entity, it stays out of the group until a regroup.
fn add_ab(mut entities: EntitiesViewMut<'_>, mut a: ViewMut<'_, A>, mut b: ViewMut<'_, B>) {
    entities.add_entity((&mut a, &mut b), (A, B));
}

/// Adds an `(A, B)` entity through `AllStorages`, which workloads don't track.
fn add_ab_indirectly(mut all_storages: AllStoragesViewMut<'_>) {
    all_storages.add_entity((A, B));
}

/// Returns a world with the AB group.
pub(super) fn world() -> World {
    World::builder_for_test().with_group::<(A, B)>().build()
}

/// Builds `workload` with the regroups it needs in `world`.
pub(super) fn prepare(workload: Workload, world: &World) -> ScheduledWorkload {
    let (mut scheduled, mut info, dependencies) = workload.build_unprepared().unwrap();

    let all_storages = world.all_storages.borrow().unwrap();
    prepare_regroups(
        &mut scheduled.batches,
        &mut info.batches_info,
        &all_storages.grouping,
        &dependencies,
    );

    scheduled
}

/// Runs `scheduled` once on `world`.
pub(super) fn run(scheduled: &ScheduledWorkload, world: &World) -> Result<(), error::RunWorkload> {
    world.run_batches(
        &scheduled.systems,
        &scheduled.system_names,
        &scheduled.batches,
        &*scheduled.name,
    )
}

/// Asserts that a system borrowing a grouped storage exclusively makes the next regroup
/// of its set run, even when it only updates values.
/// The regroup takes in what was added through `AllStorages` in between.
#[test]
fn executed() {
    let world = world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(add_ab_indirectly)
        .with_system(read_a);
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
}

/// Asserts that the regroup placed after a system runs even when its condition skipped it.
/// The regroup takes in what was added through `AllStorages` in between.
#[test]
fn skipped() {
    let world = world();
    let workload = Workload::new("")
        .with_system(write_a.run_if(|| false))
        .with_system(add_ab_indirectly)
        .with_system(read_a);
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
}

/// Asserts that every run starts with the entry regroup, for the changes made outside the workload.
#[test]
fn repeated() {
    let mut world = world();
    let workload = prepare(Workload::new("").with_system(read_a), &world);

    world.add_entity((A, B));
    run(&workload, &world).unwrap();
    assert!(evaluated::<A>(&world));

    world.add_entity((A, B));
    run(&workload, &world).unwrap();
    assert!(evaluated::<A>(&world));
}

/// Asserts that the workload regroups what its last system left pending before it ends.
#[test]
fn final_writer() {
    let world = world();
    let workload = Workload::new("").with_system(read_a).with_system(add_ab);
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
}

/// Asserts that a condition borrowing a grouped storage exclusively leaves nothing pending,
/// what it adds stays out of the group.
#[test]
fn condition() {
    let world = world();
    let workload = Workload::new("").with_system(read_a.run_if(
        |mut entities: EntitiesViewMut<'_>, mut a: ViewMut<'_, A>, mut b: ViewMut<'_, B>| {
            entities.add_entity((&mut a, &mut b), (A, B));

            true
        },
    ));
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(!evaluated::<A>(&world));
    assert!(!evaluated::<B>(&world));
}

/// Asserts that a condition borrowing a grouped storage exclusively and returning `false`
/// leaves nothing pending, what it adds stays out of the group.
#[test]
fn condition_false() {
    let world = world();
    let workload = Workload::new("").with_system(read_a.run_if(
        |mut entities: EntitiesViewMut<'_>, mut a: ViewMut<'_, A>, mut b: ViewMut<'_, B>| {
            entities.add_entity((&mut a, &mut b), (A, B));

            false
        },
    ));
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(!evaluated::<A>(&world));
    assert!(!evaluated::<B>(&world));
}

/// Asserts that a system changing a grouped storage through `AllStorages` leaves nothing pending.
#[test]
fn indirect() {
    let world = world();
    let workload = Workload::new("")
        .with_system(read_a)
        .with_system(add_ab_indirectly);
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(!evaluated::<A>(&world));
    assert!(!evaluated::<B>(&world));
}

/// Asserts that a workload stopped by a system error doesn't regroup what it left pending,
/// and that the next workload's entry regroup takes care of it.
#[test]
fn system_error() {
    fn add_ab_then_fail(
        entities: EntitiesViewMut<'_>,
        a: ViewMut<'_, A>,
        b: ViewMut<'_, B>,
    ) -> Result<(), error::SystemError> {
        add_ab(entities, a, b);

        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "",
        }
        .into())
    }

    let world = world();
    let failing = Workload::new("")
        .with_system(read_a)
        .with_system(add_ab_then_fail);
    let failing = prepare(failing, &world);
    let reading = prepare(Workload::new("").with_system(read_a), &world);

    let result = run(&failing, &world);

    assert!(matches!(result, Err(error::RunWorkload::Run(_))));
    assert!(!evaluated::<A>(&world));

    run(&reading, &world).unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
}

/// Asserts that a condition pushed behind a regroup sees the regrouped storage: the read of a
/// written storage waits for the regroup, and so does the read's condition.
#[test]
fn delayed() {
    let world = world();
    let saw_regrouped = Arc::new(AtomicBool::new(false));
    let seen = saw_regrouped.clone();
    let workload = Workload::new("")
        .with_system(add_ab)
        .with_system(read_a.run_if(move |a: View<'_, A>| {
            seen.store(a.groups.unevaluated_start == a.len(), Ordering::Relaxed);

            true
        }));
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(saw_regrouped.load(Ordering::Relaxed));
}

/// Grouped components for the six-set workload, grouped in pairs.
macro_rules! grouped_components {
    ($($name:ident),*) => {
        $(
            struct $name;
            impl Component for $name {
                type Tracking = crate::track::Untracked;
            }
        )*
    };
}

grouped_components!(G0, G1, G2, G3, G4, G5, G6, G7, G8, G9, G10, G11);

/// Returns a world grouping `G0` to `G11` in six pairs: `(G0, G1)`, `(G2, G3)`...
fn twelve_world() -> World {
    World::builder_for_test()
        .with_group::<(G0, G1)>()
        .with_group::<(G2, G3)>()
        .with_group::<(G4, G5)>()
        .with_group::<(G6, G7)>()
        .with_group::<(G8, G9)>()
        .with_group::<(G10, G11)>()
        .build()
}

fn write_low(
    _: ViewMut<'_, G0>,
    _: ViewMut<'_, G1>,
    _: ViewMut<'_, G2>,
    _: ViewMut<'_, G3>,
    _: ViewMut<'_, G4>,
    _: ViewMut<'_, G5>,
) {
}

fn write_high(
    _: ViewMut<'_, G6>,
    _: ViewMut<'_, G7>,
    _: ViewMut<'_, G8>,
    _: ViewMut<'_, G9>,
    _: ViewMut<'_, G10>,
    _: ViewMut<'_, G11>,
) {
}

fn read_g0(_: View<'_, G0>) {}

fn write_g1(_: ViewMut<'_, G1>) {}

/// Adds an entity to every set through `AllStorages`, only the regroups after it take it in.
fn add_twelve(mut all_storages: AllStoragesViewMut<'_>) {
    all_storages.add_entity((G0, G1, G2, G3, G4, G5));
    all_storages.add_entity((G6, G7, G8, G9, G10, G11));
}

/// Adds a `(G0, G1)` entity through `AllStorages`.
fn add_g0_g1(mut all_storages: AllStoragesViewMut<'_>) {
    all_storages.add_entity((G0, G1));
}

/// Returns `true` when every storage of the twelve was evaluated by a regroup.
fn twelve_evaluated(world: &World) -> bool {
    evaluated::<G0>(world)
        && evaluated::<G1>(world)
        && evaluated::<G2>(world)
        && evaluated::<G3>(world)
        && evaluated::<G4>(world)
        && evaluated::<G5>(world)
        && evaluated::<G6>(world)
        && evaluated::<G7>(world)
        && evaluated::<G8>(world)
        && evaluated::<G9>(world)
        && evaluated::<G10>(world)
        && evaluated::<G11>(world)
}

/// Asserts that a workload writing the six sets runs each of their regroups where the plan
/// puts them: `(G0, G1)` alone before `G0` is read, then the six sets when the workload ends.
#[test]
fn six_sets() {
    let world = twelve_world();
    let workload = Workload::new("")
        .with_system(write_low)
        .with_system(write_high)
        .with_system(add_twelve)
        .with_system(read_g0)
        .with_system(write_g1)
        .with_system(add_g0_g1);
    let mut workload = prepare(workload, &world);
    let runs = Arc::new(AtomicUsize::new(0));
    for regroup in &mut workload.batches.regroups {
        if regroup.trigger == RegroupTrigger::Pending {
            let runs = runs.clone();
            regroup.hook = Some(crate::scheduler::batches::RegroupHook(Arc::new(
                move || {
                    runs.fetch_add(1, Ordering::SeqCst);
                },
            )));
        }
    }

    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 7);
    assert!(twelve_evaluated(&world));
}
