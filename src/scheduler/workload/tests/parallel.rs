use super::pending::{prepare, read_a, run, write_a};
use super::*;
use crate::error;
use crate::scheduler::Step;
use crate::scheduler::batches::RegroupHook;
use crate::scheduler::info::RegroupTrigger;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;
use std::time::Instant;

fn read_c(_: View<'_, C>) {}
fn write_b(_: ViewMut<'_, B>) {}

/// Lets two jobs check that they run at the same time.
#[derive(Clone, Default)]
struct Meeting(Arc<AtomicUsize>);

impl Meeting {
    /// Waits for the other job to arrive, for a few seconds at most.
    ///
    /// Panics if it doesn't arrive, the jobs didn't run at the same time.
    fn meet(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);

        let start = Instant::now();
        while self.0.load(Ordering::SeqCst) < 2 {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the other job didn't run at the same time"
            );
            std::thread::yield_now();
        }
    }
}

/// Makes the regroup at `index` call `hook` right before it runs.
fn hook(scheduled: &mut ScheduledWorkload, index: usize, hook: impl Fn() + Send + Sync + 'static) {
    scheduled.batches.regroups[index].hook = Some(RegroupHook(Arc::new(hook)));
}

/// Returns a world with the disjoint AB and CD groups, and an unevaluated entity in each.
fn two_sets() -> World {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    world.add_entity((A, B));
    world.add_entity((C, D));

    world
}

/// Runs a workload where the entry regroup shares its batch with an unrelated system,
/// and checks that they run at the same time.
fn regroup_next_to_system(world: World) {
    let meeting = Meeting::default();
    let unrelated = {
        let meeting = meeting.clone();
        move || meeting.meet()
    };
    let workload = Workload::new("").with_system(read_a).with_system(unrelated);
    let mut workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![System(1), Step::Regroup(0)])
    );
    hook(&mut workload, 0, move || meeting.meet());

    run(&workload, &world).unwrap();

    assert!(evaluated::<A>(&world));
}

/// Asserts that a regroup runs at the same time as a system that doesn't borrow from its set,
/// with the global thread pool.
#[test]
fn global_pool() {
    regroup_next_to_system(two_sets());
}

/// Asserts that a regroup runs at the same time as a system that doesn't borrow from its set,
/// with a world's own thread pool.
#[test]
fn local_pool() {
    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_local_thread_pool(thread_pool)
        .build();
    world.add_entity((A, B));

    regroup_next_to_system(world);
}

/// Asserts that the regroups of two disjoint connected sets run at the same time.
#[test]
fn disjoint_regroups() {
    let world = two_sets();
    let workload = Workload::new("").with_system(read_a).with_system(read_c);
    let mut workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![Step::Regroup(0), Step::Regroup(1)])
    );
    let meeting = Meeting::default();
    let other = meeting.clone();
    hook(&mut workload, 0, move || meeting.meet());
    hook(&mut workload, 1, move || other.meet());

    run(&workload, &world).unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<C>(&world));
}

/// Makes the regroup at `index` count its runs in `runs`, asserting it runs on the calling thread.
fn on_calling_thread(scheduled: &mut ScheduledWorkload, index: usize, runs: &Arc<AtomicUsize>) {
    let caller = std::thread::current().id();
    let runs = runs.clone();
    hook(scheduled, index, move || {
        assert_eq!(std::thread::current().id(), caller, "ran on a worker");
        runs.fetch_add(1, Ordering::SeqCst);
    });
}

/// Asserts that the clean regroups of two disjoint sets sharing a batch both run on the calling
/// thread, without waking a worker up for nothing to move.
#[test]
fn clean_regroups() {
    let mut world = two_sets();
    world.regroup();
    let workload = Workload::new("").with_system(read_a).with_system(read_c);
    let mut workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![Step::Regroup(0), Step::Regroup(1)])
    );
    let runs = Arc::new(AtomicUsize::new(0));
    on_calling_thread(&mut workload, 0, &runs);
    on_calling_thread(&mut workload, 1, &runs);

    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

