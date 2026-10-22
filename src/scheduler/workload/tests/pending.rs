use super::*;
use crate::all_storages::ConnectedSet;
use crate::error;
use crate::scheduler::info::RegroupTrigger;
use crate::scheduler::workload::prepare::prepare_regroups;
use crate::scheduler::{RegroupStep, Step};
use crate::storage::StorageId;
use crate::views::EntitiesViewMut;
use crate::world::Pending;
use alloc::sync::Arc;
use core::ops::Range;
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

/// Returns the custom storage ids `ids`.
fn custom(ids: Range<u64>) -> Vec<StorageId> {
    ids.map(StorageId::Custom).collect()
}

/// Returns batches where system `i` writes the storages `writes[i]`, and regroup `j` regroups
/// the set of storages `sets[j]`, all custom ids.
fn spill_batches(writes: &[Vec<u64>], sets: &[Vec<u64>]) -> Batches {
    let ids = |list: &Vec<u64>| list.iter().map(|&id| StorageId::Custom(id)).collect();

    Batches {
        grouped_writes: writes.iter().map(ids).collect(),
        regroups: sets
            .iter()
            .map(|members| RegroupStep {
                set: ConnectedSet {
                    members: ids(members),
                    thread_safe: true,
                },
                borrow: Vec::new(),
                trigger: RegroupTrigger::Pending,
                hook: None,
            })
            .collect(),
        ..Batches::default()
    }
}

/// Asserts that eight pending storages fit inline, and that a ninth moves them all to the heap
/// without losing any.
#[test]
fn pending_spill() {
    let batches = spill_batches(&[(0..8).collect(), vec![8]], &[]);
    let mut pending = Pending::default();

    pending.ran(&batches, System(0));
    assert!(pending.is_inline());
    assert_eq!(pending.ids(), custom(0..8));

    pending.ran(&batches, System(1));
    assert!(!pending.is_inline());
    assert_eq!(pending.ids(), custom(0..9));
}

/// Asserts that writing pending storages again lists each of them once, whether they're inline
/// or on the heap.
#[test]
fn pending_duplicates() {
    let writes = [(0..8).collect(), vec![0, 7, 8], vec![3, 8, 9]];
    let batches = spill_batches(&writes, &[]);
    let mut pending = Pending::default();

    pending.ran(&batches, System(0));
    pending.ran(&batches, System(1));
    pending.ran(&batches, System(2));

    assert_eq!(pending.ids(), custom(0..10));
}

/// Returns twelve pending storages, over six sets of two: regroup `k` regroups storages `2k` and
/// `2k + 1`. System 2 writes storage 4 again.
fn twelve_pending() -> (Batches, Pending) {
    let writes = [(0..6).collect(), (6..12).collect(), vec![4]];
    let sets: Vec<Vec<u64>> = (0..6).map(|k| vec![2 * k, 2 * k + 1]).collect();
    let batches = spill_batches(&writes, &sets);

    let mut pending = Pending::default();
    pending.ran(&batches, System(0));
    pending.ran(&batches, System(1));
    assert!(!pending.is_inline());

    (batches, pending)
}

/// Asserts that with twelve storages pending, a regroup clears only its own set's members:
/// its set doesn't need regrouping any more, every other set still does.
#[test]
fn pending_partial_clear() {
    let (batches, mut pending) = twelve_pending();

    pending.ran(&batches, Step::Regroup(2));

    let mut expected = custom(0..12);
    expected.retain(|&id| id != StorageId::Custom(4) && id != StorageId::Custom(5));
    assert_eq!(pending.ids(), expected);
    assert!(!pending.needs(&batches, Step::Regroup(2)));
    for set in [0, 1, 3, 4, 5] {
        assert!(pending.needs(&batches, Step::Regroup(set)));
    }
}

/// Asserts that a storage written again after its set was regrouped is pending again, once.
#[test]
fn pending_after_clear() {
    let (batches, mut pending) = twelve_pending();

    pending.ran(&batches, Step::Regroup(2));
    pending.ran(&batches, System(2));

    assert!(pending.needs(&batches, Step::Regroup(2)));
    let ids = pending.ids();
    assert_eq!(ids.len(), 11);
    assert_eq!(
        ids.iter().filter(|&&id| id == StorageId::Custom(4)).count(),
        1
    );
}

/// Grouped components for the workloads leaving twelve storages pending, grouped in pairs.
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

/// Adds an entity to every set through `AllStorages`, only pending regroups take it in.
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

/// Returns the workload leaving twelve storages pending, its writers running while `writes` is
/// `true`, with every pending regroup counted in `runs`.
///
/// The two writers leave all twelve pending, then `add_twelve` adds to every set. Reading `G0`
/// regroups `(G0, G1)` alone, `write_g1` makes it pending again before `add_g0_g1` adds to it,
/// and the workload ends with the six sets pending.
fn twelve_workload(
    world: &World,
    writes: &Arc<AtomicBool>,
    runs: &Arc<AtomicUsize>,
) -> ScheduledWorkload {
    let condition = |writes: &Arc<AtomicBool>| {
        let writes = writes.clone();
        move || writes.load(Ordering::Relaxed)
    };
    let workload = Workload::new("")
        .with_system(write_low.run_if(condition(writes)))
        .with_system(write_high.run_if(condition(writes)))
        .with_system(add_twelve)
        .with_system(read_g0)
        .with_system(write_g1.run_if(condition(writes)))
        .with_system(add_g0_g1);
    let mut workload = prepare(workload, world);

    // Both writers run before any pending regroup, so all twelve storages are pending at once.
    let position = |wanted: Step| {
        workload
            .batches
            .sequential
            .iter()
            .position(|&step| step == wanted)
    };
    let first_pending = workload
        .batches
        .sequential
        .iter()
        .position(|&step| {
            matches!(step, Step::Regroup(index)
                if workload.batches.regroups[index].trigger == RegroupTrigger::Pending)
        })
        .unwrap();
    assert!(position(System(0)).unwrap() < first_pending);
    assert!(position(System(1)).unwrap() < first_pending);

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

    workload
}

/// Asserts that a workload leaving twelve storages pending at once regroups every one of them:
/// `(G0, G1)` alone before `G0` is read, then again with the five other sets when the workload ends.
#[test]
fn twelve_pending_flushed() {
    let world = twelve_world();
    let writes = Arc::new(AtomicBool::new(true));
    let runs = Arc::new(AtomicUsize::new(0));
    let workload = twelve_workload(&world, &writes, &runs);

    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 7);
    assert!(twelve_evaluated(&world));
}

/// Asserts that the next run starts with nothing pending: with its writers skipped, no pending
/// regroup runs and what it adds stays out of the groups.
#[test]
fn twelve_pending_fresh() {
    let world = twelve_world();
    let writes = Arc::new(AtomicBool::new(true));
    let runs = Arc::new(AtomicUsize::new(0));
    let workload = twelve_workload(&world, &writes, &runs);
    run(&workload, &world).unwrap();
    runs.store(0, Ordering::SeqCst);

    writes.store(false, Ordering::Relaxed);
    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 0);
    assert!(!evaluated::<G2>(&world));
    assert!(!evaluated::<G11>(&world));
}
