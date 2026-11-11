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

/// Asserts that the regroups of two clean sets sharing a batch both run,
/// a regroup runs each time the workload reaches it.
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
    for index in 0..2 {
        let runs = runs.clone();
        hook(&mut workload, index, move || {
            runs.fetch_add(1, Ordering::SeqCst);
        });
    }

    run(&workload, &world).unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 2);
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
    fn fail() -> Result<(), error::SystemError> {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "",
        }
        .into())
    }

    let world = two_sets();
    let workload = Workload::new("").with_system(read_a).with_system(fail);
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
/// condition was evaluated once, and that each reader allowed to run ran once and the others didn't.
fn check_condition_slots(
    workload: Workload,
    readers: usize,
    runs: &[AtomicUsize],
    evaluations: &[AtomicUsize],
) {
    let world = pending::world();
    let workload = prepare(workload, &world);

    // Systems are numbered in order: `prime` is 0, then the readers and the writer.
    let slots: Vec<Step> = (1..=readers + 1).map(System).collect();
    let batch = workload
        .batches
        .parallel
        .iter()
        .find(|batch| batch.1.contains(&System(1)))
        .unwrap();
    assert_eq!(batch, &(None, slots));

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
}

/// Asserts that in a batch of sixteen systems with conditions, readers allowed and skipped by
/// their conditions and a skipped writer, each condition is evaluated once and each allowed reader
/// runs once.
#[test]
fn sixteen_conditions() {
    let runs = counters(15);
    let evaluations = counters(15);
    let workload = condition_workload!(runs, evaluations; 8 9 10 11 12 13 14);

    check_condition_slots(workload, 15, &runs, &evaluations);
}

/// Returns a two-worker pool whose threads are named `custom-<index>`,
/// so tests can tell its workers apart from the global pool's.
fn named_pool() -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .thread_name(|index| alloc::format!("custom-{index}"))
        .build()
        .unwrap()
}

/// Returns a world with its own [`named_pool`] and the storages of [`non_send_world`].
#[cfg(feature = "thread_local")]
fn local_pool_world() -> World {
    non_send_world(World::builder_for_test().with_local_thread_pool(named_pool()))
}

/// Returns a world using the global pool, with the same storages as [`local_pool_world`].
#[cfg(feature = "thread_local")]
fn global_pool_world() -> World {
    non_send_world(World::builder_for_test())
}

/// Builds `builder` with the AB group with a `NonSend<B>` member and an unevaluated entity in it.
///
/// It also creates the ungrouped `NonSend<D>` and `NonSendSync<D>` storages.
/// Everything is created from the calling thread, which makes it these storages' owner.
#[cfg(feature = "thread_local")]
fn non_send_world(
    builder: crate::world::WorldBuilder<
        crate::all_storages::LockPresent,
        crate::all_storages::ThreadIdPresent,
    >,
) -> World {
    use crate::AddComponent;
    use crate::borrow::{NonSend, NonSendSync};

    let mut world = builder.with_group::<(A, NonSend<B>)>().build();
    let entity = world.add_entity(A);
    world
        .borrow::<NonSend<ViewMut<'_, B>>>()
        .unwrap()
        .add_component_unchecked(entity, B);
    world.borrow::<NonSend<ViewMut<'_, D>>>().unwrap();
    world.borrow::<NonSendSync<ViewMut<'_, D>>>().unwrap();

    world
}

/// Records the threads something ran on.
#[derive(Clone, Default)]
struct Threads(Arc<std::sync::Mutex<Vec<std::thread::Thread>>>);

impl Threads {
    fn record(&self) {
        self.0.lock().unwrap().push(std::thread::current());
    }

    fn ids(&self) -> Vec<std::thread::ThreadId> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|thread| thread.id())
            .collect()
    }

    fn names(&self) -> Vec<Option<String>> {
        let threads = self.0.lock().unwrap();

        threads
            .iter()
            .map(|thread| thread.name().map(Into::into))
            .collect()
    }
}

