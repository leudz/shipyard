use super::*;
use crate::all_storages::GroupMember;
use crate::borrow::Mutability;
use crate::error;
use crate::scheduler::info::{RegroupInfo, RegroupTrigger, TypeInfo};
use crate::scheduler::{RegroupStep, Step};
use crate::sparse_set::SparseSet;
use alloc::borrow::Cow;
use core::any::type_name;

/// Returns a world with the disjoint AB and CD, three unevaluated entities in each.
fn two_sets() -> World {
    let mut world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(C, D)>()
        .build();
    for _ in 0..3 {
        world.add_entity((A, B));
        world.add_entity((C, D));
    }

    world
}

/// Returns the entry regroup of the connected set holding `T`.
fn entry_regroup<T: GroupMember>(world: &World) -> RegroupStep {
    let all_storages = world.all_storages.borrow().unwrap();
    let grouping = &all_storages.grouping;
    let set = grouping.set_of(T::storage_id()).unwrap();

    RegroupStep::new(set, grouping, RegroupTrigger::Entry)
}

/// Returns batches running `regroups` together, in a single batch.
fn one_batch(regroups: Vec<RegroupStep>) -> Batches {
    let steps: Vec<Step> = (0..regroups.len()).map(Step::Regroup).collect();

    Batches {
        parallel: vec![(None, steps.clone())],
        parallel_run_if: vec![(usize::MAX, vec![usize::MAX; steps.len()])],
        sequential_run_if: vec![usize::MAX; steps.len()],
        sequential: steps,
        regroups,
        ..Batches::default()
    }
}

/// Asserts that a regroup step borrows every storage of its connected set exclusively,
/// including the ones only reached through another group, and lists them in workload information.
#[test]
fn footprint() {
    let world = World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, C)>()
        .build();

    let regroup = entry_regroup::<A>(&world);

    let member = |storage_id, name: &'static str| TypeInfo {
        name: Cow::Borrowed(name),
        mutability: Mutability::Exclusive,
        storage_id,
        thread_safe: true,
    };
    let mut expected = vec![
        member(A::storage_id(), type_name::<SparseSet<A>>()),
        member(B::storage_id(), type_name::<SparseSet<B>>()),
        member(C::storage_id(), type_name::<SparseSet<C>>()),
    ];
    expected.sort_unstable();
    assert_eq!(regroup.borrow, expected);
    assert!(
        regroup
            .borrow
            .iter()
            .zip(&expected)
            .all(|(a, b)| a.name == b.name)
    );
    assert_eq!(
        regroup.info(),
        RegroupInfo {
            borrow: expected,
            trigger: RegroupTrigger::Entry,
        }
    );
}

/// Asserts that two occurrences of the same callable keep their own condition:
/// the callable is stored once, and only the occurrence whose condition holds runs.
#[test]
fn repeated_occurrence() {
    fn count(mut counter: UniqueViewMut<'_, Usize>) {
        counter.0 += 1;
    }

    let world = World::new_for_test();
    world.add_unique(Usize(0));
    Workload::new("")
        .with_system(count.run_if(|| false))
        .with_system(count.run_if(|| true))
        .add_to_world(&world)
        .unwrap();

    {
        let scheduler = world.scheduler.borrow().unwrap();
        let batches = scheduler.default_workload();
        assert_eq!(scheduler.systems.len(), 1);
        assert_eq!(batches.sequential, &[System(0), System(0)]);
        assert_ne!(batches.sequential_run_if[0], batches.sequential_run_if[1]);
    }
    world.run_default_workload().unwrap();

    assert_eq!(world.borrow::<UniqueView<'_, Usize>>().unwrap().0, 1);
}

/// Asserts that a batch holding the regroups of two independent connected sets runs both.
#[test]
fn independent_regroups() {
    let world = two_sets();
    let batches = one_batch(vec![entry_regroup::<A>(&world), entry_regroup::<C>(&world)]);

    world.run_batches(&[], &[], &batches, &"").unwrap();

    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
    assert!(evaluated::<C>(&world));
    assert!(evaluated::<D>(&world));
}

/// Asserts that a registered workload runs a regroup step sharing a batch with a system,
/// and that the regroup leaves the other connected set alone.
#[test]
fn next_to_system() {
    fn count(mut counter: UniqueViewMut<'_, Usize>) {
        counter.0 += 1;
    }

    let world = two_sets();
    world.add_unique(Usize(0));
    Workload::new("")
        .with_system(count)
        .add_to_world(&world)
        .unwrap();
    let regroup = entry_regroup::<A>(&world);
    {
        let mut scheduler = world.scheduler.borrow_mut().unwrap();
        let batches = scheduler.workloads.values_mut().next().unwrap();
        batches.regroups.push(regroup);
        batches.parallel[0].1.push(Step::Regroup(0));
        batches.parallel_run_if[0].1.push(usize::MAX);
        batches.sequential.push(Step::Regroup(0));
        batches.sequential_run_if.push(usize::MAX);
    }

    world.run_default_workload().unwrap();

    assert_eq!(world.borrow::<UniqueView<'_, Usize>>().unwrap().0, 1);
    assert!(evaluated::<A>(&world));
    assert!(evaluated::<B>(&world));
    assert!(!evaluated::<C>(&world));
    assert!(!evaluated::<D>(&world));
}

/// Asserts that a regroup step failing to borrow a member returns a regroup error naming it,
/// and moves nothing.
#[test]
fn borrow_error() {
    let world = two_sets();
    let batches = one_batch(vec![entry_regroup::<A>(&world)]);
    let b = world.borrow::<ViewMut<'_, B>>().unwrap();

    let result = world.run_batches(&[], &[], &batches, &"");

    let Err(error::RunWorkload::Regroup(error::GetStorage::StorageBorrow { name, .. })) = result
    else {
        panic!("expected a regroup borrow error, got {:?}", result);
    };
    assert_eq!(name, Some(type_name::<SparseSet<B>>()));
    drop(b);
    assert!(!evaluated::<A>(&world));
}

/// Asserts that a regroup step of a set with a `NonSend` member can run in a batch's
/// calling-thread slot while another regroup runs in the same batch.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::AddComponent;
    use crate::borrow::NonSend;

    let mut world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .with_group::<(C, D)>()
        .build();
    for _ in 0..3 {
        let entity = world.add_entity(A);
        world
            .borrow::<NonSend<ViewMut<'_, B>>>()
            .unwrap()
            .add_component_unchecked(entity, B);
        world.add_entity((C, D));
    }
    let mut batches = one_batch(vec![
        entry_regroup::<NonSend<B>>(&world),
        entry_regroup::<C>(&world),
    ]);
    batches.parallel = vec![(Some(Step::Regroup(0)), vec![Step::Regroup(1)])];
    batches.parallel_run_if = vec![(usize::MAX, vec![usize::MAX])];

    world.run_batches(&[], &[], &batches, &"").unwrap();

    assert!(evaluated::<A>(&world));
    let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
    assert_eq!(b.groups.unevaluated_start, b.len());
    assert!(evaluated::<C>(&world));
    assert!(evaluated::<D>(&world));
}
