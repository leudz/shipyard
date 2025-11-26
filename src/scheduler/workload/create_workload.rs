use crate::all_storages::AllStorages;
use crate::borrow::Mutability;
use crate::error;
use crate::scheduler::info::{BatchInfo, Conflict, DedupedLabels, SystemInfo};
use crate::scheduler::{Batches, Label, TypeId, TypeInfo, Workload, WorkloadInfo, WorkloadSystem};
use crate::world::World;
use crate::ShipHashMap;
use alloc::boxed::Box;
use alloc::format;
// use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::hash::Hash;

#[allow(clippy::type_complexity)]
struct ToBePlacedSystem {
    index: usize,
    display_name: Box<dyn Label>,
    borrow_constraints: Vec<TypeInfo>,
    tags: Vec<Box<dyn Label>>,
    before_all: DedupedLabels,
    after_all: DedupedLabels,
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

    fn retain(&mut self, mut label_eq: impl FnMut(&dyn Label) -> bool) {
        self.0.retain(|id| (label_eq)(&UniqueSystemId(*id)));
    }

    fn contains(&self, system_index: usize) -> bool {
        self.0.binary_search(&system_index).is_ok()
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn create_workload(
    mut builder: Workload,
    systems: &mut Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    system_names: &mut Vec<Box<dyn Label>>,
    system_generators: &mut Vec<Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>>,
    lookup_table: &mut ShipHashMap<TypeId, usize>,
    tracking_to_enable: &mut Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    workloads: &mut ShipHashMap<Box<dyn Label>, Batches>,
    default: &mut Box<dyn Label>,
) -> Result<WorkloadInfo, error::AddWorkload> {
    if workloads.contains_key(&builder.name) {
        return Err(error::AddWorkload::AlreadyExists);
    }

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

    let mut batches = Batches {
        workload_run_if,
        ..Default::default()
    };

    let batches_info = order_systems(&mut to_be_placed_systems, &mut batches)?;
    let workload_info = WorkloadInfo {
        name: format!("{:?}", workload_name),
        batches_info,
    };

    if workloads.is_empty() {
        *default = workload_name.clone();
    }
    workloads.insert(workload_name, batches);

    Ok(workload_info)
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
    builder
        .systems
        .drain(..)
        .map(
            |WorkloadSystem {
                 type_id,
                 display_name,
                 system_fn,
                 borrow_constraints,
                 mut tracking_to_enable,
                 generator,
                 run_if,
                 mut tags,
                 before_all,
                 after_all,
                 after,
                 before,
                 unique_id,
                 require_in_workload,
                 require_before,
                 require_after,
             }| {
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

                tags.push(Box::new(UniqueSystemId(unique_id)));

                ToBePlacedSystem {
                    index: system_index,
                    display_name,
                    borrow_constraints,
                    tags,
                    before_all,
                    after_all,
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

/// Uniquely identify systems to translate the implicit ordering to tags.
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

/// Translate the implicit ordering's rules to tags and after constraints.
fn propagate_implicit_hard_ordering(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in (1..to_be_placed_systems.len()).rev() {
        let system = &to_be_placed_systems[i];
        if !system.before_all.is_empty() || !system.after_all.is_empty() {
            // system has opted out of the implicit ordering
            // and shouldn't be considered here

            continue;
        }

        let borrow_constraints = &*system.borrow_constraints;

        for j in (0..i).rev() {
            let other_system = &to_be_placed_systems[j];
            if !other_system.before_all.is_empty() || !other_system.after_all.is_empty() {
                // other_system has opted out of the implicit ordering
                // and shouldn't be considered here

                continue;
            }

            if let Some(conflict) = check_conflict(other_system, borrow_constraints) {
                to_be_placed_systems[i].hard_after.add(j);
                to_be_placed_systems[i].confict = Some(conflict);

                break;
            }
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

        for (j, other_system) in to_be_placed_systems.iter_mut().enumerate() {
            if i == j {
                continue;
            }

            if other_system
                .tags
                .iter()
                .any(|tag| before_all.iter().any(|label| tag == label))
            {
                other_system.hard_after.add(i);
            }
        }

        // Put the constraints back in to be able to display them later
        to_be_placed_systems[i].before_all = before_all;
    }
}

/// Map the after_all list to after.
///
/// The goal is to only have constraints in a single form: after.
fn map_after_all_to_after(to_be_placed_systems: &mut [ToBePlacedSystem]) {
    for i in 0..to_be_placed_systems.len() {
        let after_all = core::mem::take(&mut to_be_placed_systems[i].after_all);

        for j in 0..to_be_placed_systems.len() {
            if i == j {
                continue;
            }

            for label in after_all.iter() {
                if to_be_placed_systems[j].tags.iter().any(|tag| tag == label) {
                    to_be_placed_systems[i].hard_after.add(j);

                    break;
                }
            }
        }

        // Put the constraints back in to be able to display them later
        to_be_placed_systems[i].after_all = after_all;
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
/// Use Kahn's algorithm to find a topological ordering.
fn order_systems(
    to_be_placed_systems: &mut Vec<ToBePlacedSystem>,
    batches: &mut Batches,
) -> Result<Vec<BatchInfo>, error::AddWorkload> {
    if to_be_placed_systems.is_empty() {
        return Ok(Vec::new());
    }

    for system in &mut *to_be_placed_systems {
        system.after_info = system.hard_after.clone();
    }

    let mut constraint_free_systems = alloc::collections::VecDeque::new();

    // Create a list with systems without constraints.
    // That can be the first system in the workload or systems
    // that do not borrow any component.
    constraint_free_systems.extend(to_be_placed_systems.extract_if(.., |system| {
        system.hard_after.is_empty() && system.soft_after.is_empty()
    }));

    let mut batches_info = vec![BatchInfo {
        systems: (None, Vec::new()),
    }];
    batches.parallel.push((None, Vec::new()));
    batches.parallel_run_if.push((usize::MAX, Vec::new()));
    let mut latest_batch = &mut batches.parallel[0];
    let mut latest_batch_run_if = &mut batches.parallel_run_if[0];
    let mut latest_batch_info = &mut batches_info[0];
    let mut to_delete_tags = Vec::new();
    loop {
        while let Some(mut system) = constraint_free_systems.pop_front() {
            batches.sequential.push(system.index);

            let conflict = check_can_go_in_parallel_batch(latest_batch_info, &system);
            if conflict.is_some() {
                batches.parallel.push((None, Vec::new()));
                batches.parallel_run_if.push((usize::MAX, Vec::new()));
                batches_info.push(BatchInfo {
                    systems: (None, Vec::new()),
                });

                latest_batch = batches.parallel.last_mut().unwrap();
                latest_batch_run_if = batches.parallel_run_if.last_mut().unwrap();
                latest_batch_info = batches_info.last_mut().unwrap();
            }

            let is_single_system = system.borrow_constraints.iter().any(|constraint| {
                !constraint.thread_safe || constraint.storage_id == TypeId::of::<AllStorages>()
            });

            if is_single_system {
                latest_batch.0 = Some(system.index);
                latest_batch_info.systems.0 = Some(SystemInfo {
                    name: format!("{:?}", system.display_name),
                    borrow: system.borrow_constraints,
                    conflict: system.confict,
                    after: system.after_info.0,
                    after_all: system.after_all.to_string_vec(),
                    before_all: system.before_all.to_string_vec(),
                    unique_id: system.index,
                });
            } else {
                latest_batch.1.push(system.index);
                latest_batch_info.systems.1.push(SystemInfo {
                    name: format!("{:?}", system.display_name),
                    borrow: system.borrow_constraints,
                    conflict: system.confict,
                    after: system.after_info.0,
                    after_all: system.after_all.to_string_vec(),
                    before_all: system.before_all.to_string_vec(),
                    unique_id: system.index,
                });
            }

            let run_if_index = if let Some(run_if) = system.run_if {
                batches.systems_run_if.push(run_if);

                batches.systems_run_if.len() - 1
            } else {
                usize::MAX
            };

            batches.sequential_run_if.push(run_if_index);
            if is_single_system {
                latest_batch_run_if.0 = run_if_index;
            } else {
                latest_batch_run_if.1.push(run_if_index);
            }

            constraint_free_systems.extend(to_be_placed_systems.extract_if(.., |other_system| {
                // We remove constraints to the system we just remove
                other_system
                    .soft_after
                    .retain(|constraint| system.tags.iter().all(|tag| !tag.dyn_eq(constraint)));

                other_system.hard_after.is_empty() && other_system.soft_after.is_empty()
            }));

            to_delete_tags.append(&mut system.tags);
        }

        constraint_free_systems.extend(to_be_placed_systems.extract_if(.., |other_system| {
            // We remove constraints to the system we just remove
            other_system
                .hard_after
                .retain(|constraint| to_delete_tags.iter().all(|tag| !tag.dyn_eq(constraint)));

            other_system.hard_after.is_empty() && other_system.soft_after.is_empty()
        }));

        if constraint_free_systems.is_empty() {
            break;
        } else {
            batches.parallel.push((None, Vec::new()));
            batches.parallel_run_if.push((usize::MAX, Vec::new()));
            batches_info.push(BatchInfo {
                systems: (None, Vec::new()),
            });
            to_delete_tags.clear();

            latest_batch = batches.parallel.last_mut().unwrap();
            latest_batch_run_if = batches.parallel_run_if.last_mut().unwrap();
            latest_batch_info = batches_info.last_mut().unwrap();
        }
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

fn check_can_go_in_parallel_batch(
    batch_info: &BatchInfo,
    tested_system: &ToBePlacedSystem,
) -> Option<Conflict> {
    if let Some(single_system_info) = &batch_info.systems.0 {
        if let Some(conflict) = check_conflict(tested_system, &single_system_info.borrow) {
            return Some(conflict);
        };
    }

    for system_info in &batch_info.systems.1 {
        if let Some(conflict) = check_conflict(tested_system, &system_info.borrow) {
            return Some(conflict);
        };
    }

    None
}

fn check_conflict(
    tested_system: &ToBePlacedSystem,
    borrow_constraints: &[TypeInfo],
) -> Option<Conflict> {
    for other_type_info in &tested_system.borrow_constraints {
        for type_info in borrow_constraints {
            if !type_info.thread_safe && !other_type_info.thread_safe {
                return Some(Conflict::OtherNotSendSync {
                    system: tested_system.index,
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
                    type_info: Some(type_info.clone()),
                    other_system: tested_system.index,
                    other_type_info: other_type_info.clone(),
                });
            }
        }
    }

    None
}

// Tests are located in workload.rs