/// Asserts that a clean regroup sharing its batch with an unrelated system runs on the calling
/// thread once the system ran there.
#[test]
fn clean_regroup_next_to_system() {
    let mut world = two_sets();
    world.regroup();
    let workload = Workload::new("").with_system(read_a).with_system(|| {});
    let mut workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![System(1), Step::Regroup(0)])
    );
    let runs = Arc::new(AtomicUsize::new(0));
    on_calling_thread(&mut workload, 0, &runs);

    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

/// Asserts that when a clean set and a set with work to do share a batch, both regroups run
/// and the set with work to do ends up regrouped.
#[test]
fn clean_and_dirty_regroups() {
    let mut world = two_sets();
    world.regroup();
    world.add_entity((C, D));
    let workload = Workload::new("").with_system(read_a).with_system(read_c);
    let mut workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![Step::Regroup(0), Step::Regroup(1)])
    );
    let runs = Arc::new(AtomicUsize::new(0));
    for index in 0..2 {
        let runs = runs.clone();
        hook(&mut workload, index, move || {
            runs.fetch_add(1, Ordering::SeqCst);
        });
    }

    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 2);
    assert!(evaluated::<A>(&world));
    assert!(evaluated::<C>(&world));
}

/// Asserts that systems writing different members of a set run together,
/// and a single regroup takes care of both before they're read.
#[test]
fn shared_regroup() {
    fn read_b(_: View<'_, B>) {}

    let world = two_sets();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_b)
        .with_system(read_a)
        .with_system(read_b);
    let mut workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[1],
        (None, vec![System(0), System(1)])
    );
    let pending_runs = Arc::new(AtomicUsize::new(0));
    for index in 0..workload.batches.regroups.len() {
        if workload.batches.regroups[index].trigger == RegroupTrigger::Pending {
            let pending_runs = pending_runs.clone();
            hook(&mut workload, index, move || {
                pending_runs.fetch_add(1, Ordering::SeqCst);
            });
        }
    }

    run(&workload, &world).unwrap();

    assert_eq!(pending_runs.load(Ordering::SeqCst), 1);
}

/// Asserts that a system error next to a running regroup returns the error,
/// leaves no storage borrowed, and that the next run's entry regroup takes care of the set.
#[test]
fn failure() {
    fn fail() -> Result<(), error::MissingComponent> {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "",
        })
    }

    let world = two_sets();
    let workload = Workload::new("").with_system(read_a).with_try_system(fail);
    let workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![System(1), Step::Regroup(0)])
    );

    let result = run(&workload, &world);

    assert!(matches!(result, Err(error::RunWorkload::Run(_))));
    drop(world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap());

    let mut world = world;
    world.add_entity((A, B));
    let reading = prepare(Workload::new("").with_system(read_a), &world);
    run(&reading, &world).unwrap();

    assert!(evaluated::<A>(&world));
}

/// Asserts that the regroup of a set with a `NonSend` member no system borrows
/// runs on the world's thread, next to an unrelated system.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_member() {
    use crate::AddComponent;
    use crate::borrow::NonSend;

    fn unrelated(_: UniqueView<'_, Usize>) {}

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    world.add_unique(Usize(0));
    let entity = world.add_entity(A);
    world
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B);
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(unrelated);
    let workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (Some(Step::Regroup(0)), vec![System(1)])
    );

    run(&workload, &world).unwrap();

    assert!(evaluated::<A>(&world));
}

/// Adds a reader of `C` per index, counting its runs in `runs[index]` and its condition's
/// evaluations in `evaluations[index]`. The condition skips the readers whose index is 2 modulo 3.
///
/// Every reader is its own closure: a workload keeps a single copy of a callable listed twice.
macro_rules! counted_readers {
    ($workload:expr, $runs:expr, $evaluations:expr; $($index:literal)*) => {
        $workload$(
            .with_system({
                let runs = $runs.clone();
                let evaluations = $evaluations.clone();

                (move |_: View<'_, C>| {
                    runs[$index].fetch_add(1, Ordering::SeqCst);
                })
                .run_if(move || {
                    evaluations[$index].fetch_add(1, Ordering::SeqCst);
                    $index % 3 != 2
                })
            })
        )*
    };
}

