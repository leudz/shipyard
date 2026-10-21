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
