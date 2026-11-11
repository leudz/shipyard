//! The dataset the views display, built once per successful import.
//!
//! Records live in vectors sorted by label and are referenced by index.
//! So sorting ids also sorts by label, which is how every index below is ordered.

use crate::names::{label_cmp, unique_labels};
use shipyard::borrow::Mutability;
use shipyard::scheduler::info::{RegroupInfo, RegroupTrigger, SystemInfo, WorkloadsInfo};
use std::collections::{BTreeSet, HashMap};

/// Index of a [`Workload`] in [`Dataset::workloads`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkloadId(pub usize);

/// Index of a [`System`] in [`Dataset::systems`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemId(pub usize);

/// Index of a [`Component`] in [`Dataset::components`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub usize);

/// Index of an [`Occurrence`] in [`Dataset::occurrences`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OccurrenceId(pub usize);

/// Index of a [`Regroup`] in [`Dataset::regroups`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegroupId(pub usize);

/// Anything a batch runs: a system appearance or a regroup.
///
/// Only meaningful for the [`Dataset`] it comes from, it isn't an id from the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationId {
    System(OccurrenceId),
    Regroup(RegroupId),
}

/// Everything the views display about an imported export.
///
/// Fields are public to read, the indexes between them are only consistent as built by [`Dataset::new`].
#[derive(Debug, Default)]
pub struct Dataset {
    /// Sorted by label.
    pub workloads: Vec<Workload>,
    /// Sorted by label.
    pub systems: Vec<System>,
    /// Every storage a system or regroup borrows, sorted by label.
    pub components: Vec<Component>,
    /// Every place a system appears in a batch.
    pub occurrences: Vec<Occurrence>,
    /// Every regroup of every batch. Repeated regroups of the same storages stay distinct.
    pub regroups: Vec<Regroup>,
}

#[derive(Debug)]
pub struct Workload {
    /// The workload's key in the export.
    pub name: String,
    pub label: String,
    /// In execution order, with the export's numbering. Batches without systems are kept.
    pub batches: Vec<Batch>,
}

/// Systems and regroups a workload runs in parallel.
#[derive(Debug)]
pub struct Batch {
    /// The system running on the world's thread, when there is one.
    pub thread_bound: Option<OccurrenceId>,
    /// The other systems, sorted by label. They run in parallel, this order is only for display.
    pub others: Vec<OccurrenceId>,
    /// In the export's order.
    pub regroups: Vec<RegroupId>,
}

impl Batch {
    /// The batch's system occurrences in display order, the thread-bound one first.
    ///
    /// Regroups aren't included, see [`Batch::operations`] for everything the batch runs.
    pub fn occurrences(&self) -> impl Iterator<Item = OccurrenceId> + '_ {
        self.thread_bound.iter().chain(&self.others).copied()
    }

    /// Everything the batch runs in display order: its systems like [`Batch::occurrences`], then its regroups.
    pub fn operations(&self) -> impl Iterator<Item = OperationId> + '_ {
        let systems = self.occurrences().map(OperationId::System);
        let regroups = self.regroups.iter().copied().map(OperationId::Regroup);

        systems.chain(regroups)
    }
}

/// A system, grouped across all the workloads and batches it appears in.
///
/// Systems are the same when both their full name and their borrows match, regardless of borrow order.
#[derive(Debug)]
pub struct System {
    pub name: String,
    pub label: String,
    /// Sorted by component label.
    pub borrows: Vec<Borrow>,
    /// Sorted by workload label, then batch.
    pub occurrences: Vec<OccurrenceId>,
}

/// A storage a system borrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Borrow {
    pub component: ComponentId,
    pub mutability: Mutability,
    pub thread_safe: bool,
}

/// A storage, identified by its full name.
#[derive(Debug)]
pub struct Component {
    pub name: String,
    pub label: String,
    /// Sorted by system label.
    pub borrowers: Vec<Borrower>,
    /// Regroups including this storage, they always borrow it exclusively. Sorted by workload label, then batch.
    pub regroups: Vec<RegroupId>,
}

/// A system borrowing a [`Component`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Borrower {
    pub system: SystemId,
    pub mutability: Mutability,
    pub thread_safe: bool,
}

