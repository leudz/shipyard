//! Plans the regroups a registered workload runs around its systems.
//!
//! Planning uses the world's fixed connected sets and the systems' declared
//! borrows. It inserts entry regroups for changes made outside the workload,
//! then places later regroups before a written storage's next borrow or at exit.
//!
//! Deadlines belong to individual storages, but a regroup borrows its whole
//! connected set exclusively. With an A+B group, write A -> read B -> read A
//! can become write A -> read B -> regroup A+B -> read A. Placement must still
//! keep that regroup apart from every system borrowing A or B.
//!
//! A system's conditions run with it, so the regroup also stays apart from conditions
//! borrowing A or B. Conditions don't trigger regroups or set deadlines though.
//!
//! Regroups are then scheduled like systems: a regroup waits for the steps before it borrowing from its set
//! and the steps after it borrowing from its set wait for it.
//!
//! The plan is static: every regroup runs each time the workload reaches it,
//! even when the systems before it were skipped or only changed component values.
//! A regroup checks whether anything needs moving, a set without changes is cheap.

use crate::all_storages::{AllStorages, ConnectedSet, Grouping};
use crate::borrow::Mutability;
use crate::scheduler::info::{BatchInfo, Conflict, RegroupTrigger, SystemInfo, TypeInfo};
use crate::scheduler::workload::create_workload::{
    DedupedUniqueIds, Dependencies, Node, is_thread_bound, schedule,
};
use crate::scheduler::{Batches, RegroupStep, Step};
use crate::storage::StorageId;
use alloc::vec;
use alloc::vec::Vec;
use core::any::TypeId;
use core::mem;

/// Adds the regroups of a registered workload to its batches.
///
/// The workload regroups every connected set its systems borrow directly before their first borrow,
/// to take in the changes made outside of it.\
/// After a system borrows a grouped storage exclusively, its set is regrouped before the
/// storage's next direct borrow, or before the workload ends.
///
/// In the sequential order, each regroup goes right before the system needing it,
/// so it takes care of every write since the set's previous regroup.
/// The user systems' sequential order doesn't change.\
/// The batches are then made again with the regroups, following `dependencies`.
/// Each step only waits for its own predecessors, regroups included.
///
/// `batches` and `batches_info` have to be the ones `dependencies` was returned with,
/// `batches_info` gets the same changes as `batches`.
/// Systems waiting for a regroup get a conflict naming it.
pub(super) fn prepare_regroups(
    batches: &mut Batches,
    batches_info: &mut Vec<BatchInfo>,
    grouping: &Grouping,
    dependencies: &Dependencies,
) {
    let plan = sequential_plan(
        &batches.sequential_occurrences,
        &dependencies.borrows,
        grouping,
    );

    // Without regroups, the batches stay the same.
    if plan.len() == batches.sequential_occurrences.len() {
        return;
    }

    let nodes = nodes(&plan, &user_batches(batches), dependencies);
    let thread_bound: Vec<bool> = nodes
        .iter()
        .map(|node| is_thread_bound(&node.footprint))
        .collect();

    // Every edge goes from a step to a later one in `plan`, a valid sequential order.
    let Ok(scheduled) = schedule(nodes) else {
        unreachable!("the edges follow the sequential order, they can't make a cycle");
    };

    let user_steps = take_user_steps(batches, batches_info);
    put_batches(
        batches,
        batches_info,
        plan,
        user_steps,
        &scheduled.batches,
        &thread_bound,
    );
    point_conflicts_to_regroups(batches_info);
}

/// A step of the workload in the sequential order, before it's placed in a batch.
enum Planned {
    /// The system occurrence.
    System(usize),
    Regroup(RegroupStep),
}

