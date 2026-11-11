use crate::ShipHashMap;
use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::error;
use crate::scheduler::info::{
    BatchInfo, BeforeAfterConstraint, Conflict, DedupedLabels, SystemInfo,
};
use crate::scheduler::{
    Batches, Label, Step, TypeId, TypeInfo, Workload, WorkloadInfo, WorkloadSystem,
};
use crate::world::World;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
// use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::hash::Hash;

#[allow(clippy::type_complexity)]
struct ToBePlacedSystem {
    /// Index of the callable in the scheduler, shared by all occurrences of a callable.
    index: usize,
    /// Position of the system in the workload's list, its identity while we schedule it.
    ///
    /// Ordering edges (`soft_after`, `hard_after`, ...) point to occurrences.
    occurrence: usize,
    display_name: Box<dyn Label>,
    /// What the system borrows, without its conditions.
    ///
    /// Workload info shows it and regroups are triggered by it.
    borrow_constraints: Vec<TypeInfo>,
    /// What the system's conditions borrow.
    conditions_borrow: Vec<TypeInfo>,
    /// What the system and its conditions borrow, see [`footprint`].
    ///
    /// They're scheduled as one unit, conflicts and the calling-thread slot are decided with it.
    footprint: Vec<TypeInfo>,
    tags: Vec<Box<dyn Label>>,
    before_all: DedupedLabels,
    before_all_info: Vec<BeforeAfterConstraint>,
    after_all: DedupedLabels,
    after_all_info: Vec<BeforeAfterConstraint>,
    /// Copy of `hard_after` to be returned in `SystemInfo`.
    after_info: DedupedUniqueIds,
    // System must be after in the sequential order
    // System can run in parallel in the parallel order
    soft_after: DedupedUniqueIds,
    // System must be after in both orders
    hard_after: DedupedUniqueIds,
    // System must be before in both orders
    hard_before: DedupedLabels,
    require_in_workload: DedupedLabels,
    require_before: DedupedLabels,
    require_after: DedupedLabels,
    run_if: Option<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync + 'static>>,
    confict: Option<Conflict>,
}

#[derive(Clone)]
struct DedupedUniqueIds(Vec<usize>);

impl DedupedUniqueIds {
    fn new() -> DedupedUniqueIds {
        DedupedUniqueIds(Vec::new())
    }

