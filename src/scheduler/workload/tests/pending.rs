use super::*;
use crate::error;
use crate::scheduler::workload::prepare::prepare_regroups;
use crate::views::EntitiesViewMut;

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
    let (mut scheduled, mut info) = workload.build().unwrap();

    let all_storages = world.all_storages.borrow().unwrap();
    prepare_regroups(
        &mut scheduled.batches,
        &mut info.batches_info,
        &all_storages.grouping,
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

/// Asserts that a system skipped by its condition leaves nothing pending,
/// so the regroup after it doesn't run.
#[test]
fn skipped() {
    let world = world();
    let workload = Workload::new("")
        .with_system(write_a.run_if(|| false))
        .with_system(add_ab_indirectly)
        .with_system(read_a);
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(!evaluated::<A>(&world));
    assert!(!evaluated::<B>(&world));
}

/// Asserts that a regroup clears its set's pending storages:
/// once the first write is regrouped, the second one is skipped and the last regroup doesn't run.
#[test]
fn cleared() {
    let world = world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_a.run_if(|| false))
        .with_system(add_ab_indirectly)
        .with_system(read_a);
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    assert!(!evaluated::<A>(&world));
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
    ) -> Result<(), error::MissingComponent> {
        add_ab(entities, a, b);

        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "",
        })
    }

    let world = world();
    let failing = Workload::new("")
        .with_system(read_a)
        .with_try_system(add_ab_then_fail);
    let failing = prepare(failing, &world);
    let reading = prepare(Workload::new("").with_system(read_a), &world);

    let result = run(&failing, &world);

    assert!(matches!(result, Err(error::RunWorkload::Run(_))));
    assert!(!evaluated::<A>(&world));

    run(&reading, &world).unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
}
