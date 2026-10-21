use super::*;
use crate::all_storages::GroupMember;
use crate::scheduler::Step;
use crate::scheduler::Step::Regroup;
use crate::scheduler::info::BatchInfo;
use crate::scheduler::info::RegroupTrigger::{self, Entry, Pending};
use crate::scheduler::workload::prepare::prepare_regroups;
use crate::storage::StorageId;

fn write_a(_: ViewMut<'_, A>) {}
fn read_a(_: View<'_, A>) {}
fn write_b(_: ViewMut<'_, B>) {}
fn read_b(_: View<'_, B>) {}
fn read_c(_: View<'_, C>) {}

/// Returns a world with the disjoint AB and CD groups.
fn world() -> World {
    World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build()
}

/// Builds `workload` and prepares its regroups for `world`'s groups.
///
/// Checks that every user step is still there once, with its condition,
/// and that the sequential order of user steps didn't change.
fn prepare(workload: Workload, world: &World) -> (Batches, Vec<BatchInfo>) {
    let (scheduled, info) = workload.build().unwrap();
    let mut batches = scheduled.batches;
    let mut batches_info = info.batches_info;
    let sequential_before = user_sequential(&batches);
    let parallel_before = user_parallel(&batches);

    let all_storages = world.all_storages.borrow().unwrap();
    prepare_regroups(&mut batches, &mut batches_info, &all_storages.grouping);

    assert_eq!(user_sequential(&batches), sequential_before);
    assert_eq!(user_parallel(&batches), parallel_before);
    let parallel_len: usize = batches
        .parallel
        .iter()
        .map(|(single, steps)| usize::from(single.is_some()) + steps.len())
        .sum();
    assert_eq!(batches.sequential.len(), parallel_len);

    (batches, batches_info)
}

/// Returns the user steps with their condition, in sequential order.
fn user_sequential(batches: &Batches) -> Vec<(Step, usize)> {
    batches
        .sequential
        .iter()
        .copied()
        .zip(batches.sequential_run_if.iter().copied())
        .filter(|(step, _)| matches!(step, System(_)))
        .collect()
}

/// Returns the user steps' callable index and condition from the parallel batches, sorted.
fn user_parallel(batches: &Batches) -> Vec<(usize, usize)> {
    let mut parallel: Vec<(usize, usize)> = batches
        .parallel
        .iter()
        .zip(&batches.parallel_run_if)
        .flat_map(|((single, steps), (single_run_if, steps_run_if))| {
            single
                .iter()
                .zip([single_run_if])
                .chain(steps.iter().zip(steps_run_if))
        })
        .filter_map(|(step, &run_if)| match step {
            System(index) => Some((*index, run_if)),
            Regroup(_) => None,
        })
        .collect();
    parallel.sort_unstable();

    parallel
}

/// Returns the trigger and connected set of each regroup, in step order.
fn regroups(batches: &Batches) -> Vec<(RegroupTrigger, Vec<StorageId>)> {
    batches
        .regroups
        .iter()
        .map(|regroup| (regroup.trigger, regroup.set.members.clone()))
        .collect()
}

/// Returns the connected set made of `T` and `U`'s storages.
fn set<T: GroupMember, U: GroupMember>() -> Vec<StorageId> {
    let mut members = vec![T::storage_id(), U::storage_id()];
    members.sort_unstable();

    members
}

/// Asserts that a written storage is regrouped before it's read, after the entry regroup.
#[test]
fn write_read() {
    let workload = Workload::new("").with_system(write_a).with_system(read_a);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![Regroup(1)]),
            (None, vec![System(1)]),
        ]
    );
    assert_eq!(
        batches.sequential,
        &[Regroup(0), System(0), Regroup(1), System(1)]
    );
    assert_eq!(
        regroups(&batches),
        &[(Entry, set::<A, B>()), (Pending, set::<A, B>())]
    );
}

/// Asserts that consecutive writers of a storage have a regroup between them,
/// and the last one is regrouped before the workload ends.
#[test]
fn write_write() {
    fn write_a_again(_: ViewMut<'_, A>) {}

    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_a_again);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![Regroup(1)]),
            (None, vec![System(1)]),
            (None, vec![Regroup(2)]),
        ]
    );
    assert_eq!(
        regroups(&batches),
        &[
            (Entry, set::<A, B>()),
            (Pending, set::<A, B>()),
            (Pending, set::<A, B>())
        ]
    );
}