    /// Returns `true` if the system was already present.
    fn add(&mut self, id: usize) -> bool {
        match self.0.binary_search(&id) {
            Ok(_) => true,
            Err(index) => {
                self.0.insert(index, id);
                false
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn remove(&mut self, id: usize) {
        if let Ok(index) = self.0.binary_search(&id) {
            self.0.remove(index);
        }
    }

    fn retain(&mut self, mut keep: impl FnMut(usize) -> bool) {
        self.0.retain(|&id| keep(id));
    }

    fn contains(&self, system_index: usize) -> bool {
        self.0.binary_search(&system_index).is_ok()
    }
}

/// Registers `builder`'s new callables and splits its systems into batches.
///
/// The workload isn't published, the caller decides where the returned batches go.
#[allow(clippy::type_complexity)]
pub(super) fn create_workload(
    mut builder: Workload,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: &mut Vec<Box<dyn Label>>,
    system_generators: &mut Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    lookup_table: &mut ShipHashMap<TypeId, usize>,
    tracking_to_enable: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
) -> Result<(Batches, WorkloadInfo), error::AddWorkload> {
    let mut to_be_placed_systems = insert_systems_in_scheduler(
        &mut builder,
        systems,
        system_names,
        system_generators,
        lookup_table,
        tracking_to_enable,
    );

    if let Err(err) = check_require_in_workload(&mut to_be_placed_systems) {
        return Err(err);
    }

    let Workload {
        name: workload_name,
        run_if: workload_run_if,
        barriers,
        // Systems were emptied by insert_systems_in_scheduler
        systems: _,
        // This workload will not be ordered with anything else
        tags: _,
        before_all: _,
        after_all: _,
        overwritten_name: _,
        require_before: _,
        require_after: _,
    } = builder;

    propagate_barriers(&mut to_be_placed_systems, barriers);
    remove_absent_labels(&mut to_be_placed_systems);
    propagate_implicit_hard_ordering(&mut to_be_placed_systems);
    propagate_implicit_soft_ordering(&mut to_be_placed_systems);
    map_before_all_to_after(&mut to_be_placed_systems);
    map_after_all_to_after(&mut to_be_placed_systems);
    map_before_to_after(&mut to_be_placed_systems);

    if let Err(err) = check_require_before(&mut to_be_placed_systems) {
        return Err(err);
    }
    if let Err(err) = check_require_after(&mut to_be_placed_systems) {
        return Err(err);
    }

    // The top-level condition isn't propagated to the systems, so we collect its tracking here.
    if let Some(workload_run_if) = &workload_run_if {
        tracking_to_enable.extend(workload_run_if.tracking_to_enable.iter().copied());
    }

    let mut batches = Batches {
        // Systems are still in occurrence order, `order_systems` moves them.
        conditions_borrow: to_be_placed_systems
            .iter_mut()
            .map(|system| core::mem::take(&mut system.conditions_borrow))
            .collect(),
        workload_run_if: workload_run_if.map(|workload_run_if| workload_run_if.system_fn),
        ..Default::default()
    };

    let batches_info = order_systems(&mut to_be_placed_systems, &mut batches)?;
    let workload_info = WorkloadInfo {
        name: format!("{:?}", workload_name),
        batches_info,
    };

    Ok((batches, workload_info))
}

fn check_require_in_workload(
    to_be_placed_systems: &mut [ToBePlacedSystem],
) -> Result<(), error::AddWorkload> {
    'outer: for i in 0..to_be_placed_systems.len() {
        let mut require_in_workload =
            core::mem::take(&mut to_be_placed_systems[i].require_in_workload);

        for system in &*to_be_placed_systems {
            require_in_workload.retain(|required| system.tags.iter().all(|tag| tag != required));

            if require_in_workload.is_empty() {
                continue 'outer;
            }
        }

        if !require_in_workload.is_empty() {
            return Err(error::AddWorkload::MissingInWorkload(
                to_be_placed_systems[i].display_name.clone(),
                require_in_workload.into_iter().collect(),
            ));
        }
    }

    Ok(())
}

fn check_require_before(
    to_be_placed_systems: &mut [ToBePlacedSystem],
) -> Result<(), error::AddWorkload> {
    fn recursive_add_systems_before(
        systems_before: &mut DedupedUniqueIds,
        to_be_placed_systems: &[ToBePlacedSystem],
        system_index: usize,
    ) {
        if systems_before.add(system_index) {
            return;
        }

        let system = &to_be_placed_systems[system_index];
        for &system_index in &system.hard_after.0 {
            recursive_add_systems_before(systems_before, to_be_placed_systems, system_index);
        }
    }

    'outer: for i in 0..to_be_placed_systems.len() {
        let mut require_before = core::mem::take(&mut to_be_placed_systems[i].require_before);
        // Without requirements there is nothing to check, we skip walking its predecessors.
        // Every conflict is a hard edge, so that walk can cover most of the workload.
        if require_before.is_empty() {
            continue;
        }

        let mut systems_before = DedupedUniqueIds::new();

        for &system_index in &to_be_placed_systems[i].hard_after.0 {
            recursive_add_systems_before(&mut systems_before, to_be_placed_systems, system_index);
        }

        for system_before in systems_before.0 {
            let system = &to_be_placed_systems[system_before];

            require_before.retain(|required| system.tags.iter().all(|tag| tag != required));

            if require_before.is_empty() {
                continue 'outer;
            }
        }

        if !require_before.is_empty() {
            return Err(error::AddWorkload::MissingBefore(
                to_be_placed_systems[i].display_name.clone(),
                require_before.into_iter().collect(),
            ));
        }
    }

    Ok(())
}

fn check_require_after(
    to_be_placed_systems: &mut [ToBePlacedSystem],
) -> Result<(), error::AddWorkload> {
    fn recursive_add_systems_after(
        systems_after: &mut DedupedUniqueIds,
        to_be_placed_systems: &[ToBePlacedSystem],
        system_index: usize,
    ) {
        if systems_after.add(system_index) {
            return;
        }

        for i in 0..to_be_placed_systems.len() {
            if system_index == i {
                continue;
            }

            let system = &to_be_placed_systems[i];
            if system.hard_after.contains(system_index) {
                recursive_add_systems_after(systems_after, to_be_placed_systems, i);
            }
        }
    }

    'outer: for i in 0..to_be_placed_systems.len() {
        let mut require_after = core::mem::take(&mut to_be_placed_systems[i].require_after);
        // Without requirements there is nothing to check, we skip walking its successors.
        if require_after.is_empty() {
            continue;
        }

        let mut systems_after = DedupedUniqueIds::new();

        for j in 0..to_be_placed_systems.len() {
            if i == j {
                continue;
            }

            let system = &to_be_placed_systems[j];

            if system.hard_after.contains(i) {
                recursive_add_systems_after(&mut systems_after, to_be_placed_systems, i);
            }
        }

        for system_after in systems_after.0 {
            let system = &to_be_placed_systems[system_after];

            require_after.retain(|required| system.tags.iter().all(|tag| tag != required));

            if require_after.is_empty() {
                continue 'outer;
            }
        }

        if !require_after.is_empty() {
            return Err(error::AddWorkload::MissingAfter(
                to_be_placed_systems[i].display_name.clone(),
                require_after.into_iter().collect(),
            ));
        }
    }