/// Runs a system borrowing `NonSend<ViewMut<D>>` on `world`, in one batch with `read_c`,
/// returns its result and the threads the `NonSend` system ran on.
///
/// `D` isn't grouped, so there's no regroup step in the workload.
/// `read_c` is declared first: if the `NonSend` system wasn't given the calling-thread slot,
/// the runner's habit of running a batch's first ordinary system on the caller
/// would pick `read_c` and leave the `NonSend` system on a worker.
#[cfg(feature = "thread_local")]
fn run_non_send_system(world: &World) -> (Result<(), error::RunWorkload>, Threads) {
    use crate::borrow::NonSend;

    let threads = Threads::default();
    let system = {
        let threads = threads.clone();
        move |_: NonSend<ViewMut<'_, D>>| threads.record()
    };
    let workload = Workload::new("").with_system(read_c).with_system(system);
    let workload = prepare(workload, world);
    assert_eq!(
        workload.batches.parallel,
        [(Some(System(1)), vec![System(0)])]
    );

    (run(&workload, world), threads)
}

/// Asserts that with the global pool, a `NonSend` system takes the calling-thread slot
/// of a batch it shares with a thread-safe system, and runs on the calling thread.
#[cfg(feature = "thread_local")]
#[test]
fn global_pool_non_send_system() {
    let world = global_pool_world();

    let (result, threads) = run_non_send_system(&world);

    result.unwrap();
    assert_eq!(threads.ids(), [std::thread::current().id()]);
}

/// Asserts that with a world's own pool, a `NonSend` system takes the calling-thread slot
/// of a batch it shares with a thread-safe system, and runs on the calling thread.
#[cfg(feature = "thread_local")]
#[test]
fn local_pool_non_send_system() {
    let world = local_pool_world();

    let (result, threads) = run_non_send_system(&world);

    result.unwrap();
    assert_eq!(threads.ids(), [std::thread::current().id()]);
}

/// Runs a workload where the entry regroup of the AB set is the only step reading `NonSend<B>`,
/// next to `read_a` which only borrows the thread-safe member.
/// Returns the result and the threads the regroup ran on.
#[cfg(feature = "thread_local")]
fn run_non_send_regroup(world: &World) -> (Result<(), error::RunWorkload>, Threads) {
    let mut workload = prepare(Workload::new("").with_system(read_a), world);
    assert_eq!(
        workload.batches.parallel[0],
        (Some(Step::Regroup(0)), vec![])
    );
    let threads = Threads::default();
    let recorder = threads.clone();
    hook(&mut workload, 0, move || recorder.record());

    (run(&workload, world), threads)
}

/// Asserts that with the global pool, the regroup of a set with a `NonSend` member
/// runs on the calling thread.
#[cfg(feature = "thread_local")]
#[test]
fn global_pool_non_send_regroup() {
    let world = global_pool_world();

    let (result, threads) = run_non_send_regroup(&world);

    result.unwrap();
    assert_eq!(threads.ids(), [std::thread::current().id()]);
    assert!(evaluated::<A>(&world));
}

/// Asserts that with a world's own pool, the regroup of a set with a `NonSend` member
/// runs on the calling thread.
#[cfg(feature = "thread_local")]
#[test]
fn local_pool_non_send_regroup() {
    let world = local_pool_world();

    let (result, threads) = run_non_send_regroup(&world);

    result.unwrap();
    assert_eq!(threads.ids(), [std::thread::current().id()]);
    assert!(evaluated::<A>(&world));
}

/// Runs a thread-safe system whose condition borrows `NonSendSync<View<D>>`,
/// in one batch with a thread-safe peer they meet with.
/// Returns the result and the threads the condition, then its system, ran on.
///
/// Unlike `NonSend`, a `NonSendSync` storage can't even be read from another thread.
/// The condition takes its system to the calling-thread slot, the peer is declared first
/// so the runner's habit of running a batch's first system on the caller can't do it instead.
#[cfg(feature = "thread_local")]
fn run_non_send_condition(world: &World) -> (Result<(), error::RunWorkload>, Threads) {
    use crate::borrow::NonSendSync;

    let threads = Threads::default();
    let meeting = Meeting::default();
    let peer = {
        let meeting = meeting.clone();
        move |_: View<'_, C>| meeting.meet()
    };
    let system = {
        let threads = threads.clone();
        move |_: View<'_, C>| {
            threads.record();
            meeting.meet();
        }
    };
    let condition = {
        let threads = threads.clone();
        move |_: NonSendSync<View<'_, D>>| {
            threads.record();
            true
        }
    };
    let workload = Workload::new("")
        .with_system(peer)
        .with_system(system.run_if(condition));
    let workload = prepare(workload, world);
    assert_eq!(
        workload.batches.parallel,
        [(Some(System(1)), vec![System(0)])]
    );

    (run(&workload, world), threads)
}