/// One appearance of a [`System`] in a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occurrence {
    pub workload: WorkloadId,
    /// Index in [`Workload::batches`].
    pub batch: usize,
    /// The export's `unique_id`, only unique within the workload.
    pub local_id: usize,
    pub system: SystemId,
    /// Whether this is the batch's [`Batch::thread_bound`] system.
    pub thread_bound: bool,
}

/// One regroup in a batch.
///
/// The workload adds regroups itself, two regroups of the same storages are still distinct records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regroup {
    pub workload: WorkloadId,
    /// Index in [`Workload::batches`].
    pub batch: usize,
    /// Every storage of the regrouped set, all exclusive. Sorted by component label.
    pub borrows: Vec<Borrow>,
    /// A pending regroup only means the set may need it, not that a system ran or changed anything.
    pub trigger: RegroupTrigger,
    /// Whether one of the storages isn't `thread_safe`, so the regroup needs the world's thread.
    pub thread_bound: bool,
}

/// What makes two exported systems the same [`System`].
///
/// We compare names instead of `TypeInfo`, its equality uses a `storage_id` the export doesn't contain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SystemKey {
    name: String,
    /// Sorted, so borrow order doesn't matter.
    borrows: Vec<(String, Mutability, bool)>,
}

impl SystemKey {
    fn new(system: &SystemInfo) -> SystemKey {
        let mut borrows: Vec<_> = system
            .borrow
            .iter()
            .map(|type_info| {
                let name = type_info.name.to_string();

                (name, type_info.mutability, type_info.thread_safe)
            })
            .collect();
        borrows.sort();

        SystemKey {
            name: system.name.clone(),
            borrows,
        }
    }
}