/// Returns the systems in `sequential_occurrences`'s order, with the regroups they need inserted.
///
/// A set is regrouped right before the first system borrowing one of its storages directly,
/// to take in the changes made outside of the workload.
/// Then right before a system borrowing a storage of the set that was borrowed exclusively since the set's last regroup.\
/// The sets with such storages left are regrouped at the end, so the workload leaves them regrouped.
///
/// Reading another storage of the set doesn't need a regroup, it only has to stay apart from it.
fn sequential_plan(
    sequential_occurrences: &[usize],
    borrows: &[Vec<TypeInfo>],
    grouping: &Grouping,
) -> Vec<Planned> {
    let mut entered = vec![false; grouping.sets.len()];
    // Grouped storages a system might have changed since their set's last regroup, sorted.
    let mut pending: Vec<StorageId> = Vec::new();

    let mut plan = Vec::new();
    for &occurrence in sequential_occurrences {
        let borrow = &borrows[occurrence];

        for set in sets_of(
            grouping,
            borrow.iter().map(|type_info| &type_info.storage_id),
        ) {
            let members = &grouping.sets[set].members;

            let trigger = if !entered[set] {
                RegroupTrigger::Entry
            } else if borrow.iter().any(|type_info| {
                pending.binary_search(&type_info.storage_id).is_ok()
                    && members.binary_search(&type_info.storage_id).is_ok()
            }) {
                RegroupTrigger::Pending
            } else {
                continue;
            };

            plan.push(Planned::Regroup(RegroupStep::new(
                &grouping.sets[set],
                grouping,
                trigger,
            )));
            entered[set] = true;
            pending.retain(|storage_id| members.binary_search(storage_id).is_err());
        }

        plan.push(Planned::System(occurrence));

        for storage_id in grouped_writes(borrow, grouping) {
            if let Err(position) = pending.binary_search(storage_id) {
                pending.insert(position, *storage_id);
            }
        }
    }

    for set in sets_of(grouping, pending.iter()) {
        plan.push(Planned::Regroup(RegroupStep::new(
            &grouping.sets[set],
            grouping,
            RegroupTrigger::Pending,
        )));
    }

    plan
}

/// Returns the batch of each occurrence in `batches`, indexed by occurrence.
fn user_batches(batches: &Batches) -> Vec<usize> {
    let mut user_batches = vec![usize::MAX; batches.sequential_occurrences.len()];

    for (batch, (single, occurrences)) in batches.parallel_occurrences.iter().enumerate() {
        for &occurrence in occurrences {
            user_batches[occurrence] = batch;
        }
        if *single != usize::MAX {
            user_batches[*single] = batch;
        }
    }

    user_batches
}

/// Returns the node of each step of `plan`, edges pointing to positions in `plan`.
///
/// `user_batches` is the batch of each occurrence before regroups were added.
fn nodes(plan: &[Planned], user_batches: &[usize], dependencies: &Dependencies) -> Vec<Node> {
    let mut positions = vec![usize::MAX; dependencies.nodes.len()];
    for (position, planned) in plan.iter().enumerate() {
        if let Planned::System(occurrence) = planned {
            positions[*occurrence] = position;
        }
    }

    let mut nodes: Vec<Node> = plan
        .iter()
        .map(|planned| match planned {
            Planned::System(occurrence) => {
                let user = &dependencies.nodes[*occurrence];

                let mut batch_after = DedupedUniqueIds::new();
                for predecessor in user.batch_after.iter() {
                    batch_after.add(positions[predecessor]);
                }

                // The systems of a batch were free to run in any order, a regroup can now split them.
                // So we only keep the `order_after` edges between batches, for the others they're already met.
                let mut order_after = DedupedUniqueIds::new();
                for predecessor in user.order_after.iter() {
                    if user_batches[predecessor] != user_batches[*occurrence] {
                        order_after.add(positions[predecessor]);
                    }
                }

                Node {
                    callable: user.callable,
                    footprint: user.footprint.clone(),
                    batch_after,
                    order_after,
                }
            }
            Planned::Regroup(regroup) => Node {
                callable: usize::MAX,
                footprint: regroup.borrow.clone(),
                batch_after: DedupedUniqueIds::new(),
                order_after: DedupedUniqueIds::new(),
            },
        })
        .collect();

    // A regroup borrows its whole set exclusively, it stays in its sequential place
    // relative to every step borrowing from the set, conditions included.
    for (position, planned) in plan.iter().enumerate() {
        let Planned::Regroup(regroup) = planned else {
            continue;
        };

        for other in 0..nodes.len() {
            if other == position || !borrows_from(&nodes[other].footprint, &regroup.set) {
                continue;
            }

            if other < position {
                nodes[position].batch_after.add(other);
            } else {
                nodes[other].batch_after.add(position);
            }
        }
    }

    nodes
}

