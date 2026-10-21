//! Plans the regroups a registered workload runs around its systems.

use crate::all_storages::Grouping;
use crate::borrow::Mutability;
use crate::scheduler::info::{BatchInfo, RegroupTrigger, TypeInfo};
use crate::scheduler::{Batches, RegroupStep, Step};
use crate::storage::StorageId;
use alloc::vec::Vec;
use core::mem;

/// A batch and everything describing it.
struct Batch {
    parallel: (Option<Step>, Vec<Step>),
    parallel_run_if: (usize, Vec<usize>),
    /// The batch's steps, in sequential order.
    sequential: Vec<Step>,
    sequential_run_if: Vec<usize>,
    info: BatchInfo,
}

impl Batch {
    fn empty() -> Batch {
        Batch {
            parallel: (None, Vec::new()),
            parallel_run_if: (usize::MAX, Vec::new()),
            sequential: Vec::new(),
            sequential_run_if: Vec::new(),
            info: BatchInfo {
                systems: (None, Vec::new()),
                regroups: Vec::new(),
            },
        }
    }

    /// Returns every storage borrow of the batch's systems.
    ///
    /// Conditions and accesses through `AllStorages` aren't part of it.
    fn borrows(&self) -> impl Iterator<Item = &TypeInfo> {
        self.info.systems().flat_map(|system| &system.borrow)
    }
}

/// Adds regroup batches to `batches`, `batches_info` describing them gets the same batches.
///
/// The workload starts by regrouping every connected set its systems borrow directly,
/// to take in the changes made outside of it.\
/// After a system borrows a grouped storage exclusively, its set is regrouped before the
/// storage's next direct borrow, or before the workload ends.
///
/// User batches don't change, regroups get their own batches between them.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "workloads don't plan regroups yet")
)]
pub(super) fn prepare_regroups(
    batches: &mut Batches,
    batches_info: &mut Vec<BatchInfo>,
    grouping: &Grouping,
) {
    if grouping.sets.is_empty() {
        return;
    }

    let user_batches = take_batches(batches, batches_info);
    let mut planned = Vec::with_capacity(user_batches.len());

    let entry_sets = sets_of(
        grouping,
        user_batches
            .iter()
            .flat_map(Batch::borrows)
            .map(|type_info| type_info.storage_id),
    );
    push_regroups(
        &mut planned,
        &mut batches.regroups,
        grouping,
        &entry_sets,
        RegroupTrigger::Entry,
    );

    // Grouped storages a system might have changed since their set's last regroup, sorted.
    let mut pending: Vec<StorageId> = Vec::new();
    for batch in user_batches {
        // A pending storage has to be regrouped before a system borrows it again.
        // Other members of its set can be used in the meantime, they don't wait for it.
        let due_sets = sets_of(
            grouping,
            batch
                .borrows()
                .map(|type_info| type_info.storage_id)
                .filter(|storage_id| pending.binary_search(storage_id).is_ok()),
        );
        push_regroups(
            &mut planned,
            &mut batches.regroups,
            grouping,
            &due_sets,
            RegroupTrigger::Pending,
        );

        // Regrouping a set takes care of all its members.
        pending.retain(|storage_id| {
            due_sets.iter().all(|&set| {
                grouping.sets[set]
                    .members
                    .binary_search(storage_id)
                    .is_err()
            })
        });

        // We can't know what a system does with an exclusive borrow,
        // even one only updating values could add or remove components.
        for type_info in batch.borrows() {
            let storage_id = type_info.storage_id;

            if type_info.mutability == Mutability::Exclusive
                && grouping.set_of(storage_id).is_some()
                && let Err(index) = pending.binary_search(&storage_id)
            {
                pending.insert(index, storage_id);
            }
        }

        planned.push(batch);
    }

    // The workload leaves every set regrouped.
    let exit_sets = sets_of(grouping, pending.into_iter());
    push_regroups(
        &mut planned,
        &mut batches.regroups,
        grouping,
        &exit_sets,
        RegroupTrigger::Pending,
    );

    put_batches(batches, batches_info, planned);
}

/// Empties `batches` and `batches_info` into a list of batches.
fn take_batches(batches: &mut Batches, batches_info: &mut Vec<BatchInfo>) -> Vec<Batch> {
    let mut sequential = mem::take(&mut batches.sequential).into_iter();
    let mut sequential_run_if = mem::take(&mut batches.sequential_run_if).into_iter();

    mem::take(&mut batches.parallel)
        .into_iter()
        .zip(mem::take(&mut batches.parallel_run_if))
        .zip(mem::take(batches_info))
        .map(|((parallel, parallel_run_if), info)| {
            // Systems are added to the sequential order as they're placed in batches,
            // so each batch's steps are next to each other, in batch order.
            let len = usize::from(parallel.0.is_some()) + parallel.1.len();

            Batch {
                parallel,
                parallel_run_if,
                sequential: sequential.by_ref().take(len).collect(),
                sequential_run_if: sequential_run_if.by_ref().take(len).collect(),
                info,
            }
        })
        .collect()
}

/// Puts `planned` back into `batches` and `batches_info`, in order.
fn put_batches(batches: &mut Batches, batches_info: &mut Vec<BatchInfo>, planned: Vec<Batch>) {
    for batch in planned {
        batches.parallel.push(batch.parallel);
        batches.parallel_run_if.push(batch.parallel_run_if);
        batches.sequential.extend(batch.sequential);
        batches.sequential_run_if.extend(batch.sequential_run_if);
        batches_info.push(batch.info);
    }
}

/// Returns the sorted indexes of the connected sets holding `storage_ids`.
///
/// Storages outside every group are skipped.
fn sets_of(grouping: &Grouping, storage_ids: impl Iterator<Item = StorageId>) -> Vec<usize> {
    let mut sets: Vec<usize> = storage_ids
        .filter_map(|storage_id| {
            grouping
                .sets
                .iter()
                .position(|set| set.members.binary_search(&storage_id).is_ok())
        })
        .collect();
    sets.sort_unstable();
    sets.dedup();

    sets
}

/// Adds batches regrouping each of `sets`, as few as possible.
///
/// Sets are disjoint so they can regroup in parallel, but a batch only has one slot
/// for work that has to run on the world's thread.
fn push_regroups(
    planned: &mut Vec<Batch>,
    regroups: &mut Vec<RegroupStep>,
    grouping: &Grouping,
    sets: &[usize],
    trigger: RegroupTrigger,
) {
    let mut batch = Batch::empty();
    for &set in sets {
        let regroup = RegroupStep::new(&grouping.sets[set], grouping, trigger);
        let step = Step::Regroup(regroups.len());

        if regroup.set.thread_safe {
            batch.parallel.1.push(step);
            batch.parallel_run_if.1.push(usize::MAX);
        } else {
            if batch.parallel.0.is_some() {
                planned.push(mem::replace(&mut batch, Batch::empty()));
            }

            batch.parallel.0 = Some(step);
        }
        batch.sequential.push(step);
        batch.sequential_run_if.push(usize::MAX);
        batch.info.regroups.push(regroup.info());

        regroups.push(regroup);
    }

    if !batch.sequential.is_empty() {
        planned.push(batch);
    }
}
