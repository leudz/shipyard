use crate::ShipHashMap;
use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::error;
use crate::scheduler::info::{
    BatchInfo, BeforeAfterConstraint, Conflict, DedupedLabels, OccurrenceRef, OrderingOrigin,
    Predecessor, SystemInfo, SystemOccurrence,
};
use crate::scheduler::label::SequentialLabel;
use crate::scheduler::{
    Batches, Label, Step, TypeId, TypeInfo, Workload, WorkloadInfo, WorkloadSystem,
};
use crate::world::World;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::vec;
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
    /// Ordering edges (`order_after`, `batch_after`, ...) point to occurrences.
    occurrence: usize,
    display_name: Box<dyn Label>,
    /// What the system borrows, without its conditions.
    ///
    /// Workload info shows it and regroups are triggered by it.
    borrow_constraints: Vec<TypeInfo>,
    /// What the system's conditions borrow, workload info shows it.
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
    /// Copy of `batch_after` to be returned in `SystemInfo`.
    after_info: DedupedUniqueIds,
    /// The systems placed before this one, it can share their batch.
    ///
    /// They keep the declaration order of systems that don't conflict.
    order_after: DedupedUniqueIds,
    /// The systems in an earlier batch than this one.
    batch_after: DedupedUniqueIds,
    /// Why each `batch_after` edge exists, with the predecessor's occurrence.
    ///
    /// An edge can have several origins, `batch_after` only keeps the edge once.
    origins: Vec<(usize, OrderingOrigin)>,
    // System must be before in both orders
    hard_before: DedupedLabels,
    require_in_workload: DedupedLabels,
    require_before: DedupedLabels,
    require_after: DedupedLabels,
    run_if: Option<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync + 'static>>,
    /// Why the system can't join the previous batch, with the occurrence the conflict is with.
    confict: Option<(usize, Conflict)>,
}

#[derive(Clone, Default)]
pub(super) struct DedupedUniqueIds(Vec<usize>);

impl DedupedUniqueIds {
    pub(super) fn new() -> DedupedUniqueIds {
        DedupedUniqueIds(Vec::new())
    }

    /// Returns the ids, sorted.
    pub(super) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().copied()
    }

    /// Returns `true` if the system was already present.
    pub(super) fn add(&mut self, id: usize) -> bool {
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
/// The workload isn't published, the caller decides where the returned batches go.\
/// The returned [`Dependencies`] are what [`prepare_regroups`](super::prepare::prepare_regroups)
/// needs to add regroups to the batches.
#[allow(clippy::type_complexity)]
pub(super) fn create_workload(
    mut builder: Workload,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: &mut Vec<Box<dyn Label>>,
    system_generators: &mut Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    lookup_table: &mut ShipHashMap<TypeId, usize>,
    tracking_to_enable: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
) -> Result<(Batches, WorkloadInfo, Dependencies), error::AddWorkload> {
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
    propagate_implicit_batch_after(&mut to_be_placed_systems);
    propagate_implicit_order_after(&mut to_be_placed_systems);
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
        workload_run_if: workload_run_if.map(|workload_run_if| workload_run_if.system_fn),
        ..Default::default()
    };

    // Systems are still in occurrence order, so are the nodes.
    let mut dependencies = Dependencies {
        nodes: take_nodes(&mut to_be_placed_systems),
        borrows: to_be_placed_systems
            .iter()
            .map(|system| system.borrow_constraints.clone())
            .collect(),
        conflicts: vec![None; to_be_placed_systems.len()],
    };

    let batches_info = order_systems(
        to_be_placed_systems,
        dependencies.nodes.clone(),
        &mut dependencies.conflicts,
        &mut batches,
    )?;
    let workload_info = WorkloadInfo {
        name: format!("{:?}", workload_name),
        batches_info,
    };

    Ok((batches, workload_info, dependencies))
}

