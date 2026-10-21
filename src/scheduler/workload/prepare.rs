//! Plans the regroups a registered workload runs around its systems.

use crate::all_storages::{AllStorages, Grouping};
use crate::borrow::Mutability;
use crate::scheduler::info::{BatchInfo, RegroupInfo, RegroupTrigger, SystemInfo, TypeInfo};
use crate::scheduler::{Batches, RegroupStep, Step};
use crate::storage::StorageId;
use alloc::boxed::Box;
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
/// Each regroup runs as late as possible, so it can take care of more writes,
/// alongside systems that don't borrow from its set when there are some.
/// A batch can be split for this, its systems waiting for the regroup run in the next batch.\
/// The user systems' sequential order doesn't change.
///
/// `batches_info` describes `batches` and gets the same changes.
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
    batches.grouped_writes = systems_writes(&user_batches, grouping);

    let mut planner = Planner::new(grouping);
    for batch in user_batches {
        planner.plan(batch);
    }
    planner.finish();

    put_batches(batches, batches_info, &planner.planned);
    batches.regroups = planner.regroups;
}

/// A step of the workload, with what the planner needs to know about it.
#[derive(Clone)]
struct Job {
    step: Step,
    /// Index of its condition in `Batches::systems_run_if`, `usize::MAX` without one.
    run_if: usize,
    kind: JobKind,
}

#[derive(Clone)]
enum JobKind {
    System {
        info: Box<SystemInfo>,
        /// Position of the system in the workload's sequential order.
        sequential: usize,
    },
    Regroup(RegroupInfo),
}

impl Job {
    /// Returns the storages the job borrows.
    ///
    /// Conditions and accesses through `AllStorages` aren't part of it.
    fn borrow(&self) -> &[TypeInfo] {
        match &self.kind {
            JobKind::System { info, .. } => &info.borrow,
            JobKind::Regroup(info) => &info.borrow,
        }
    }

    /// Returns `true` if the job borrows one of `storages`.
    fn borrows_from(&self, storages: &[TypeInfo]) -> bool {
        self.borrow().iter().any(|type_info| {
            storages
                .iter()
                .any(|storage| storage.storage_id == type_info.storage_id)
        })
    }
}

/// Jobs that can run in parallel, `single` runs on the calling thread.
#[derive(Clone)]
struct Batch {
    single: Option<Job>,
    jobs: Vec<Job>,
}

impl Batch {
    fn new() -> Batch {
        Batch {
            single: None,
            jobs: Vec::new(),
        }
    }

    fn iter(&self) -> impl Iterator<Item = &Job> {
        self.single.iter().chain(&self.jobs)
    }

    fn is_empty(&self) -> bool {
        self.single.is_none() && self.jobs.is_empty()
    }

    /// Returns `true` if `regroup` can run alongside every job of the batch.
    fn can_take(&self, regroup: &RegroupStep) -> bool {
        // A batch only has one slot running on the world's thread.
        if !regroup.set.thread_safe && self.single.is_some() {
            return false;
        }

        // A system borrowing `AllStorages` can reach any storage, it runs alone.
        self.iter().all(|job| {
            !job.borrows_from(&regroup.borrow)
                && job
                    .borrow()
                    .iter()
                    .all(|type_info| type_info.storage_id != TypeId::of::<AllStorages>())
        })
    }

    /// Adds `regroup` to the batch, it has to be able to take it.
    fn add(&mut self, regroups: &mut Vec<RegroupStep>, regroup: RegroupStep) {
        let job = Job {
            step: Step::Regroup(regroups.len()),
            run_if: usize::MAX,
            kind: JobKind::Regroup(regroup.info()),
        };

        if regroup.set.thread_safe {
            self.jobs.push(job);
        } else {
            self.single = Some(job);
        }

        regroups.push(regroup);
    }

    /// Moves the jobs `blocked` returns `true` for to a new batch, keeping their order.
    fn split_off(&mut self, blocked: impl Fn(&Job) -> bool) -> Batch {
        let single = self.single.take_if(|job| blocked(job));
        let (blocked_jobs, free_jobs) = mem::take(&mut self.jobs).into_iter().partition(blocked);
        self.jobs = free_jobs;

        Batch {
            single,
            jobs: blocked_jobs,
        }
    }
}