/// Runs before the counted readers and the skipped writer, they all wait for it.
fn prime(_: View<'_, A>, _: ViewMut<'_, C>) {}

/// Returns `count` counters at zero.
fn counters(count: usize) -> Arc<Vec<AtomicUsize>> {
    Arc::new((0..count).map(|_| AtomicUsize::new(0)).collect())
}

/// Returns the workload `prime`, eight counted readers, `write_a` skipped by its condition, the
/// other counted readers up to `last`, then `read_a`. The writer sits in the middle of the batch,
/// so its last slots hold readers.
macro_rules! condition_workload {
    ($runs:expr, $evaluations:expr; $($index:literal)*) => {
        counted_readers!(
            counted_readers!(
                Workload::new("").with_system(prime), $runs, $evaluations;
                0 1 2 3 4 5 6 7
            )
            .with_system(write_a.run_if(|| false)),
            $runs, $evaluations; $($index)*
        )
        .with_system(read_a)
    };
}

/// Runs a workload from `condition_workload!` with `readers` counted readers.
///
/// Asserts that the readers and the writer fill a single batch's parallel slots, that every
/// condition was evaluated once, that each reader allowed to run ran once and the others didn't,
/// and that the skipped writer left nothing for the regroup before `read_a`.
fn check_condition_slots(
    workload: Workload,
    readers: usize,
    runs: &[AtomicUsize],
    evaluations: &[AtomicUsize],
) {
    let world = pending::world();
    let mut workload = prepare(workload, &world);

    // Systems are numbered in order: `prime` is 0, then the readers and the writer.
    let slots: Vec<Step> = (1..=readers + 1).map(System).collect();
    let batch = workload
        .batches
        .parallel
        .iter()
        .find(|batch| batch.1.contains(&System(1)))
        .unwrap();
    assert_eq!(batch, &(None, slots));

    let pending_runs = Arc::new(AtomicUsize::new(0));
    for index in 0..workload.batches.regroups.len() {
        if workload.batches.regroups[index].trigger == RegroupTrigger::Pending {
            let pending_runs = pending_runs.clone();
            hook(&mut workload, index, move || {
                pending_runs.fetch_add(1, Ordering::SeqCst);
            });
        }
    }

    run(&workload, &world).unwrap();

    for index in 0..readers {
        let evaluated = evaluations[index].load(Ordering::SeqCst);
        assert_eq!(evaluated, 1, "condition {index}");
        let expected = usize::from(index % 3 != 2);
        assert_eq!(
            runs[index].load(Ordering::SeqCst),
            expected,
            "reader {index}"
        );
    }
    assert_eq!(pending_runs.load(Ordering::SeqCst), 0);
}

/// Asserts that a batch filling exactly the sixteen inline condition slots, with readers allowed
/// and skipped by their conditions and a skipped writer, runs each allowed reader once and leaves
/// nothing pending.
#[test]
fn sixteen_conditions() {
    let runs = counters(15);
    let evaluations = counters(15);
    let workload = condition_workload!(runs, evaluations; 8 9 10 11 12 13 14);

    check_condition_slots(workload, 15, &runs, &evaluations);
}

/// Asserts that a batch one slot past the sixteen inline condition slots keeps every condition
/// with its system: each allowed reader runs once, including the one in the seventeenth slot,
/// the others and the skipped writer don't, and nothing is left pending.
#[test]
fn seventeen_conditions() {
    let runs = counters(16);
    let evaluations = counters(16);
    let workload = condition_workload!(runs, evaluations; 8 9 10 11 12 13 14 15);

    check_condition_slots(workload, 16, &runs, &evaluations);
}