/// Returns `true` if `footprint` borrows one of `set`'s storages, `AllStorages` can reach all of them.
fn borrows_from(footprint: &[TypeInfo], set: &ConnectedSet) -> bool {
    footprint.iter().any(|type_info| {
        type_info.storage_id == TypeId::of::<AllStorages>()
            || set.members.binary_search(&type_info.storage_id).is_ok()
    })
}

/// A system occurrence as the batches had it before regroups were added.
struct UserStep {
    step: Step,
    /// Index of its condition in `Batches::systems_run_if`, `usize::MAX` without one.
    run_if: usize,
    info: SystemInfo,
}

/// Empties `batches` and `batches_info`, returning their user steps indexed by occurrence.
fn take_user_steps(
    batches: &mut Batches,
    batches_info: &mut Vec<BatchInfo>,
) -> Vec<Option<UserStep>> {
    batches.sequential.clear();
    batches.sequential_run_if.clear();
    let occurrence_count = mem::take(&mut batches.sequential_occurrences).len();

    let parallel = mem::take(&mut batches.parallel);
    let parallel_run_if = mem::take(&mut batches.parallel_run_if);
    let parallel_occurrences = mem::take(&mut batches.parallel_occurrences);
    let infos = mem::take(batches_info);

    let mut user_steps: Vec<Option<UserStep>> = (0..occurrence_count).map(|_| None).collect();
    for (
        ((single, steps), (single_run_if, steps_run_if)),
        ((single_occurrence, occurrences), info),
    ) in parallel
        .into_iter()
        .zip(parallel_run_if)
        .zip(parallel_occurrences.into_iter().zip(infos))
    {
        let (single_info, steps_info) = info.systems;

        if let (Some(step), Some(info)) = (single, single_info) {
            user_steps[single_occurrence] = Some(UserStep {
                step,
                run_if: single_run_if,
                info,
            });
        }

        for (((step, run_if), occurrence), info) in steps
            .into_iter()
            .zip(steps_run_if)
            .zip(occurrences)
            .zip(steps_info)
        {
            user_steps[occurrence] = Some(UserStep { step, run_if, info });
        }
    }

    user_steps
}

/// Fills the emptied `batches` and `batches_info` with `plan`'s steps.
///
/// `placed` lists the positions in `plan` of each batch's steps, `thread_bound` tells which one
/// goes in the calling-thread slot. The sequential order is `plan`'s.
fn put_batches(
    batches: &mut Batches,
    batches_info: &mut Vec<BatchInfo>,
    plan: Vec<Planned>,
    mut user_steps: Vec<Option<UserStep>>,
    placed: &[Vec<usize>],
    thread_bound: &[bool],
) {
    // Each step of `plan` with its condition, occurrence and info.
    // Regroups are numbered in the sequential order.
    let mut steps = Vec::with_capacity(plan.len());
    let mut infos = Vec::with_capacity(plan.len());
    for planned in plan {
        match planned {
            Planned::System(occurrence) => {
                let user = user_steps[occurrence]
                    .take()
                    .expect("each occurrence is in the batches once");

                steps.push((user.step, user.run_if, occurrence));
                infos.push(Some(user.info));
            }
            Planned::Regroup(regroup) => {
                steps.push((
                    Step::Regroup(batches.regroups.len()),
                    usize::MAX,
                    usize::MAX,
                ));
                infos.push(None);
                batches.regroups.push(regroup);
            }
        }
    }

    for &(step, run_if, occurrence) in &steps {
        batches.sequential.push(step);
        batches.sequential_run_if.push(run_if);
        batches.sequential_occurrences.push(occurrence);
    }

    for batch in placed {
        let mut parallel = (None, Vec::new());
        let mut parallel_run_if = (usize::MAX, Vec::new());
        let mut parallel_occurrences = (usize::MAX, Vec::new());
        let mut batch_info = BatchInfo {
            systems: (None, Vec::new()),
            regroups: Vec::new(),
        };

        for &position in batch {
            let (step, run_if, occurrence) = steps[position];

            if thread_bound[position] {
                parallel.0 = Some(step);
                parallel_run_if.0 = run_if;
                parallel_occurrences.0 = occurrence;
            } else {
                parallel.1.push(step);
                parallel_run_if.1.push(run_if);
                parallel_occurrences.1.push(occurrence);
            }

            match (step, infos[position].take()) {
                (Step::Regroup(index), _) => {
                    batch_info.regroups.push(batches.regroups[index].info());
                }
                (Step::System(_), Some(info)) if thread_bound[position] => {
                    batch_info.systems.0 = Some(info);
                }
                (Step::System(_), Some(info)) => batch_info.systems.1.push(info),
                (Step::System(_), None) => unreachable!("each step is placed once"),
            }
        }

        batches.parallel.push(parallel);
        batches.parallel_run_if.push(parallel_run_if);
        batches.parallel_occurrences.push(parallel_occurrences);
        batches_info.push(batch_info);
    }
}

