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
    let (scheduled, info, dependencies) = workload.build_unprepared().unwrap();
    let mut batches = scheduled.batches;
    let mut batches_info = info.batches_info;
    let sequential_before = user_sequential(&batches);
    let parallel_before = user_parallel(&batches);

    let all_storages = world.all_storages.borrow().unwrap();
    prepare_regroups(
        &mut batches,
        &mut batches_info,
        &all_storages.grouping,
        &dependencies,
    );

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

/// Returns the user steps with their condition and occurrence, in sequential order.
///
/// Asserts that regroups have no occurrence.
fn user_sequential(batches: &Batches) -> Vec<(Step, usize, usize)> {
    let steps = batches
        .sequential
        .iter()
        .copied()
        .zip(batches.sequential_run_if.iter().copied())
        .zip(batches.sequential_occurrences.iter().copied())
        .map(|((step, run_if), occurrence)| (step, run_if, occurrence));

    steps
        .filter(|&(step, _, occurrence)| match step {
            System(_) => true,
            Regroup(_) => {
                assert_eq!(occurrence, usize::MAX, "a regroup has an occurrence");
                false
            }
        })
        .collect()
}

/// Returns the user steps' callable index, condition and occurrence from the parallel batches,
/// sorted.
///
/// Asserts that regroups have no occurrence.
fn user_parallel(batches: &Batches) -> Vec<(usize, usize, usize)> {
    let mut parallel: Vec<(usize, usize, usize)> = batches
        .parallel
        .iter()
        .zip(&batches.parallel_run_if)
        .zip(&batches.parallel_occurrences)
        .flat_map(
            |(
                ((single, steps), (single_run_if, steps_run_if)),
                (single_occurrence, occurrences),
            )| {
                single
                    .iter()
                    .zip([(single_run_if, single_occurrence)])
                    .chain(steps.iter().zip(steps_run_if.iter().zip(occurrences)))
            },
        )
        .filter_map(|(step, (&run_if, &occurrence))| match step {
            System(index) => Some((*index, run_if, occurrence)),
            Regroup(_) => {
                assert_eq!(occurrence, usize::MAX, "a regroup has an occurrence");
                None
            }
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

/// Asserts that each occurrence of a repeated writer keeps its own occurrence once regroups are
/// placed, even when the sequential order differs from the declaration order.
///
/// `read_c` is declared first but follows both writers.
#[test]
fn repeated_occurrences() {
    let workload = Workload::new("")
        .with_system(read_c.after_all(write_a))
        .with_system(write_a)
        .with_system(write_a);

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.sequential,
        &[
            Regroup(0),
            System(1),
            Regroup(1),
            System(1),
            Regroup(2),
            System(0),
            Regroup(3)
        ]
    );
    let max = usize::MAX;
    assert_eq!(
        batches.sequential_occurrences,
        &[max, 1, max, 2, max, 0, max]
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

/// Asserts that the systems after a barrier stay in later batches than the ones before it,
/// once regroups are added. `prepare` checks that conditions and the sequential order don't change.
#[test]
fn user_order() {
    fn write_c(_: ViewMut<'_, C>) {}
    let workload = Workload::new("")
        .with_system(write_a.run_if(|| true))
        .with_system(read_b.tag("b"))
        .with_barrier()
        .with_system(read_a)
        .with_system(write_c.after_all("b"))
        .with_system(read_c.skip_if(|| false));

    let (batches, _) = prepare(workload, &world());

    for after in 2..5 {
        for before in 0..2 {
            assert!(batch_of(&batches, System(after)) > batch_of(&batches, System(before)));
        }
    }
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
            (None, vec![Regroup(1), System(2)]),
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
            (None, vec![Regroup(2), System(3), Regroup(3)]),
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

/// Borrows an ungrouped storage.
fn read_u16(_: View<'_, U16>) {}

/// Asserts that a regroup doesn't run alongside a system whose condition borrows from its set,
/// that system waits for it like `read_a`.
#[test]
fn condition_apart() {
    let workload = Workload::new("")
        .with_system(read_a)
        .with_system(read_u16.run_if(|_: View<'_, B>| true));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[(None, vec![Regroup(0)]), (None, vec![System(0), System(1)])]
    );
    assert_eq!(regroups(&batches), &[(Entry, set::<A, B>())]);
}

/// Asserts that a condition borrowing a grouped storage doesn't add a regroup.
#[test]
fn condition_no_trigger() {
    let workload = Workload::new("").with_system(read_u16.run_if(|_: ViewMut<'_, B>| true));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(batches.parallel, &[(None, vec![System(0)])]);
    assert!(batches.regroups.is_empty());
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
            (None, vec![Regroup(1), System(2)]),
            (None, vec![System(1)]),
            (None, vec![System(3)]),
        ]
    );
}

/// Asserts that a regroup needing the world's thread doesn't share a batch with
/// a system taking the world's thread slot.
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

/// Asserts that a set's regroup needing the world's thread runs alongside
/// the systems that don't borrow from it, when the world's thread slot is free.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_split() {
    use crate::borrow::NonSend;

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let workload = Workload::new("").with_system(read_a).with_system(read_c);

    let (batches, _) = prepare(workload, &world);

    assert_eq!(
        batches.parallel,
        &[(Some(Regroup(0)), vec![System(1)]), (None, vec![System(0)]),]
    );
}

/// Asserts that with two disjoint sets needing the world's thread, the reader of the set
/// regrouped first runs alongside the other set's regroup instead of waiting for it.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_two_sets() {
    use crate::borrow::NonSend;

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .with_group::<(C, NonSend<D>)>()
        .build();
    let workload = Workload::new("").with_system(read_a).with_system(read_c);

    let (batches, _) = prepare(workload, &world);

    // We don't rely on the sets' order, we look up which reader belongs to the first regroup.
    let first_reader = if batches.regroups[0].set.members.contains(&A::storage_id()) {
        System(0)
    } else {
        System(1)
    };
    assert_eq!(
        batch_of(&batches, first_reader),
        batch_of(&batches, Regroup(1))
    );
}

/// Asserts that a system unrelated to a due set runs alongside its regroup needing the world's thread,
/// even when another system of its batch takes the world's thread slot.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_free_local() {
    use crate::borrow::NonSend;

    fn local(_: NonSend<ViewMut<'_, Usize>>) {}

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let workload = Workload::new("")
        .with_system(read_a)
        .with_system(local)
        .with_system(read_c);

    let (batches, _) = prepare(workload, &world);

    assert_eq!(
        batch_of(&batches, System(2)),
        batch_of(&batches, Regroup(0))
    );
}

/// Asserts that a system whose condition needs the world's thread doesn't share a batch with
/// a regroup needing it too, while an unrelated system runs alongside the regroup.
#[cfg(feature = "thread_local")]
#[test]
fn non_send_condition() {
    use crate::borrow::{NonSend, NonSendSync};

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();
    let workload = Workload::new("")
        .with_system(read_a)
        .with_system(read_u16.run_if(|_: NonSendSync<View<'_, Usize>>| true))
        .with_system(read_c);

    let (batches, _) = prepare(workload, &world);

    assert_eq!(
        batches.parallel,
        &[
            (Some(Regroup(0)), vec![System(2)]),
            (Some(System(1)), vec![System(0)]),
        ]
    );
}

/// Asserts that two occurrences of a callable are placed by their own conditions' borrows:
/// the one whose condition reads B waits for the AB regroup, the other one runs alongside it.
#[test]
fn repeated_conditions() {
    let workload = Workload::new("")
        .with_system(read_a)
        .with_system(read_u16.run_if(|_: View<'_, B>| true))
        .with_system(read_u16.run_if(|_: View<'_, U32>| true));

    let (batches, _) = prepare(workload, &world());

    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0), System(1)]),
            (None, vec![System(0), System(1)]),
        ]
    );
    let max = usize::MAX;
    assert_eq!(
        batches.parallel_occurrences,
        &[(max, vec![max, 2]), (max, vec![0, 1])]
    );
}

/// Returns the index of the parallel batch running `step`.
fn batch_of(batches: &Batches, step: Step) -> usize {
    batches
        .parallel
        .iter()
        .position(|(single, steps)| *single == Some(step) || steps.contains(&step))
        .unwrap()
}

fn read_u32(_: View<'_, U32>) {}

/// Asserts that once a regroup splits `read A` from `write X`, `read X` runs alongside `read A`
/// instead of waiting for it, `read X` only waits for `write X`.
///
/// Without regroups, `write A`, `read A`, `write X` after `write A` and `read X` after `write X`
/// are built as `[write A] -> [read A | write X] -> [read X]`.
#[test]
fn stale_boundary() {
    let workload = || {
        Workload::new("")
            .with_system(write_a)
            .with_system(read_a)
            .with_system(write_u32.after_all(write_a))
            .with_system(read_u32.after_all(write_u32))
    };

    let built = Schedule::built(workload());
    let (batches, _) = prepare(workload(), &world());

    assert_eq!(
        built.parallel,
        vec![
            steps(&["write_a"]),
            steps(&["read_a", "write_u32"]),
            steps(&["read_u32"]),
        ]
    );
    assert_eq!(
        batches.parallel,
        &[
            (None, vec![Regroup(0)]),
            (None, vec![System(0)]),
            (None, vec![Regroup(1), System(2)]),
            (None, vec![System(1), System(3)]),
        ]
    );
}

/// Asserts that a barrier after `read A` keeps `read X` in a later batch,
/// even once a regroup split `read A` from `write X`.
#[test]
fn stale_barrier() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_u32.after_all(write_a))
        .with_barrier()
        .with_system(read_u32.after_all(write_u32));

    let (batches, _) = prepare(workload, &world());

    assert!(batch_of(&batches, System(3)) > batch_of(&batches, System(1)));
}

/// Asserts that `read X` explicitly following `read A` stays in a later batch,
/// even once a regroup split `read A` from `write X`.
#[test]
fn stale_after_read() {
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_u32.after_all(write_a))
        .with_system(read_u32.after_all(write_u32).after_all(read_a));

    let (batches, _) = prepare(workload, &world());

    assert!(batch_of(&batches, System(3)) > batch_of(&batches, System(1)));
}