    Ok(())
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn insert_systems_in_scheduler(
    builder: &mut Workload,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: &mut Vec<Box<dyn Label>>,
    system_generators: &mut Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    lookup_table: &mut ShipHashMap<TypeId, usize>,
    all_tracking_to_enable: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
) -> Vec<ToBePlacedSystem> {
    // An occurrence is identified by its position in the final list.
    // We don't use `WorkloadSystem::unique_id`, only `with_system` keeps it in sync with positions.
    builder
        .systems
        .drain(..)
        .enumerate()
        .map(
            |(
                occurrence,
                WorkloadSystem {
                    type_id,
                    display_name,
                    system_fn,
                    borrow_constraints,
                    run_if_borrow_constraints,
                    mut tracking_to_enable,
                    generator,
                    run_if,
                    mut tags,
                    before_all,
                    after_all,
                    after,
                    before,
                    unique_id: _,
                    require_in_workload,
                    require_before,
                    require_after,
                },
            )| {
                let system_index = *lookup_table.entry(type_id).or_insert_with(|| {
                    systems.push(system_fn);
                    system_names.push(display_name.clone());
                    system_generators.push(generator);

                    systems.len() - 1
                });

                all_tracking_to_enable.append(&mut tracking_to_enable);

                let mut hard_after = DedupedLabels::new();
                hard_after.extend(after.into_iter().map(|id| {
                    let id: Box<dyn Label> = Box::new(UniqueSystemId(id));

                    id
                }));
                let mut hard_before = DedupedLabels::new();
                hard_before.extend(before.into_iter().map(|id| {
                    let id: Box<dyn Label> = Box::new(UniqueSystemId(id));

                    id
                }));

                tags.push(Box::new(UniqueSystemId(occurrence)));

                ToBePlacedSystem {
                    index: system_index,
                    occurrence,
                    display_name,
                    footprint: footprint(&borrow_constraints, &run_if_borrow_constraints),
                    borrow_constraints,
                    conditions_borrow: run_if_borrow_constraints,
                    tags,
                    before_all,
                    after_all_info: Vec::new(),
                    after_all,
                    before_all_info: Vec::new(),
                    after_info: DedupedUniqueIds::new(),
                    hard_after: DedupedUniqueIds::new(),
                    hard_before,
                    soft_after: DedupedUniqueIds::new(),
                    require_in_workload,
                    require_before,
                    require_after,
                    run_if,
                    confict: None,
                }
            },
        )
        .collect()
}

/// Returns what an occurrence borrows when it runs: what its system and its conditions borrow.
///
/// A storage borrowed several times is listed once, exclusive if one of the borrows is,
/// not thread safe if one of them isn't.
/// Every condition of a chain counts, even the ones a previous condition can skip.
fn footprint(system: &[TypeInfo], conditions: &[TypeInfo]) -> Vec<TypeInfo> {
    let mut footprint: Vec<TypeInfo> = Vec::with_capacity(system.len() + conditions.len());

    for type_info in system.iter().chain(conditions) {
        let Some(listed) = footprint
            .iter_mut()
            .find(|listed| listed.storage_id == type_info.storage_id)
        else {
            footprint.push(type_info.clone());

            continue;
        };

        if type_info.mutability == Mutability::Exclusive {
            listed.mutability = Mutability::Exclusive;
        }
        listed.thread_safe &= type_info.thread_safe;
    }

    footprint
}

/// Tags an occurrence with its position, so the numeric `before` constraints can find it.
///
/// The system name or TypeId cannot be used since systems can be present
/// multiple times in a workload.
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
struct UniqueSystemId(usize);

impl Label for UniqueSystemId {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn dyn_eq(&self, other: &dyn Label) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }

    fn dyn_hash(&self, mut state: &mut dyn core::hash::Hasher) {
        Self::hash(self, &mut state);
    }

    fn dyn_clone(&self) -> Box<dyn Label> {
        Box::new(*self)
    }

    fn dyn_debug(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        self.fmt(f)
    }
}

/// Apply barriers as systems' tags.
fn propagate_barriers(to_be_placed_systems: &mut [ToBePlacedSystem], barriers: Vec<usize>) {
    for index in barriers {
        for system in &mut to_be_placed_systems[index..] {
            for i in 0..index {
                system.hard_after.add(i);
            }
        }
    }
}

/// Removes the `before_all` and `after_all` labels no other system of the workload has.
///
/// These labels can't order anything, a system with only those keeps its implicit ordering.
fn remove_absent_labels(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in 0..to_be_placed_systems.len() {
        let mut before_all = core::mem::take(&mut to_be_placed_systems[i].before_all);
        let mut after_all = core::mem::take(&mut to_be_placed_systems[i].after_all);

        let is_present = |label: &dyn Label| {
            to_be_placed_systems
                .iter()
                .enumerate()
                .any(|(j, other_system)| {
                    j != i && other_system.tags.iter().any(|tag| **tag == *label)
                })
        };
        before_all.retain(|label| is_present(&**label));
        after_all.retain(|label| is_present(&**label));

        to_be_placed_systems[i].before_all = before_all;
        to_be_placed_systems[i].after_all = after_all;
    }
}

/// Translate the implicit ordering's rules to tags and after constraints.
fn propagate_implicit_hard_ordering(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in (1..to_be_placed_systems.len()).rev() {
        let system = &to_be_placed_systems[i];
        if !system.before_all.is_empty() || !system.after_all.is_empty() {
            // system has opted out of the implicit ordering
            // and shouldn't be considered here

            continue;
        }

        // Every earlier conflicting system has to run before this one, not only the nearest.
        // Requirement checks follow hard edges, so they need all of them.
        // The nearest conflict stays the one we export as explanation.
        let mut conflicts = Vec::new();
        for j in (0..i).rev() {
            let other_system = &to_be_placed_systems[j];
            if !other_system.before_all.is_empty() || !other_system.after_all.is_empty() {
                // other_system has opted out of the implicit ordering
                // and shouldn't be considered here

                continue;
            }

            if let Some(conflict) =
                check_conflict(system, other_system.index, &other_system.footprint)
            {
                conflicts.push((j, conflict));
            }
        }

        let system = &mut to_be_placed_systems[i];
        for &(j, _) in &conflicts {
            system.hard_after.add(j);
        }
        if let Some((_, nearest)) = conflicts.into_iter().next() {
            system.confict = Some(nearest);
        }
    }
}

/// Translate the implicit ordering's rules to tags and after constraints.
fn propagate_implicit_soft_ordering(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in (1..to_be_placed_systems.len()).rev() {
        let system = &to_be_placed_systems[i];
        if !system.before_all.is_empty() || !system.after_all.is_empty() {
            // system has opted out of the implicit ordering
            // and shouldn't be considered here

            continue;
        }

        for j in (0..i).rev() {
            let other_system = &to_be_placed_systems[j];
            if !other_system.before_all.is_empty() || !other_system.after_all.is_empty() {
                // other_system has opted out of the implicit ordering
                // and shouldn't be considered here

                continue;
            }

            to_be_placed_systems[i].soft_after.add(j);

            break;
        }
    }
}

/// Map the before_all list to after.
///
/// The goal is to only have constraints in a single form: after.
fn map_before_all_to_after(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in 0..to_be_placed_systems.len() {
        let before_all = core::mem::take(&mut to_be_placed_systems[i].before_all);
        let mut before_all_info = Vec::new();

        for (j, other_system) in to_be_placed_systems.iter_mut().enumerate() {
            if i == j {
                continue;
            }

            if let Some(tag) = other_system
                .tags
                .iter()
                .find(|tag| before_all.iter().any(|label| tag == &label))
            {
                other_system.hard_after.add(i);
                before_all_info.push(BeforeAfterConstraint {
                    other_system: other_system.index,
                    constraint: format!("{:?}", tag),
                })
            }
        }

        to_be_placed_systems[i].before_all_info = before_all_info;
    }
}