/// Asserts that two occurrences of the same writer get the same regroups as two writers.
#[test]
fn repeated() {
    let workload = Workload::new("").with_system(write_a).with_system(write_a);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.sequential,
        &[Regroup(0), System(0), Regroup(1), System(0), Regroup(2)]
    );
}

/// Asserts that reading another member of the set doesn't need the pending regroup,
/// only reading the written storage does.
#[test]
fn other_member() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_b.after_all(write_a))
        .with_system(read_a.after_all(read_b));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![System(1)]),
            (None, vec![Regroup(1)]),
            (None, vec![System(2)]),
        ]
    );
}

/// Asserts that writers of different members stay in the same batch and share a regroup.
#[test]
fn shared() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_b)
        .with_system(read_a)
        .with_system(read_b);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0), System(1)]),
            (None, vec![Regroup(1)]),
            (None, vec![System(2), System(3)]),
        ]
    );
    assert_eq!(
        regroups(&batches),
        &[(Entry, set::<A, B>()), (Pending, set::<A, B>())]
    );
}

/// Asserts that a system writing to two connected sets gets both regrouped, in one batch.
#[test]
fn several_sets() {
    fn write_a_c(_: ViewMut<'_, A>, _: ViewMut<'_, C>) {}

    let workload = Workload::new("")
        .with_system(write_a_c)
        .with_system(read_a)
        .with_system(read_c);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0), Regroup(1)]),
            (None, vec![System(0)]),
            (None, vec![Regroup(2), Regroup(3)]),
            (None, vec![System(1), System(2)]),
        ]
    );
    assert_eq!(
        regroups(&batches),
        &[
            (Entry, set::<A, B>()),
            (Entry, set::<C, D>()),
            (Pending, set::<A, B>()),
            (Pending, set::<C, D>())
        ]
    );
}

/// Asserts that a workload only reading gets its entry regroup and nothing else.
#[test]
fn read_only() {
    let workload = Workload::new("").with_system(read_a);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[(None, vec![Regroup(0)]), (None, vec![System(0)])]
    );
    assert_eq!(regroups(&batches), &[(Entry, set::<A, B>())]);
}

/// Asserts that a workload without direct borrows of grouped storages gets no regroup,
/// even when it borrows `AllStorages`.
#[test]
fn ungrouped() {
    fn other(_: View<'_, U32>, _: UniqueViewMut<'_, Usize>) {}
    fn all(_: AllStoragesViewMut<'_>) {}
    let workload = || Workload::new("").with_system(other).with_system(all);
    let (unprepared, _) = workload().build().unwrap();

    let (batches, _) = prepare(workload(), &world());

    assert_eq!(batches, unprepared.batches);
}

/// Asserts that regroups are only added to the batches: barriers, ordering constraints
/// and conditions still apply to user systems as before.
#[test]
fn user_order() {
    fn write_c(_: ViewMut<'_, C>) {}
    let workload = || {
        Workload::new("")
            .with_system(write_a.run_if(|| true))
            .with_system(read_b.tag("b"))
            .with_barrier()
            .with_system(read_a)
            .with_system(write_c.after_all("b"))
            .with_system(read_c.skip_if(|| false))
    };
    let (unprepared, _) = workload().build().unwrap();

    let (batches, _) = prepare(workload(), &world());

    let user_batches: Vec<(Option<Step>, Vec<Step>)> = batches
        .parallel
        .iter()
        .map(|(single, steps)| {
            let is_system = |step: &Step| matches!(step, System(_));

            (
                single.filter(is_system),
                steps.iter().copied().filter(is_system).collect::<Vec<_>>(),
            )
        })
        .filter(|(single, steps)| single.is_some() || !steps.is_empty())
        .collect();
    assert_eq!(user_batches, unprepared.batches.parallel);
}

/// Asserts that workload information gets the same batches, regroup batches listing their regroups.
#[test]
fn info() {
    let workload = Workload::new("").with_system(write_a).with_system(read_a);

    let (batches, info) = prepare(workload, &world());

    assert_eq!(info.len(), batches.parallel.len());
    assert_eq!(info[0].regroups, &[batches.regroups[0].info()]);
    assert_eq!(info[0].systems().count(), 0);
    assert!(info[1].regroups.is_empty());
    assert_eq!(info[1].systems().count(), 1);
    assert_eq!(info[2].regroups, &[batches.regroups[1].info()]);
    assert_eq!(info[3].systems().count(), 1);
}

/// Asserts that sets needing the world's thread don't share a batch,
/// a batch only has one slot running there.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::borrow::NonSend;

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .with_group::<(C, NonSend<D>)>()
        .build();
    let workload = Workload::new("").with_system(|_: View<'_, A>, _: View<'_, C>| {});

    let (batches, _) = prepare(workload, &world);

    assert_eq!(
        batches.parallel,
        &[
            (Some(Regroup(0)), vec![]),
            (Some(Regroup(1)), vec![]),
            (None, vec![System(0)]),
        ]
    );
    assert_eq!(batches.parallel_run_if[0], (usize::MAX, vec![]));
    assert_eq!(batches.sequential, &[Regroup(0), Regroup(1), System(0)]);
}

fn write_u32(_: ViewMut<'_, U32>) {}

/// Asserts that the regroup of a write waits for a later write to another member,
/// a single regroup taking care of both.
#[test]
fn late_sharing() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_b.after_all(write_a))
        .with_system(read_a.after_all(write_b))
        .with_system(read_b.after_all(write_b));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![System(1)]),
            (None, vec![Regroup(1)]),
            (None, vec![System(2), System(3)]),
        ]
    );
    assert_eq!(
        regroups(&batches),
        &[(Entry, set::<A, B>()), (Pending, set::<A, B>())]
    );
}