/// What placing regroups among a workload's systems needs to know about them, indexed by occurrence.
pub(super) struct Dependencies {
    /// Each occurrence's node, with all its edges.
    pub(super) nodes: Vec<Node>,
    /// What each occurrence's system borrows, without its conditions. Regroups are triggered by it.
    pub(super) borrows: Vec<Vec<TypeInfo>>,
    /// The occurrence each occurrence's exported conflict is with, `None` without one.
    ///
    /// The exported conflict only has the other system's callable index, shared by all its occurrences.
    pub(super) conflicts: Vec<Option<usize>>,
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
        for &system_index in &system.batch_after.0 {
            recursive_add_systems_before(systems_before, to_be_placed_systems, system_index);
        }
    }

    'outer: for i in 0..to_be_placed_systems.len() {
        let mut require_before = core::mem::take(&mut to_be_placed_systems[i].require_before);
        // Without requirements there is nothing to check, we skip walking its predecessors.
        // Every conflict is a `batch_after` edge, so that walk can cover most of the workload.
        if require_before.is_empty() {
            continue;
        }

        let mut systems_before = DedupedUniqueIds::new();

        for &system_index in &to_be_placed_systems[i].batch_after.0 {
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
            if system.batch_after.contains(system_index) {
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

            if system.batch_after.contains(i) {
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
                    batch_after: DedupedUniqueIds::new(),
                    origins: Vec::new(),
                    hard_before,
                    order_after: DedupedUniqueIds::new(),
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
                system.batch_after.add(i);
                system.origins.push((i, OrderingOrigin::Barrier));
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

/// Puts each system in a later batch than the systems declared before it that it conflicts with.
fn propagate_implicit_batch_after(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in (1..to_be_placed_systems.len()).rev() {
        let system = &to_be_placed_systems[i];
        if !system.before_all.is_empty() || !system.after_all.is_empty() {
            // system has opted out of the implicit ordering
            // and shouldn't be considered here

            continue;
        }

        // Every earlier conflicting system has to run before this one, not only the nearest.
        // Requirement checks follow `batch_after` edges, so they need all of them.
        // The nearest conflict stays the one we export as explanation.
        let mut conflicts = Vec::new();
        for j in (0..i).rev() {
            let other_system = &to_be_placed_systems[j];
            if !other_system.before_all.is_empty() || !other_system.after_all.is_empty() {
                // other_system has opted out of the implicit ordering
                // and shouldn't be considered here

                continue;
            }

            if let Some(conflict) = check_conflict(
                &system.footprint,
                other_system.index,
                &other_system.footprint,
            ) {
                conflicts.push((j, conflict));
            }
        }

        let system = &mut to_be_placed_systems[i];
        for &(j, _) in &conflicts {
            system.batch_after.add(j);
            system.origins.push((j, OrderingOrigin::Implicit));
        }
        if let Some(nearest) = conflicts.into_iter().next() {
            system.confict = Some(nearest);
        }
    }
}

/// Places each system after the one declared before it, they can share a batch.
fn propagate_implicit_order_after(to_be_placed_systems: &mut [ToBePlacedSystem]) {
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

            to_be_placed_systems[i].order_after.add(j);

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

            let matching_tags: Vec<&Box<dyn Label>> = other_system
                .tags
                .iter()
                .filter(|tag| before_all.iter().any(|label| tag == &label))
                .collect();
            let Some(first_tag) = matching_tags.first() else {
                continue;
            };

            // The legacy info only names the first tag, the origins keep all of them.
            let constraint = format!("{:?}", first_tag);
            let origins: Vec<OrderingOrigin> = matching_tags
                .iter()
                .map(|tag| before_all_origin(&***tag))
                .collect();

            other_system.batch_after.add(i);
            other_system
                .origins
                .extend(origins.into_iter().map(|origin| (i, origin)));
            before_all_info.push(BeforeAfterConstraint {
                other_system: other_system.index,
                constraint,
            })
        }

        to_be_placed_systems[i].before_all_info = before_all_info;
    }
}

/// Returns why a `before_all` matching `tag` orders two systems.
///
/// Sequential workloads link their children with private labels, they aren't user labels.
fn before_all_origin(tag: &dyn Label) -> OrderingOrigin {
    if tag.as_any().is::<SequentialLabel>() {
        OrderingOrigin::Sequential
    } else {
        OrderingOrigin::BeforeAll(format!("{:?}", tag))
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

            let matching_labels: Vec<&Box<dyn Label>> = after_all
                .iter()
                .filter(|label| to_be_placed_systems[j].tags.iter().any(|tag| tag == *label))
                .collect();
            let Some(first_label) = matching_labels.first() else {
                continue;
            };

            // The legacy info only names the first label, the origins keep all of them.
            after_all_info.push(BeforeAfterConstraint {
                other_system: j,
                constraint: format!("{:?}", first_label),
            });
            let system = &mut to_be_placed_systems[i];
            system.batch_after.add(j);
            for label in matching_labels {
                system
                    .origins
                    .push((j, OrderingOrigin::AfterAll(format!("{:?}", label))));
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
                other_system.batch_after.add(i);
                other_system.origins.push((i, OrderingOrigin::Before));

                break;
            }
        }
    }
}