/// Map the after_all list to after.
///
/// The goal is to only have constraints in a single form: after.
fn map_after_all_to_after(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in 0..to_be_placed_systems.len() {
        let after_all = core::mem::take(&mut to_be_placed_systems[i].after_all);
        let mut after_all_info = Vec::new();

        for j in 0..to_be_placed_systems.len() {
            if i == j {
                continue;
            }

            for label in after_all.iter() {
                if to_be_placed_systems[j].tags.iter().any(|tag| tag == label) {
                    to_be_placed_systems[i].hard_after.add(j);

                    after_all_info.push(BeforeAfterConstraint {
                        other_system: j,
                        constraint: format!("{:?}", label),
                    });

                    break;
                }
            }
        }

        to_be_placed_systems[i].after_all_info = after_all_info;
    }
}

/// Map the before list to after.
///
/// The goal is to only have constraints in a single form: after.
fn map_before_to_after(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in 0..to_be_placed_systems.len() {
        let before = core::mem::take(&mut to_be_placed_systems[i].hard_before);

        for (j, other_system) in to_be_placed_systems.iter_mut().enumerate() {
            if i == j {
                continue;
            }

            if other_system
                .tags
                .iter()
                .any(|tag| before.iter().any(|label| tag == label))
            {
                other_system.hard_after.add(i);

                break;
            }
        }
    }
}

/// Move systems around until all constraints are satisfied.
///
/// Use Kahn's algorithm to find a topological ordering, one batch at a time.
fn order_systems(
    to_be_placed_systems: &mut Vec<ToBePlacedSystem>,
    batches: &mut Batches,
) -> Result<Vec<BatchInfo>, error::AddWorkload> {
    for system in &mut *to_be_placed_systems {
        system.after_info = system.hard_after.clone();
    }

    // Systems free of constraints can go in the batch being filled.
    // That can be the first system in the workload or systems
    // that do not borrow any component.
    let mut ready = VecDeque::new();
    ready.extend(to_be_placed_systems.extract_if(.., |system| {
        system.hard_after.is_empty() && system.soft_after.is_empty()
    }));

    let mut batches_info = Vec::new();
    while !ready.is_empty() {
        let placed = fill_batch(&mut ready, to_be_placed_systems, batches, &mut batches_info);

        // The batch is closed, the systems waiting on its systems can go in the next one.
        // They come after the systems that didn't fit in the batch.
        ready.extend(to_be_placed_systems.extract_if(.., |other_system| {
            other_system
                .hard_after
                .retain(|occurrence| !placed.contains(&occurrence));

            other_system.hard_after.is_empty() && other_system.soft_after.is_empty()
        }));
    }

    if !to_be_placed_systems.is_empty() {
        return Err(error::AddWorkload::ImpossibleRequirements(
            error::ImpossibleRequirements::Cycle(
                to_be_placed_systems
                    .drain(..)
                    .map(|system| system.display_name)
                    .collect(),
            ),
        ));
    }

    Ok(batches_info)
}

/// Opens a batch and places every system of `ready` that fits in it, in order.
///
/// Placing a system frees the systems only waiting on it in the sequential order,
/// they're tried for the same batch.\
/// The systems that conflict with the batch are left in `ready`, in order.
///
/// Returns the occurrences placed in the batch.
fn fill_batch(
    ready: &mut VecDeque<ToBePlacedSystem>,
    to_be_placed_systems: &mut Vec<ToBePlacedSystem>,
    batches: &mut Batches,
    batches_info: &mut Vec<BatchInfo>,
) -> Vec<usize> {
    batches.parallel.push((None, Vec::new()));
    batches.parallel_run_if.push((usize::MAX, Vec::new()));
    batches.parallel_occurrences.push((usize::MAX, Vec::new()));
    batches_info.push(BatchInfo {
        systems: (None, Vec::new()),
        regroups: Vec::new(),
    });

    let mut footprints = BatchFootprints::default();
    let mut conflicting = VecDeque::new();
    let mut placed = Vec::new();
    while let Some(mut system) = ready.pop_front() {
        // A system can't join a batch that already conflicts with it, the batch only grows.
        // So we set it aside for the next batch and keep trying the others.
        if let Some(conflict) = check_can_go_in_parallel_batch(&footprints, &system) {
            system.confict = Some(conflict);
            conflicting.push_back(system);

            continue;
        }

        ready.extend(to_be_placed_systems.extract_if(.., |other_system| {
            other_system.soft_after.remove(system.occurrence);

            other_system.hard_after.is_empty() && other_system.soft_after.is_empty()
        }));

        placed.push(system.occurrence);
        place_system(system, &mut footprints, batches, batches_info);
    }

    *ready = conflicting;

    placed
}