/// Walks the user batches in order, placing regroups as it goes.
struct Planner<'a> {
    grouping: &'a Grouping,
    /// The batches so far, user and regroup ones.
    planned: Vec<Batch>,
    /// The regroups `Step::Regroup` points to.
    regroups: Vec<RegroupStep>,
    /// Whether each set had its entry regroup.
    entered: Vec<bool>,
    /// Grouped storages a system might have changed since their set's last regroup, sorted.
    pending: Vec<StorageId>,
    /// For each set, the index in `planned` of the last batch with a system borrowing
    /// one of its storages exclusively.
    last_write: Vec<Option<usize>>,
}

impl<'a> Planner<'a> {
    fn new(grouping: &'a Grouping) -> Planner<'a> {
        Planner {
            grouping,
            planned: Vec::new(),
            regroups: Vec::new(),
            entered: vec![false; grouping.sets.len()],
            pending: Vec::new(),
            last_write: vec![None; grouping.sets.len()],
        }
    }

    /// Adds the user batch `batch`, after the regroups it needs.
    fn plan(&mut self, batch: Batch) {
        let due = self.due_regroups(&batch);
        if due.is_empty() {
            self.push_user(batch);

            return;
        }

        // Systems borrowing from a due set wait for its regroup, the others can run alongside it.
        let mut free = batch.clone();
        let blocked = free.split_off(|job| {
            due.iter()
                .any(|(_, regroup)| job.borrows_from(&regroup.borrow))
        });
        let mut own_batches = Vec::new();
        let mut split = false;

        for (set, regroup) in due {
            self.clear(set);

            // The latest spot is alongside the free systems, then the batches since the set's last write.
            if !free.is_empty() && free.can_take(&regroup) {
                free.add(&mut self.regroups, regroup);
                split = true;
            } else if let Some(regroup) = self.join_planned(set, regroup) {
                add_to_own_batch(&mut own_batches, &mut self.regroups, regroup);
            }
        }

        if split {
            self.push_user(free);
            self.planned.extend(own_batches);
            self.push_user(blocked);
        } else {
            // The whole batch waits, its systems keep running together.
            self.planned.extend(own_batches);
            self.push_user(batch);
        }
    }

    /// Adds the regroups of the storages still pending, so the workload leaves every set regrouped.
    fn finish(&mut self) {
        let exit_sets = sets_of(self.grouping, self.pending.clone().into_iter());
        let mut own_batches = Vec::new();

        for set in exit_sets {
            let regroup = RegroupStep::new(
                &self.grouping.sets[set],
                self.grouping,
                RegroupTrigger::Pending,
            );
            self.clear(set);

            if let Some(regroup) = self.join_planned(set, regroup) {
                add_to_own_batch(&mut own_batches, &mut self.regroups, regroup);
            }
        }

        self.planned.extend(own_batches);
    }

    /// Returns the regroups `batch` has to wait for with their set's index, in set order.
    fn due_regroups(&self, batch: &Batch) -> Vec<(usize, RegroupStep)> {
        let borrowed: Vec<StorageId> = batch
            .iter()
            .flat_map(Job::borrow)
            .map(|type_info| type_info.storage_id)
            .collect();

        let mut due = Vec::new();
        for set in sets_of(self.grouping, borrowed.iter().copied()) {
            let members = &self.grouping.sets[set].members;

            // A pending storage has to be regrouped before a system borrows it again.
            // Other members of its set can be used in the meantime, they don't wait for it.
            let trigger = if !self.entered[set] {
                RegroupTrigger::Entry
            } else if borrowed.iter().any(|storage_id| {
                self.pending.binary_search(storage_id).is_ok()
                    && members.binary_search(storage_id).is_ok()
            }) {
                RegroupTrigger::Pending
            } else {
                continue;
            };

            due.push((
                set,
                RegroupStep::new(&self.grouping.sets[set], self.grouping, trigger),
            ));
        }

        due
    }

    /// Adds `regroup` to the latest batch of `planned` able to take it since the set's last write.
    ///
    /// Returns `regroup` when none can.
    fn join_planned(&mut self, set: usize, regroup: RegroupStep) -> Option<RegroupStep> {
        let after_write = self.last_write[set].map_or(0, |index| index + 1);

        match (after_write..self.planned.len())
            .rev()
            .find(|&index| self.planned[index].can_take(&regroup))
        {
            Some(index) => {
                self.planned[index].add(&mut self.regroups, regroup);

                None
            }
            None => Some(regroup),
        }
    }

    /// Adds a batch of user systems, keeping track of what they might write.
    fn push_user(&mut self, batch: Batch) {
        let index = self.planned.len();

        for job in batch.iter() {
            let JobKind::System { info, .. } = &job.kind else {
                continue;
            };

            for storage_id in grouped_writes(info, self.grouping) {
                let set = set_index(self.grouping, storage_id).expect("the storage is grouped");

                if let Err(position) = self.pending.binary_search(&storage_id) {
                    self.pending.insert(position, storage_id);
                }
                self.last_write[set] = Some(index);
            }
        }

        self.planned.push(batch);
    }

    /// Marks `set` regrouped: entered, with none of its storages pending.
    fn clear(&mut self, set: usize) {
        let members = &self.grouping.sets[set].members;

        self.entered[set] = true;
        self.pending
            .retain(|storage_id| members.binary_search(storage_id).is_err());
    }
}

/// Returns the grouped storages `info`'s system borrows exclusively, the ones it leaves pending.
fn grouped_writes(info: &SystemInfo, grouping: &Grouping) -> Vec<StorageId> {
    // We can't know what a system does with an exclusive borrow,
    // even one only updating values could add or remove components.
    info.borrow
        .iter()
        .filter(|type_info| type_info.mutability == Mutability::Exclusive)
        .map(|type_info| type_info.storage_id)
        .filter(|&storage_id| set_index(grouping, storage_id).is_some())
        .collect()
}

/// Returns the grouped storages each system of `user_batches` leaves pending,
/// indexed like the systems list.
fn systems_writes(user_batches: &[Batch], grouping: &Grouping) -> Vec<Vec<StorageId>> {
    let mut writes = Vec::new();

    for job in user_batches.iter().flat_map(Batch::iter) {
        let (Step::System(index), JobKind::System { info, .. }) = (job.step, &job.kind) else {
            continue;
        };

        let storage_ids = grouped_writes(info, grouping);
        if storage_ids.is_empty() {
            continue;
        }

        if writes.len() <= index {
            writes.resize(index + 1, Vec::new());
        }
        writes[index] = storage_ids;
    }

    writes
}

/// Adds `regroup` to the first batch of `own_batches` able to take it, or a new one.
///
/// Sets are disjoint so their regroups can run in parallel,
/// but a batch only has one slot for work that has to run on the world's thread.
fn add_to_own_batch(
    own_batches: &mut Vec<Batch>,
    regroups: &mut Vec<RegroupStep>,
    regroup: RegroupStep,
) {
    match own_batches
        .iter_mut()
        .find(|batch| batch.can_take(&regroup))
    {
        Some(batch) => batch.add(regroups, regroup),
        None => {
            let mut batch = Batch::new();
            batch.add(regroups, regroup);
            own_batches.push(batch);
        }
    }
}

/// Returns a user job's position in the sequential order, regroups go last.
fn sequential_position(job: &Job) -> usize {
    match job.kind {
        JobKind::System { sequential, .. } => sequential,
        JobKind::Regroup(_) => usize::MAX,
    }
}

/// Returns the sorted indexes of the connected sets holding `storage_ids`.
///
/// Storages outside every group are skipped.
fn sets_of(grouping: &Grouping, storage_ids: impl Iterator<Item = StorageId>) -> Vec<usize> {
    let mut sets: Vec<usize> = storage_ids
        .filter_map(|storage_id| set_index(grouping, storage_id))
        .collect();
    sets.sort_unstable();
    sets.dedup();

    sets
}

/// Returns the index of the connected set holding `storage_id`, `None` if it isn't grouped.
fn set_index(grouping: &Grouping, storage_id: StorageId) -> Option<usize> {
    grouping
        .sets
        .iter()
        .position(|set| set.members.binary_search(&storage_id).is_ok())
}

/// Empties `batches` and `batches_info` into a list of batches.
fn take_batches(batches: &mut Batches, batches_info: &mut Vec<BatchInfo>) -> Vec<Batch> {
    let sequential = mem::take(&mut batches.sequential);
    batches.sequential_run_if.clear();

    let parallel = mem::take(&mut batches.parallel);
    let parallel_run_if = mem::take(&mut batches.parallel_run_if);
    let infos = mem::take(batches_info);

    let mut user_batches = Vec::with_capacity(parallel.len());
    // Position in the sequential order of the current batch's first step.
    let mut start = 0;
    for (((single, steps), (single_run_if, steps_run_if)), info) in
        parallel.into_iter().zip(parallel_run_if).zip(infos)
    {
        let (single_info, steps_info) = info.systems;

        // Systems are added to the sequential order as they're placed in batches,
        // so each batch's steps are next to each other.
        // The parallel ones keep their order, the single one can be anywhere among them.
        let len = usize::from(single.is_some()) + steps.len();
        let segment = &sequential[start..start + len];
        let single_offset = single.map(|step| {
            segment
                .iter()
                .position(|&other| other == step)
                .expect("a batch's single step is in its sequential order")
        });

        let single = single
            .zip(single_info)
            .zip(single_offset)
            .map(|((step, info), offset)| Job {
                step,
                run_if: single_run_if,
                kind: JobKind::System {
                    info: Box::new(info),
                    sequential: start + offset,
                },
            });
        let jobs = steps
            .into_iter()
            .zip(steps_run_if)
            .zip(steps_info)
            .enumerate()
            .map(|(index, ((step, run_if), info))| {
                let skips_single = single_offset.is_some_and(|offset| offset <= index);

                Job {
                    step,
                    run_if,
                    kind: JobKind::System {
                        info: Box::new(info),
                        sequential: start + index + usize::from(skips_single),
                    },
                }
            })
            .collect();

        user_batches.push(Batch { single, jobs });
        start += len;
    }

    user_batches
}

/// Puts `planned` back into `batches` and `batches_info`.
fn put_batches(batches: &mut Batches, batches_info: &mut Vec<BatchInfo>, planned: &[Batch]) {
    for batch in planned {
        batches.parallel.push((
            batch.single.as_ref().map(|job| job.step),
            batch.jobs.iter().map(|job| job.step).collect(),
        ));
        batches.parallel_run_if.push((
            batch.single.as_ref().map_or(usize::MAX, |job| job.run_if),
            batch.jobs.iter().map(|job| job.run_if).collect(),
        ));

        let system_info = |job: &Job| match &job.kind {
            JobKind::System { info, .. } => Some(SystemInfo::clone(info)),
            JobKind::Regroup(_) => None,
        };
        batches_info.push(BatchInfo {
            systems: (
                batch.single.as_ref().and_then(system_info),
                batch.jobs.iter().filter_map(system_info).collect(),
            ),
            regroups: batch
                .iter()
                .filter_map(|job| match &job.kind {
                    JobKind::Regroup(info) => Some(info.clone()),
                    JobKind::System { .. } => None,
                })
                .collect(),
        });
    }

    for (step, run_if) in sequential_order(planned) {
        batches.sequential.push(step);
        batches.sequential_run_if.push(run_if);
    }
}

/// Returns every step of `planned` and its condition, in sequential order.
///
/// User systems keep the workload's sequential order.\
/// Each regroup goes right before the first system after it in `planned` borrowing from its set,
/// that's the first one needing it. The ones no system needs go last.
fn sequential_order(planned: &[Batch]) -> Vec<(Step, usize)> {
    let mut systems: Vec<&Job> = planned
        .iter()
        .flat_map(Batch::iter)
        .filter(|job| matches!(job.kind, JobKind::System { .. }))
        .collect();
    systems.sort_by_key(|job| sequential_position(job));

    // Each regroup with the sequential position of the first system needing it.
    let mut regroups: Vec<(Option<usize>, Step)> = Vec::new();
    for (index, batch) in planned.iter().enumerate() {
        for job in batch.iter() {
            let JobKind::Regroup(info) = &job.kind else {
                continue;
            };

            let first_need = planned[index + 1..]
                .iter()
                .flat_map(Batch::iter)
                .filter(|later| {
                    matches!(later.kind, JobKind::System { .. }) && later.borrows_from(&info.borrow)
                })
                .map(sequential_position)
                .min();
            regroups.push((first_need, job.step));
        }
    }

    let regroups_before = |position: Option<usize>| {
        regroups
            .iter()
            .filter(move |(first_need, _)| *first_need == position)
            .map(|&(_, step)| (step, usize::MAX))
    };

    let mut order = Vec::new();
    for system in systems {
        order.extend(regroups_before(Some(sequential_position(system))));
        order.push((system.step, system.run_if));
    }
    order.extend(regroups_before(None));

    order
}