/// Returns the grouped storages `borrow` borrows exclusively, the ones it leaves pending.
fn grouped_writes<'a>(
    borrow: &'a [TypeInfo],
    grouping: &'a Grouping,
) -> impl Iterator<Item = &'a StorageId> {
    // We can't know what a system does with an exclusive borrow,
    // even one only updating values could add or remove components.
    borrow
        .iter()
        .filter(|type_info| type_info.mutability == Mutability::Exclusive)
        .map(|type_info| &type_info.storage_id)
        .filter(|storage_id| set_index(grouping, storage_id).is_some())
}

/// Returns the sorted indexes of the connected sets holding `storage_ids`.
///
/// Storages outside every group are skipped.
fn sets_of<'a>(
    grouping: &Grouping,
    storage_ids: impl Iterator<Item = &'a StorageId>,
) -> Vec<usize> {
    let mut sets: Vec<usize> = storage_ids
        .filter_map(|storage_id| set_index(grouping, storage_id))
        .collect();
    sets.sort_unstable();
    sets.dedup();

    sets
}

/// Returns the index of the connected set holding `storage_id`, `None` if it isn't grouped.
fn set_index(grouping: &Grouping, storage_id: &StorageId) -> Option<usize> {
    grouping
        .sets
        .iter()
        .position(|set| set.members.binary_search(storage_id).is_ok())
}

/// Gives the systems waiting for a regroup of the previous batch a conflict naming it.
///
/// Conflicts explain why a system couldn't join the previous batch. With regroup batches in
/// between, the system named by a conflict can now be further back.
/// We keep the conflicts that still name a system of the previous batch.
fn point_conflicts_to_regroups(batches_info: &mut [BatchInfo]) {
    for index in 1..batches_info.len() {
        let (before, after) = batches_info.split_at_mut(index);
        let previous = &before[index - 1];
        let batch = &mut after[0];

        for system in batch.systems.0.iter_mut().chain(&mut batch.systems.1) {
            let names_previous = match &system.conflict {
                Some(
                    Conflict::Borrow { other_system, .. }
                    | Conflict::OtherNotSendSync {
                        system: other_system,
                        ..
                    },
                ) => previous
                    .systems()
                    .any(|other| other.unique_id == *other_system),
                _ => false,
            };
            if names_previous {
                continue;
            }

            let waited_for = system.borrow.iter().find(|type_info| {
                previous.regroups.iter().any(|regroup| {
                    regroup
                        .borrow
                        .iter()
                        .any(|member| member.storage_id == type_info.storage_id)
                })
            });
            if let Some(type_info) = waited_for {
                system.conflict = Some(Conflict::Regroup(type_info.clone()));
            }
        }
    }
}