/// Asserts that a regroup runs alongside an unrelated system between the write and the read,
/// instead of in its own batch.
#[test]
fn unrelated_batch() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_u32.after_all(write_a))
        .with_system(read_a.after_all(write_u32));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![System(1), Regroup(1)]),
            (None, vec![System(2)]),
        ]
    );
    assert_eq!(
        batches.sequential,
        &[Regroup(0), System(0), System(1), Regroup(1), System(2)]
    );
}

/// Asserts that `write A -> [read A | X]` becomes `write A -> [regroup AB | X] -> read A`,
/// the read keeping its condition and its place in the sequential order.
#[test]
fn split() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a.run_if(|| true))
        .with_system(write_u32.after_all(write_a));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![System(2), Regroup(1)]),
            (None, vec![System(1)]),
        ]
    );
    assert_eq!(batches.parallel_run_if[3], (usize::MAX, vec![0]));
    assert_eq!(
        batches.sequential,
        &[Regroup(0), System(0), Regroup(1), System(1), System(2)]
    );
}

/// Asserts that the regroups of two sets due before the same batch both run alongside
/// the systems that don't need them.
#[test]
fn several_due() {
    fn write_a_c(_: ViewMut<'_, A>, _: ViewMut<'_, C>) {}

    let workload = Workload::new("")
        .with_system(write_a_c)
        .with_system(read_a)
        .with_system(read_c)
        .with_system(write_u32.after_all(write_a_c));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0), Regroup(1)]),
            (None, vec![System(0)]),
            (None, vec![System(3), Regroup(2), Regroup(3)]),
            (None, vec![System(1), System(2)]),
        ]
    );
}

/// Asserts that the final regroup runs alongside the last batch when it can.
#[test]
fn exit_alongside() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_u32.after_all(write_a));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![System(1), Regroup(1)]),
        ]
    );
}

/// Asserts that a regroup doesn't join a batch with a system borrowing `AllStorages`,
/// it gets its own batch.
#[test]
fn all_storages() {
    fn all(_: AllStoragesViewMut<'_>) {}

    let workload = Workload::new("").with_system(write_a).with_system(all);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (Some(System(1)), vec![]),
            (None, vec![Regroup(1)]),
        ]
    );
}

/// Asserts that a split keeps barriers and ordering constraints:
/// the systems after the split batch still come after both its parts.
#[test]
fn split_constraints() {
    fn after(_: View<'_, U16>) {}

    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_u32.after_all(write_a).tag("u32"))
        .with_barrier()
        .with_system(after.after_all("u32"));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![System(2), Regroup(1)]),
            (None, vec![System(1)]),
            (None, vec![System(3)]),
        ]
    );
}

/// Asserts that a regroup needing the world's thread doesn't join a batch whose
/// world's thread slot is taken, the batch then waits for it whole.
#[cfg(feature = "thread_local")]
#[test]
fn thread_slot() {
    use crate::borrow::NonSend;

    fn local(_: NonSend<ViewMut<'_, Usize>>) {}

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(local.after_all(write_a));

    let (batches, _) = prepare(workload, &world);

    assert_eq!(
        batches.parallel,
        &[
            (Some(Regroup(0)), vec![]),
            (None, vec![System(0)]),
            (Some(Regroup(1)), vec![]),
            (Some(System(2)), vec![System(1)]),
        ]
    );
}