impl Dataset {
    /// Builds the records and indexes for `info`.
    pub fn new(info: &WorkloadsInfo) -> Dataset {
        let all_batches = || info.0.values().flat_map(|workload| &workload.batches_info);
        let all_systems = || all_batches().flat_map(|batch| batch.systems());
        let all_regroups = || all_batches().flat_map(|batch| &batch.regroups);

        // Components come from system and regroup borrows.
        let system_storages = all_systems().flat_map(|system| &system.borrow);
        let regroup_storages = all_regroups().flat_map(|regroup| &regroup.borrow);
        let component_names: BTreeSet<&str> = system_storages
            .chain(regroup_storages)
            .map(|type_info| &*type_info.name)
            .collect();
        let mut components: Vec<Component> = labelled(component_names)
            .map(|(name, label)| Component {
                name: name.to_string(),
                label,
                borrowers: Vec::new(),
                regroups: Vec::new(),
            })
            .collect();
        components.sort_by(|a, b| label_cmp(&a.label, &b.label));
        let component_ids: HashMap<&str, ComponentId> = components
            .iter()
            .enumerate()
            .map(|(index, component)| (component.name.as_str(), ComponentId(index)))
            .collect();

        // The key set orders systems sharing a name by their borrows, which decides their ` #n`.
        let system_keys: BTreeSet<SystemKey> = all_systems().map(SystemKey::new).collect();
        let key_names: Vec<&str> = system_keys.iter().map(|key| key.name.as_str()).collect();
        let mut keyed_systems: Vec<(&SystemKey, System)> = system_keys
            .iter()
            .zip(unique_labels(&key_names))
            .map(|(key, label)| (key, system_record(key, label, &component_ids)))
            .collect();
        keyed_systems.sort_by(|(_, a), (_, b)| label_cmp(&a.label, &b.label));
        let system_ids: HashMap<&SystemKey, SystemId> = keyed_systems
            .iter()
            .enumerate()
            .map(|(index, (key, _))| (*key, SystemId(index)))
            .collect();
        let mut systems: Vec<System> = keyed_systems
            .into_iter()
            .map(|(_, system)| system)
            .collect();

        let mut sorted_workloads: Vec<_> = labelled(info.0.keys().map(String::as_str))
            .map(|(name, label)| (name, label, &info.0[name]))
            .collect();
        sorted_workloads.sort_by(|(_, a, _), (_, b, _)| label_cmp(a, b));

        // We walk workloads in label order, so each system's occurrences come out sorted too.
        let mut occurrences = Vec::new();
        let mut regroups = Vec::new();
        let mut workloads = Vec::new();
        for (workload_index, (name, label, workload_info)) in
            sorted_workloads.into_iter().enumerate()
        {
            let mut batches = Vec::new();

            for (batch_index, batch_info) in workload_info.batches_info.iter().enumerate() {
                let mut add_occurrence = |system: &SystemInfo, thread_bound: bool| {
                    let id = OccurrenceId(occurrences.len());
                    let system_id = system_ids[&SystemKey::new(system)];

                    occurrences.push(Occurrence {
                        workload: WorkloadId(workload_index),
                        batch: batch_index,
                        local_id: system.unique_id,
                        system: system_id,
                        thread_bound,
                    });
                    systems[system_id.0].occurrences.push(id);

                    id
                };

                let thread_bound = batch_info
                    .systems
                    .0
                    .as_ref()
                    .map(|system| add_occurrence(system, true));
                let mut others: Vec<OccurrenceId> = batch_info
                    .systems
                    .1
                    .iter()
                    .map(|system| add_occurrence(system, false))
                    .collect();
                others.sort_by_key(|id| {
                    let occurrence = &occurrences[id.0];

                    (occurrence.system, occurrence.local_id)
                });

                let batch_regroups: Vec<RegroupId> = batch_info
                    .regroups
                    .iter()
                    .map(|regroup_info| {
                        let id = RegroupId(regroups.len());
                        let regroup = regroup_record(
                            regroup_info,
                            WorkloadId(workload_index),
                            batch_index,
                            &component_ids,
                        );

                        regroups.push(regroup);

                        id
                    })
                    .collect();

                batches.push(Batch {
                    thread_bound,
                    others,
                    regroups: batch_regroups,
                });
            }

            workloads.push(Workload {
                name: name.to_string(),
                label,
                batches,
            });
        }

        // `component_ids` borrows the component names, so we fill the reverse indexes once we're done with it.
        // We walk systems in label order, so each component's borrowers come out sorted.
        for (index, system) in systems.iter().enumerate() {
            for borrow in &system.borrows {
                components[borrow.component.0].borrowers.push(Borrower {
                    system: SystemId(index),
                    mutability: borrow.mutability,
                    thread_safe: borrow.thread_safe,
                });
            }
        }

        // Regroups were created in workload label order, then batch order.
        for (index, regroup) in regroups.iter().enumerate() {
            for borrow in &regroup.borrows {
                components[borrow.component.0]
                    .regroups
                    .push(RegroupId(index));
            }
        }

        Dataset {
            workloads,
            systems,
            components,
            occurrences,
            regroups,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.workloads.is_empty()
    }

    pub fn workload(&self, id: WorkloadId) -> &Workload {
        &self.workloads[id.0]
    }

    pub fn system(&self, id: SystemId) -> &System {
        &self.systems[id.0]
    }

    pub fn component(&self, id: ComponentId) -> &Component {
        &self.components[id.0]
    }

    pub fn occurrence(&self, id: OccurrenceId) -> &Occurrence {
        &self.occurrences[id.0]
    }

    pub fn regroup(&self, id: RegroupId) -> &Regroup {
        &self.regroups[id.0]
    }

    /// The workload and batch index `operation` runs in.
    pub fn location(&self, operation: OperationId) -> (WorkloadId, usize) {
        match operation {
            OperationId::System(id) => {
                let occurrence = self.occurrence(id);

                (occurrence.workload, occurrence.batch)
            }
            OperationId::Regroup(id) => {
                let regroup = self.regroup(id);

                (regroup.workload, regroup.batch)
            }
        }
    }
}

/// Pairs each name with its label.
fn labelled<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> impl Iterator<Item = (&'a str, String)> {
    let names: Vec<&str> = names.into_iter().collect();
    let labels = unique_labels(&names);

    names.into_iter().zip(labels)
}

fn system_record(
    key: &SystemKey,
    label: String,
    component_ids: &HashMap<&str, ComponentId>,
) -> System {
    let mut borrows: Vec<Borrow> = key
        .borrows
        .iter()
        .map(|(name, mutability, thread_safe)| Borrow {
            component: component_ids[name.as_str()],
            mutability: *mutability,
            thread_safe: *thread_safe,
        })
        .collect();
    borrows.sort_by_key(|borrow| borrow.component);

    System {
        name: key.name.clone(),
        label,
        borrows,
        occurrences: Vec::new(),
    }
}

fn regroup_record(
    info: &RegroupInfo,
    workload: WorkloadId,
    batch: usize,
    component_ids: &HashMap<&str, ComponentId>,
) -> Regroup {
    let mut borrows: Vec<Borrow> = info
        .borrow
        .iter()
        .map(|type_info| Borrow {
            component: component_ids[&*type_info.name],
            mutability: type_info.mutability,
            thread_safe: type_info.thread_safe,
        })
        .collect();
    borrows.sort_by_key(|borrow| borrow.component);

    // Same rule as the scheduler: a single storage that isn't thread safe pins the regroup to the world's thread.
    let thread_bound = borrows.iter().any(|borrow| !borrow.thread_safe);

    Regroup {
        workload,
        batch,
        borrows,
        trigger: info.trigger,
        thread_bound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// An exported system borrowing each of `borrows` as `Shared`.
    fn system(name: &str, unique_id: usize, borrows: &[&str]) -> Value {
        let borrows: Vec<Value> = borrows
            .iter()
            .map(|name| json!({"name": name, "mutability": "Shared", "thread_safe": true}))
            .collect();

        json!({
            "name": name,
            "borrow": borrows,
            "conflict": null,
            "after": [],
            "after_all": [],
            "before_all": [],
            "unique_id": unique_id,
        })
    }

    fn batch(thread_bound: Option<Value>, others: Vec<Value>) -> Value {
        json!({"systems": [thread_bound, others]})
    }

    fn dataset(workloads: Vec<(&str, Vec<Value>)>) -> Dataset {
        let workloads: serde_json::Map<String, Value> = workloads
            .into_iter()
            .map(|(name, batches)| {
                let workload = json!({"name": name, "batches_info": batches});

                (name.to_string(), workload)
            })
            .collect();

        Dataset::new(&serde_json::from_value(Value::Object(workloads)).unwrap())
    }

    fn system_labels(dataset: &Dataset) -> Vec<&str> {
        dataset
            .systems
            .iter()
            .map(|system| system.label.as_str())
            .collect()
    }

    /// Asserts that an export without workloads builds an empty dataset.
    #[test]
    fn empty() {
        let dataset = dataset(vec![]);

        assert!(dataset.is_empty());
        assert!(dataset.systems.is_empty());
    }

    /// Asserts that the same local id in two workloads makes two distinct occurrences of different systems.
    #[test]
    fn local_id_reuse() {
        let dataset = dataset(vec![
            ("W(a)", vec![batch(None, vec![system("a::run", 0, &[])])]),
            ("W(b)", vec![batch(None, vec![system("b::walk", 0, &[])])]),
        ]);

        let [first, second] = &dataset.occurrences[..] else {
            panic!("expected two occurrences");
        };
        assert_eq!((first.local_id, second.local_id), (0, 0));
        assert_ne!(first.workload, second.workload);
        assert_ne!(first.system, second.system);
    }

    /// Asserts that a system appearing in two workloads and two batches is grouped, with one occurrence each.
    #[test]
    fn repeated_occurrences() {
        let dataset = dataset(vec![
            (
                "a",
                vec![
                    batch(None, vec![system("run", 0, &["C"])]),
                    batch(None, vec![system("run", 1, &["C"])]),
                ],
            ),
            ("b", vec![batch(None, vec![system("run", 4, &["C"])])]),
        ]);

        assert_eq!(dataset.systems.len(), 1);
        let places: Vec<_> = dataset.systems[0]
            .occurrences
            .iter()
            .map(|&id| {
                let occurrence = dataset.occurrence(id);

                (
                    dataset.workload(occurrence.workload).label.as_str(),
                    occurrence.batch,
                )
            })
            .collect();
        assert_eq!(places, [("a", 0), ("a", 1), ("b", 0)]);
    }

    /// Asserts that systems with the same short name from different modules stay distinct and readable.
    #[test]
    fn same_short_name() {
        let dataset = dataset(vec![(
            "w",
            vec![batch(
                None,
                vec![
                    system("System(a::run)", 0, &[]),
                    system("System(b::run)", 1, &[]),
                ],
            )],
        )]);

        assert_eq!(system_labels(&dataset), ["a::run", "b::run"]);
    }

    /// Asserts that equal names borrowing different storages are different systems.
    #[test]
    fn equal_names_different_borrows() {
        let dataset = dataset(vec![(
            "w",
            vec![batch(
                None,
                vec![system("run", 0, &["B"]), system("run", 1, &["A"])],
            )],
        )]);

        assert_eq!(system_labels(&dataset), ["run #1", "run #2"]);
        let first_borrow = dataset.systems[0].borrows[0].component;
        assert_eq!(dataset.component(first_borrow).name, "A");
    }

    /// Asserts that the same borrows listed in another order belong to the same system.
    #[test]
    fn reordered_borrows() {
        let dataset = dataset(vec![
            ("a", vec![batch(None, vec![system("run", 0, &["A", "B"])])]),
            ("b", vec![batch(None, vec![system("run", 0, &["B", "A"])])]),
        ]);

        assert_eq!(dataset.systems.len(), 1);
        assert_eq!(dataset.systems[0].occurrences.len(), 2);
    }

    /// Asserts that mutability is part of a system's identity.
    #[test]
    fn different_mutability() {
        let mut exclusive = system("run", 1, &["A"]);
        exclusive["borrow"][0]["mutability"] = json!("Exclusive");

        let dataset = dataset(vec![(
            "w",
            vec![batch(None, vec![system("run", 0, &["A"]), exclusive])],
        )]);

        assert_eq!(dataset.systems.len(), 2);
    }

    /// Asserts that different storage names stay distinct even though their skipped `storage_id`s deserialize equal.
    #[test]
    fn distinct_storages() {
        let dataset = dataset(vec![(
            "w",
            vec![batch(None, vec![system("run", 0, &["a::A", "b::B"])])],
        )]);

        let names: Vec<&str> = dataset.components.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["a::A", "b::B"]);
        assert_eq!(dataset.systems[0].borrows.len(), 2);
    }

    /// Asserts that the thread-bound system comes first in its batch and the others are sorted by label.
    #[test]
    fn thread_bound() {
        let dataset = dataset(vec![(
            "w",
            vec![batch(
                Some(system("zebra", 0, &[])),
                vec![system("mole", 1, &[]), system("ant", 2, &[])],
            )],
        )]);

        let batch = &dataset.workloads[0].batches[0];
        let labels: Vec<&str> = batch
            .occurrences()
            .map(|id| dataset.system(dataset.occurrence(id).system).label.as_str())
            .collect();
        assert_eq!(labels, ["zebra", "ant", "mole"]);
        assert!(dataset.occurrence(batch.thread_bound.unwrap()).thread_bound);
    }

    /// Asserts that empty batches keep their place, so later batches keep the export's numbering.
    #[test]
    fn empty_batch() {
        let dataset = dataset(vec![(
            "w",
            vec![
                batch(None, vec![]),
                batch(None, vec![system("run", 0, &[])]),
            ],
        )]);

        assert_eq!(dataset.workloads[0].batches.len(), 2);
        assert_eq!(dataset.occurrences[0].batch, 1);
    }

    /// An exported regroup of `storages`, all thread safe.
    fn regroup(storages: &[&str], trigger: &str) -> Value {
        let borrows: Vec<Value> = storages
            .iter()
            .map(|name| json!({"name": name, "mutability": "Exclusive", "thread_safe": true}))
            .collect();

        json!({"borrow": borrows, "trigger": trigger})
    }

    fn with_regroups(mut batch: Value, regroups: Vec<Value>) -> Value {
        batch["regroups"] = Value::Array(regroups);

        batch
    }

    /// Asserts that a storage only a regroup borrows becomes a component, listing that regroup.
    #[test]
    fn regroup_storage() {
        let regrouping = with_regroups(batch(None, vec![]), vec![regroup(&["Z"], "Entry")]);

        let dataset = dataset(vec![(
            "w",
            vec![regrouping, batch(None, vec![system("run", 0, &["A"])])],
        )]);

        let names: Vec<&str> = dataset.components.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["A", "Z"]);
        assert!(dataset.components[1].borrowers.is_empty());
        assert_eq!(dataset.components[1].regroups, [RegroupId(0)]);
    }