/// Asserts that with the global pool, a condition borrowing `NonSendSync` storage
/// and its system run on the calling thread, while a peer runs alongside them.
#[cfg(feature = "thread_local")]
#[test]
fn global_pool_non_send_condition() {
    let world = global_pool_world();

    let (result, threads) = run_non_send_condition(&world);

    result.unwrap();
    let caller = std::thread::current().id();
    assert_eq!(threads.ids(), [caller, caller]);
}

/// Asserts that with a world's own pool, a condition borrowing `NonSendSync` storage
/// and its system run on the calling thread, while a peer runs alongside them.
#[cfg(feature = "thread_local")]
#[test]
fn local_pool_non_send_condition() {
    let world = local_pool_world();

    let (result, threads) = run_non_send_condition(&world);

    result.unwrap();
    let caller = std::thread::current().id();
    assert_eq!(threads.ids(), [caller, caller]);
}

/// Asserts that the conditions of two systems in the same batch run at the same time.
#[test]
fn conditions_overlap() {
    let world = World::new_for_test();
    let meeting = Meeting::default();
    let first = (|_: View<'_, C>| {}).run_if({
        let meeting = meeting.clone();
        move || {
            meeting.meet();
            true
        }
    });
    let second = (|_: View<'_, Usize>| {}).run_if(move || {
        meeting.meet();
        true
    });
    let workload = Workload::new("").with_system(first).with_system(second);
    let workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel,
        [(None, vec![System(0), System(1)])]
    );

    run(&workload, &world).unwrap();
}

/// Returns a condition recording its thread in `threads`.
fn recording_condition(threads: &Threads) -> impl Fn() -> bool + Clone + Send + Sync + 'static {
    let threads = threads.clone();

    move || {
        threads.record();
        true
    }
}

/// Asserts that a condition runs on the same thread as its system,
/// for both systems of a batch, one of them on a worker.
#[test]
fn condition_system_thread() {
    let world = World::new_for_test();
    let meeting = Meeting::default();
    let (first, second) = (Threads::default(), Threads::default());
    let first_system = {
        let (threads, meeting) = (first.clone(), meeting.clone());
        move |_: View<'_, C>| {
            threads.record();
            meeting.meet();
        }
    };
    let second_system = {
        let threads = second.clone();
        move |_: View<'_, Usize>| {
            threads.record();
            meeting.meet();
        }
    };
    let workload = Workload::new("")
        .with_system(first_system.run_if(recording_condition(&first)))
        .with_system(second_system.run_if(recording_condition(&second)));
    let workload = prepare(workload, &world);

    run(&workload, &world).unwrap();

    let (first, second) = (first.ids(), second.ids());
    assert_eq!((first[0], second[0]), (first[1], second[1]));
    assert_ne!(first[0], second[0]);
}

/// Asserts that with a world's own pool, the thread-safe systems of a batch
/// run in that pool or on the calling thread, never in the global pool.
///
/// The two systems meet so they run at the same time, one of them has to be on a worker.
#[test]
fn local_pool_workers() {
    let world = World::builder_for_test()
        .with_local_thread_pool(named_pool())
        .build();
    let threads = Threads::default();
    let meeting = Meeting::default();
    let first = {
        let (threads, meeting) = (threads.clone(), meeting.clone());
        move |_: View<'_, C>| {
            threads.record();
            meeting.meet();
        }
    };
    let second = {
        let threads = threads.clone();
        move |_: View<'_, Usize>| {
            threads.record();
            meeting.meet();
        }
    };
    let workload = Workload::new("").with_system(first).with_system(second);
    let workload = prepare(workload, &world);
    assert_eq!(
        workload.batches.parallel[0],
        (None, vec![System(0), System(1)])
    );

    run(&workload, &world).unwrap();

    // Whatever didn't run on the calling thread has to be on one of the world's workers.
    let caller = std::thread::current().id();
    let on_worker = threads
        .ids()
        .into_iter()
        .zip(threads.names())
        .filter(|(id, _)| *id != caller)
        .map(|(_, name)| name)
        .collect::<Vec<_>>();
    assert!(!on_worker.is_empty());
    for name in on_worker {
        assert!(
            name.as_deref()
                .is_some_and(|name| name.starts_with("custom-")),
            "ran on {name:?}"
        );
    }
}