/// Moves the systems' edges into the nodes [`schedule`] places, indexed by occurrence.
///
/// The systems keep a copy of their `batch_after` edges for their info.
fn take_nodes(to_be_placed_systems: &mut [ToBePlacedSystem]) -> Vec<Node> {
    to_be_placed_systems
        .iter_mut()
        .map(|system| {
            system.after_info = system.batch_after.clone();

            Node {
                callable: system.index,
                footprint: system.footprint.clone(),
                batch_after: core::mem::take(&mut system.batch_after),
                order_after: core::mem::take(&mut system.order_after),
            }
        })
        .collect()
}

/// Splits the systems into batches following `nodes`, their nodes indexed by occurrence.
///
/// The sequential order is the order the systems are placed in.
/// `conflicts` gets the occurrence each system's conflict is with.
fn order_systems(
    to_be_placed_systems: Vec<ToBePlacedSystem>,
    nodes: Vec<Node>,
    conflicts: &mut [Option<usize>],
    batches: &mut Batches,
) -> Result<Vec<BatchInfo>, error::AddWorkload> {
    let mut systems: Vec<Option<ToBePlacedSystem>> =
        to_be_placed_systems.into_iter().map(Some).collect();

    let mut scheduled = match schedule(nodes) {
        Ok(scheduled) => scheduled,
        Err(cycle) => {
            return Err(error::AddWorkload::ImpossibleRequirements(
                error::ImpossibleRequirements::Cycle(
                    cycle
                        .into_iter()
                        .map(|occurrence| systems[occurrence].take().unwrap().display_name)
                        .collect(),
                ),
            ));
        }
    };

    let mut batches_info = Vec::new();
    for batch in scheduled.batches {
        batches.parallel.push((None, Vec::new()));
        batches.parallel_run_if.push((usize::MAX, Vec::new()));
        batches.parallel_occurrences.push((usize::MAX, Vec::new()));
        batches_info.push(BatchInfo {
            systems: (None, Vec::new()),
            regroups: Vec::new(),
        });

        for occurrence in batch {
            let mut system = systems[occurrence].take().unwrap();

            // Not fitting in a batch is a more recent reason to be in a later batch than the implicit conflict.
            if let Some(conflict) = scheduled.conflicts[occurrence].take() {
                system.confict = Some(conflict);
            }
            conflicts[occurrence] = system.confict.as_ref().map(|&(other, _)| other);

            place_system(system, batches, &mut batches_info);
        }
    }

    Ok(batches_info)
}