    /// Asserts that a batch running only a regroup keeps it, with its location and no system occurrence.
    #[test]
    fn regroup_only_batch() {
        let regrouping = with_regroups(batch(None, vec![]), vec![regroup(&["A"], "Entry")]);

        let dataset = dataset(vec![("w", vec![batch(None, vec![]), regrouping])]);

        let batch = &dataset.workloads[0].batches[1];
        assert_eq!(batch.occurrences().count(), 0);
        let operations: Vec<_> = batch.operations().collect();
        assert_eq!(operations, [OperationId::Regroup(RegroupId(0))]);
        assert_eq!(dataset.location(operations[0]), (WorkloadId(0), 1));
    }

    /// Asserts that a mixed batch lists its systems first, then its regroups, and that `occurrences` stays system-only.
    #[test]
    fn mixed_batch() {
        let mixed = with_regroups(
            batch(Some(system("bound", 0, &[])), vec![system("free", 1, &[])]),
            vec![regroup(&["A"], "Pending")],
        );

        let dataset = dataset(vec![("w", vec![mixed])]);

        let batch = &dataset.workloads[0].batches[0];
        assert_eq!(batch.occurrences().count(), 2);
        let operations: Vec<_> = batch.operations().collect();
        assert_eq!(
            operations,
            [
                OperationId::System(OccurrenceId(0)),
                OperationId::System(OccurrenceId(1)),
                OperationId::Regroup(RegroupId(0)),
            ]
        );
    }