/// Adds `system` to the last batch, and after every placed system in the sequential order.
fn place_system(
    system: ToBePlacedSystem,
    footprints: &mut BatchFootprints,
    batches: &mut Batches,
    batches_info: &mut [BatchInfo],
) {
    let run_if_index = if let Some(run_if) = system.run_if {
        batches.systems_run_if.push(run_if);

        batches.systems_run_if.len() - 1
    } else {
        usize::MAX
    };

    batches.sequential.push(Step::System(system.index));
    batches.sequential_occurrences.push(system.occurrence);
    batches.sequential_run_if.push(run_if_index);

    let info = SystemInfo {
        name: format!("{:?}", system.display_name),
        borrow: system.borrow_constraints,
        conflict: system.confict,
        after: system.after_info.0,
        after_all: system.after_all_info,
        before_all: system.before_all_info,
        unique_id: system.index,
    };

    let batch = batches.parallel.last_mut().unwrap();
    let batch_run_if = batches.parallel_run_if.last_mut().unwrap();
    let batch_occurrences = batches.parallel_occurrences.last_mut().unwrap();
    let batch_info = batches_info.last_mut().unwrap();

    // A condition needing the calling thread takes its system along, they run together.
    let is_single_system = system.footprint.iter().any(|constraint| {
        !constraint.thread_safe || constraint.storage_id == TypeId::of::<AllStorages>()
    });

    if is_single_system {
        footprints.single = Some((system.index, system.footprint));
        batch.0 = Some(Step::System(system.index));
        batch_run_if.0 = run_if_index;
        batch_occurrences.0 = system.occurrence;
        batch_info.systems.0 = Some(info);
    } else {
        footprints.others.push((system.index, system.footprint));
        batch.1.push(Step::System(system.index));
        batch_run_if.1.push(run_if_index);
        batch_occurrences.1.push(system.occurrence);
        batch_info.systems.1.push(info);
    }
}

/// The footprints of the systems in the batch being filled, with their callable index.
///
/// Unlike the batch's info, they include what the systems' conditions borrow.
#[derive(Default)]
struct BatchFootprints {
    /// The system in the calling-thread slot.
    single: Option<(usize, Vec<TypeInfo>)>,
    others: Vec<(usize, Vec<TypeInfo>)>,
}

fn check_can_go_in_parallel_batch(
    batch: &BatchFootprints,
    tested_system: &ToBePlacedSystem,
) -> Option<Conflict> {
    batch
        .single
        .iter()
        .chain(&batch.others)
        .find_map(|(index, footprint)| check_conflict(tested_system, *index, footprint))
}

fn check_conflict(
    tested_system: &ToBePlacedSystem,
    other_system_id: usize,
    borrow_constraints: &[TypeInfo],
) -> Option<Conflict> {
    for type_info in &tested_system.footprint {
        for other_type_info in borrow_constraints {
            if !type_info.thread_safe && !other_type_info.thread_safe {
                return Some(Conflict::OtherNotSendSync {
                    system: other_system_id,
                    type_info: other_type_info.clone(),
                });
            }

            let identical_storage = type_info.storage_id == other_type_info.storage_id;
            let either_storage_exclusive = type_info.mutability == Mutability::Exclusive
                || other_type_info.mutability == Mutability::Exclusive;

            if (identical_storage && either_storage_exclusive)
                || type_info.storage_id == TypeId::of::<AllStorages>()
                || other_type_info.storage_id == TypeId::of::<AllStorages>()
            {
                return Some(Conflict::Borrow {
                    type_info: type_info.clone(),
                    other_system: other_system_id,
                    other_type_info: other_type_info.clone(),
                });
            }
        }
    }

    None
}

// Tests are located in workload.rs