/// Adds `system` to the last batch, and after every placed system in the sequential order.
fn place_system(system: ToBePlacedSystem, batches: &mut Batches, batches_info: &mut [BatchInfo]) {
    let run_if_index = if let Some(run_if) = system.run_if {
        batches.systems_run_if.push(run_if);

        batches.systems_run_if.len() - 1
    } else {
        usize::MAX
    };

    batches.sequential.push(Step::System(system.index));
    batches.sequential_occurrences.push(system.occurrence);
    batches.sequential_run_if.push(run_if_index);

    let thread_bound = is_thread_bound(&system.footprint);

    let occurrence = SystemOccurrence {
        id: system.occurrence,
        conditions: system.conditions_borrow,
        footprint: system.footprint,
        thread_bound,
        after: predecessors(system.origins),
    };
    // Every `batch_after` edge records its origin when it's added, so both list the same predecessors.
    debug_assert!(
        occurrence
            .after
            .iter()
            .map(|predecessor| predecessor.occurrence)
            .eq(system.after_info.iter().map(OccurrenceRef::System))
    );
    let info = SystemInfo {
        name: format!("{:?}", system.display_name),
        borrow: system.borrow_constraints,
        conflict: system.confict.map(|(_, conflict)| conflict),
        after: system.after_info.0,
        after_all: system.after_all_info,
        before_all: system.before_all_info,
        unique_id: system.index,
        occurrence,
    };

    let batch = batches.parallel.last_mut().unwrap();
    let batch_run_if = batches.parallel_run_if.last_mut().unwrap();
    let batch_occurrences = batches.parallel_occurrences.last_mut().unwrap();
    let batch_info = batches_info.last_mut().unwrap();

    if thread_bound {
        batch.0 = Some(Step::System(system.index));
        batch_run_if.0 = run_if_index;
        batch_occurrences.0 = system.occurrence;
        batch_info.systems.0 = Some(info);
    } else {
        batch.1.push(Step::System(system.index));
        batch_run_if.1.push(run_if_index);
        batch_occurrences.1.push(system.occurrence);
        batch_info.systems.1.push(info);
    }
}

/// Groups `origins` by predecessor occurrence, both sorted and without duplicates.
fn predecessors(mut origins: Vec<(usize, OrderingOrigin)>) -> Vec<Predecessor> {
    origins.sort();
    origins.dedup();

    let mut predecessors: Vec<Predecessor> = Vec::new();
    for (occurrence, origin) in origins {
        let occurrence = OccurrenceRef::System(occurrence);

        match predecessors.last_mut() {
            Some(last) if last.occurrence == occurrence => last.origins.push(origin),
            _ => predecessors.push(Predecessor {
                occurrence,
                origins: vec![origin],
            }),
        }
    }

    predecessors
}

/// Returns `true` if what borrows `footprint` has to run on the calling thread.
///
/// A batch has a single slot for it. A condition needing the calling thread takes its system along,
/// they run together.
pub(super) fn is_thread_bound(footprint: &[TypeInfo]) -> bool {
    footprint.iter().any(|type_info| {
        !type_info.thread_safe || type_info.storage_id == TypeId::of::<AllStorages>()
    })
}

/// Something [`schedule`] places in a batch: a system occurrence or a regroup.
#[derive(Clone)]
pub(super) struct Node {
    /// Index of the system's callable, the conflicts with the node name it. `usize::MAX` for a regroup.
    pub(super) callable: usize,
    /// What the node borrows when it runs, a system's conditions included.
    pub(super) footprint: Vec<TypeInfo>,
    /// The nodes that have to be in an earlier batch.
    pub(super) batch_after: DedupedUniqueIds,
    /// The nodes that have to be placed first, the node can join their batch.
    pub(super) order_after: DedupedUniqueIds,
}

/// The batches [`schedule`] made.
pub(super) struct Schedule {
    /// The nodes of each batch, in the order they were placed.
    pub(super) batches: Vec<Vec<usize>>,
    /// For each node, why it couldn't join the last batch it was tried for, with the node it conflicts with.
    /// `None` if it joined the first one.
    pub(super) conflicts: Vec<Option<(usize, Conflict)>>,
}