    /// Asserts that an empty batch has no operations at all.
    #[test]
    fn empty_batch_operations() {
        let dataset = dataset(vec![("w", vec![batch(None, vec![])])]);

        assert_eq!(dataset.workloads[0].batches[0].operations().count(), 0);
        assert!(dataset.regroups.is_empty());
    }

    /// Asserts that repeated regroups of one set, entry and pending in the same workload, stay distinct records.
    #[test]
    fn repeated_regroups() {
        let first = with_regroups(batch(None, vec![]), vec![regroup(&["B", "A"], "Entry")]);
        let second = with_regroups(batch(None, vec![]), vec![regroup(&["A", "B"], "Pending")]);

        let dataset = dataset(vec![("w", vec![first, second])]);

        let [entry, pending] = &dataset.regroups[..] else {
            panic!("expected two regroups");
        };
        assert_eq!((entry.batch, entry.trigger), (0, RegroupTrigger::Entry));
        assert_eq!(
            (pending.batch, pending.trigger),
            (1, RegroupTrigger::Pending)
        );
        assert_eq!(entry.borrows, pending.borrows);
        let first_storage = dataset.component(entry.borrows[0].component);
        assert_eq!(first_storage.name, "A");
        assert_eq!(first_storage.regroups, [RegroupId(0), RegroupId(1)]);
    }

    /// Asserts that a regroup with a storage that isn't thread safe needs the world's thread, and one without doesn't.
    #[test]
    fn thread_bound_regroup() {
        let mut bound = regroup(&["A", "B"], "Pending");
        bound["borrow"][1]["thread_safe"] = json!(false);
        let regrouping =
            with_regroups(batch(None, vec![]), vec![bound, regroup(&["C"], "Pending")]);

        let dataset = dataset(vec![("w", vec![regrouping])]);

        assert!(dataset.regroups[0].thread_bound);
        assert!(!dataset.regroups[1].thread_bound);
    }

    /// Asserts that regroups don't change grouped systems or their borrowers.
    #[test]
    fn regroups_keep_systems() {
        let mixed = with_regroups(
            batch(None, vec![system("run", 0, &["A"])]),
            vec![regroup(&["A"], "Pending")],
        );

        let dataset = dataset(vec![("w", vec![mixed])]);

        assert_eq!(system_labels(&dataset), ["run"]);
        assert_eq!(dataset.components[0].borrowers.len(), 1);
        assert_eq!(dataset.components[0].regroups, [RegroupId(0)]);
    }

    /// Asserts that components list their borrowers by system label, with the borrow's mutability.
    #[test]
    fn borrowers() {
        let mut exclusive = system("beta", 1, &["A"]);
        exclusive["borrow"][0]["mutability"] = json!("Exclusive");

        let dataset = dataset(vec![(
            "w",
            vec![batch(None, vec![exclusive, system("alpha", 0, &["A"])])],
        )]);

        let borrowers: Vec<_> = dataset.components[0]
            .borrowers
            .iter()
            .map(|borrower| {
                (
                    dataset.system(borrower.system).label.as_str(),
                    borrower.mutability,
                )
            })
            .collect();
        assert_eq!(
            borrowers,
            [
                ("alpha", Mutability::Shared),
                ("beta", Mutability::Exclusive)
            ]
        );
    }

    /// Asserts that workloads are sorted by label, digit runs by value.
    #[test]
    fn workload_order() {
        let dataset = dataset(vec![("w10", vec![]), ("w2", vec![]), ("W1", vec![])]);

        let labels: Vec<&str> = dataset.workloads.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, ["W1", "w2", "w10"]);
    }
}