/// Splits `nodes` into batches following their edges, with Kahn's algorithm, one batch at a time.
///
/// Edges point to indexes in `nodes`. A node is ready once its edges are satisfied.\
/// Each batch takes every ready node that fits in it, in order, the others wait for the next batch.
/// Ready nodes are tried in the order of `nodes`, after the ones that didn't fit in the previous batch.
///
/// ### Errors
///
/// The nodes left when their edges make a cycle, in order.
pub(super) fn schedule(mut nodes: Vec<Node>) -> Result<Schedule, Vec<usize>> {
    let mut conflicts = vec![None; nodes.len()];
    let mut waiting: Vec<usize> = (0..nodes.len()).collect();

    let mut ready: VecDeque<usize> = waiting
        .extract_if(.., |node| {
            nodes[*node].batch_after.is_empty() && nodes[*node].order_after.is_empty()
        })
        .collect();

    let mut batches = Vec::new();
    while !ready.is_empty() {
        let placed = fill_batch(&mut ready, &mut waiting, &mut nodes, &mut conflicts);

        // The batch is closed, the nodes waiting on its nodes can go in the next one.
        // They come after the nodes that didn't fit in the batch.
        ready.extend(waiting.extract_if(.., |node| {
            let node = &mut nodes[*node];
            node.batch_after
                .retain(|predecessor| !placed.contains(&predecessor));

            node.batch_after.is_empty() && node.order_after.is_empty()
        }));

        batches.push(placed);
    }

    if !waiting.is_empty() {
        return Err(waiting);
    }

    Ok(Schedule { batches, conflicts })
}

/// Opens a batch and places every node of `ready` that fits in it, in order.
///
/// Placing a node frees the nodes only waiting on it in the sequential order,
/// they're tried for the same batch.\
/// The nodes that conflict with the batch are left in `ready`, in order.
///
/// Returns the nodes placed in the batch.
fn fill_batch(
    ready: &mut VecDeque<usize>,
    waiting: &mut Vec<usize>,
    nodes: &mut [Node],
    conflicts: &mut [Option<(usize, Conflict)>],
) -> Vec<usize> {
    let mut deferred = VecDeque::new();
    let mut placed = Vec::new();
    // The calling-thread node, it's compared first.
    let mut single = None;
    let mut others = Vec::new();
    while let Some(node) = ready.pop_front() {
        // A node can't join a batch that already conflicts with it, the batch only grows.
        // So we set it aside for the next batch and keep trying the others.
        let conflict = single.iter().chain(&others).find_map(|&other: &usize| {
            check_conflict(
                &nodes[node].footprint,
                nodes[other].callable,
                &nodes[other].footprint,
            )
            .map(|conflict| (other, conflict))
        });
        if let Some(conflict) = conflict {
            conflicts[node] = Some(conflict);
            deferred.push_back(node);

            continue;
        }

        ready.extend(waiting.extract_if(.., |other| {
            let other = &mut nodes[*other];
            other.order_after.remove(node);

            other.batch_after.is_empty() && other.order_after.is_empty()
        }));

        if is_thread_bound(&nodes[node].footprint) {
            single = Some(node);
        } else {
            others.push(node);
        }
        placed.push(node);
    }

    *ready = deferred;

    placed
}

/// Returns why what borrows `footprint` can't run alongside what borrows `other_footprint`,
/// `other_callable` being its callable index.
fn check_conflict(
    footprint: &[TypeInfo],
    other_callable: usize,
    other_footprint: &[TypeInfo],
) -> Option<Conflict> {
    for type_info in footprint {
        for other_type_info in other_footprint {
            if !type_info.thread_safe && !other_type_info.thread_safe {
                return Some(Conflict::OtherNotSendSync {
                    system: other_callable,
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
                    other_system: other_callable,
                    other_type_info: other_type_info.clone(),
                });
            }
        }
    }

    None
}

// Tests are located in workload.rs
