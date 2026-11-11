//! What the export says about how each operation was scheduled, checked and normalized.
//!
//! Exports describe every system and regroup with its occurrence information, whose references
//! can't be confused. We check them and reject the file when they're broken.

use crate::import::ImportError;
use crate::model::{OccurrenceId, OperationId, RegroupId};
use shipyard::borrow::Mutability;
use shipyard::scheduler::info::{OccurrenceRef, OrderingOrigin, RegroupInfo, SystemInfo, TypeInfo};
use std::collections::HashMap;

/// The scheduling facts of every operation of a dataset.
#[derive(Debug, Default)]
pub struct Facts {
    /// Indexed by [`OccurrenceId`].
    systems: Vec<OperationFacts>,
    /// Indexed by [`RegroupId`].
    regroups: Vec<OperationFacts>,
}

impl Facts {
    pub fn get(&self, operation: OperationId) -> &OperationFacts {
        match operation {
            OperationId::System(id) => &self.systems[id.0],
            OperationId::Regroup(id) => &self.regroups[id.0],
        }
    }
}

/// How one operation was scheduled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationFacts {
    /// What the operation borrows when it runs, sorted by storage name.
    pub accesses: Vec<Access>,
    /// Whether the operation runs on the calling thread.
    pub thread_bound: bool,
    /// The operations that have to run in an earlier batch, sorted.
    pub predecessors: Vec<Prerequisite>,
}

/// A storage an operation borrows when it runs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Access {
    /// The storage's full name, it identifies the storage.
    pub storage: String,
    /// How the scheduler compared it: exclusive when the system or one of its conditions is.
    pub mutability: Mutability,
    pub thread_safe: bool,
    /// How the system itself, or the regroup, borrows it.
    pub body: Option<Mutability>,
    /// How the system's conditions borrow it.
    pub condition: Option<Mutability>,
}

/// An operation that has to run in an earlier batch, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prerequisite {
    pub operation: OperationId,
    /// Sorted, never empty.
    pub origins: Vec<OrderingOrigin>,
}

/// One workload's operations, with the ids [`crate::model::Dataset`] gave them.
pub(crate) struct WorkloadInput<'a> {
    pub(crate) name: &'a str,
    pub(crate) systems: Vec<(OccurrenceId, &'a SystemInfo)>,
    pub(crate) regroups: Vec<(RegroupId, &'a RegroupInfo)>,
}

/// Builds the facts of every operation of `workloads`.
///
/// Returns [`ImportError::UnsupportedFormat`] when a workload's occurrence information is broken:
/// ids repeat, or a reference doesn't resolve.
pub(crate) fn normalize(
    workloads: &[WorkloadInput<'_>],
    system_count: usize,
    regroup_count: usize,
) -> Result<Facts, ImportError> {
    let mut systems: Vec<Option<OperationFacts>> = vec![None; system_count];
    let mut regroups: Vec<Option<OperationFacts>> = vec![None; regroup_count];

    for workload in workloads {
        let normalized = workload_facts(workload).map_err(|reason| {
            ImportError::UnsupportedFormat(format!("Workload {}: {reason}", workload.name))
        })?;

        for (operation, facts) in normalized {
            match operation {
                OperationId::System(id) => systems[id.0] = Some(facts),
                OperationId::Regroup(id) => regroups[id.0] = Some(facts),
            }
        }
    }

    // Every operation belongs to one of the workloads.
    let complete = |facts: Vec<Option<OperationFacts>>| -> Vec<OperationFacts> {
        facts.into_iter().map(Option::unwrap).collect()
    };

    Ok(Facts {
        systems: complete(systems),
        regroups: complete(regroups),
    })
}

fn workload_facts(
    workload: &WorkloadInput<'_>,
) -> Result<Vec<(OperationId, OperationFacts)>, String> {
    // Both id spaces are local to the workload, we resolve references in this workload only.
    let mut system_ids: HashMap<usize, OccurrenceId> = HashMap::new();
    for &(id, info) in &workload.systems {
        let exported = info.occurrence.id;

        if system_ids.insert(exported, id).is_some() {
            return Err(format!("two systems have occurrence id {exported}."));
        }
    }
    let mut regroup_ids: HashMap<usize, RegroupId> = HashMap::new();
    for &(id, info) in &workload.regroups {
        let exported = info.occurrence.id;

        if regroup_ids.insert(exported, id).is_some() {
            return Err(format!("two regroups have occurrence id {exported}."));
        }
    }

    let resolve = |reference: OccurrenceRef| -> Result<OperationId, String> {
        match reference {
            OccurrenceRef::System(exported) => match system_ids.get(&exported) {
                Some(&id) => Ok(OperationId::System(id)),
                None if regroup_ids.contains_key(&exported) => Err(format!(
                    "a reference to system {exported} only matches a regroup."
                )),
                None => Err(format!("a reference to system {exported} matches nothing.")),
            },
            OccurrenceRef::Regroup(exported) => match regroup_ids.get(&exported) {
                Some(&id) => Ok(OperationId::Regroup(id)),
                None if system_ids.contains_key(&exported) => Err(format!(
                    "a reference to regroup {exported} only matches a system."
                )),
                None => Err(format!(
                    "a reference to regroup {exported} matches nothing."
                )),
            },
        }
    };

    let mut normalized = Vec::new();
    for &(id, info) in &workload.systems {
        let operation = OperationId::System(id);
        let occurrence = &info.occurrence;

        let mut accesses: Vec<Access> = occurrence
            .footprint
            .iter()
            .map(|type_info| Access {
                storage: type_info.name.to_string(),
                mutability: type_info.mutability,
                thread_safe: type_info.thread_safe,
                body: mutability_in(&info.borrow, type_info),
                condition: mutability_in(&occurrence.conditions, type_info),
            })
            .collect();
        accesses.sort_by(|a, b| a.storage.cmp(&b.storage));

        let predecessors = prerequisites(operation, &occurrence.after, &resolve)?;

        normalized.push((
            operation,
            OperationFacts {
                accesses,
                thread_bound: occurrence.thread_bound,
                predecessors,
            },
        ));
    }
    for &(id, info) in &workload.regroups {
        let operation = OperationId::Regroup(id);
        let occurrence = &info.occurrence;

        let predecessors = prerequisites(operation, &occurrence.after, &resolve)?;

        normalized.push((
            operation,
            OperationFacts {
                accesses: regroup_accesses(info),
                thread_bound: occurrence.thread_bound,
                predecessors,
            },
        ));
    }

    Ok(normalized)
}

/// Resolves `after`'s references, merging the origins of a predecessor listed twice.
fn prerequisites(
    operation: OperationId,
    after: &[shipyard::scheduler::info::Predecessor],
    resolve: &impl Fn(OccurrenceRef) -> Result<OperationId, String>,
) -> Result<Vec<Prerequisite>, String> {
    let mut merged: Vec<Prerequisite> = Vec::new();

    for predecessor in after {
        let other = resolve(predecessor.occurrence)?;
        if other == operation {
            return Err("an operation has to run before itself.".to_string());
        }
        if predecessor.origins.is_empty() {
            return Err("a predecessor has no origin.".to_string());
        }

        match merged.iter_mut().find(|known| known.operation == other) {
            Some(known) => known.origins.extend(predecessor.origins.iter().cloned()),
            None => merged.push(Prerequisite {
                operation: other,
                origins: predecessor.origins.clone(),
            }),
        }
    }

    for prerequisite in &mut merged {
        prerequisite.origins.sort();
        prerequisite.origins.dedup();
    }
    merged.sort_by_key(|prerequisite| prerequisite.operation);

    Ok(merged)
}

/// A regroup borrows every storage of its set exclusively.
fn regroup_accesses(info: &RegroupInfo) -> Vec<Access> {
    let mut accesses: Vec<Access> = info
        .borrow
        .iter()
        .map(|type_info| Access {
            storage: type_info.name.to_string(),
            mutability: type_info.mutability,
            thread_safe: type_info.thread_safe,
            body: Some(type_info.mutability),
            condition: None,
        })
        .collect();
    accesses.sort_by(|a, b| a.storage.cmp(&b.storage));

    accesses
}

/// How `borrows` borrows `storage`'s storage, compared by name: deserialized storage ids are all equal.
fn mutability_in(borrows: &[TypeInfo], storage: &TypeInfo) -> Option<Mutability> {
    let mut found = None;

    for type_info in borrows
        .iter()
        .filter(|type_info| type_info.name == storage.name)
    {
        found = match (found, type_info.mutability) {
            (Some(Mutability::Exclusive), _) | (_, Mutability::Exclusive) => {
                Some(Mutability::Exclusive)
            }
            _ => Some(Mutability::Shared),
        };
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Dataset;
    use serde_json::{json, Value};

    /// An exported system borrowing `storage` exclusively, with occurrence `id` and `after`.
    fn system(name: &str, unique_id: usize, id: usize, storage: &str, after: Value) -> Value {
        json!({
            "name": name,
            "borrow": [{"name": storage, "mutability": "Exclusive", "thread_safe": true}],
            "conflict": null,
            "after": [],
            "after_all": [],
            "before_all": [],
            "unique_id": unique_id,
            "occurrence": {
                "id": id,
                "conditions": [],
                "footprint": [{"name": storage, "mutability": "Exclusive", "thread_safe": true}],
                "thread_bound": false,
                "after": after,
            },
        })
    }

    /// An exported regroup of `storage` with occurrence `id` and `after`.
    fn regroup(id: usize, storage: &str, after: Value) -> Value {
        json!({
            "borrow": [{"name": storage, "mutability": "Exclusive", "thread_safe": true}],
            "trigger": "Pending",
            "occurrence": {"id": id, "thread_bound": false, "after": after},
        })
    }

    fn batch(systems: Vec<Value>, regroups: Vec<Value>) -> Value {
        json!({"systems": [null, systems], "regroups": regroups})
    }

    fn load(workloads: Value) -> Result<Dataset, ImportError> {
        let workloads = workloads
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, batches)| {
                let workload = json!({"name": name, "batches_info": batches});

                (name.clone(), workload)
            })
            .collect();

        Dataset::new(&serde_json::from_value(Value::Object(workloads)).unwrap())
    }

    fn rejection(workloads: Value) -> String {
        match load(workloads) {
            Err(ImportError::UnsupportedFormat(details)) => details,
            other => panic!("expected an unsupported format, got {other:?}"),
        }
    }

    fn system_facts(dataset: &Dataset, occurrence: usize) -> &OperationFacts {
        dataset
            .facts
            .get(OperationId::System(OccurrenceId(occurrence)))
    }

    fn predecessors(facts: &OperationFacts) -> Vec<OperationId> {
        facts
            .predecessors
            .iter()
            .map(|prerequisite| prerequisite.operation)
            .collect()
    }

    /// Asserts that the footprint replaces what the system borrows, even when it's empty.
    #[test]
    fn empty_footprint() {
        let mut empty = system("s", 0, 0, "A", json!([]));
        empty["occurrence"]["footprint"] = json!([]);

        let dataset = load(json!({"w": [batch(vec![empty], vec![])]})).unwrap();

        let facts = system_facts(&dataset, 0);
        assert!(facts.accesses.is_empty());
        assert!(facts.predecessors.is_empty());
    }

    /// Asserts that references use occurrence ids, not the callable indices, which can be offset.
    #[test]
    fn offset_callables() {
        let first = system("first", 7, 0, "A", json!([]));
        let second = system(
            "second",
            8,
            1,
            "A",
            json!([{"occurrence": {"System": 0}, "origins": ["Implicit"]}]),
        );

        let dataset =
            load(json!({"w": [batch(vec![first], vec![]), batch(vec![second], vec![])]})).unwrap();

        assert_eq!(
            predecessors(system_facts(&dataset, 1)),
            [OperationId::System(OccurrenceId(0))]
        );
    }

    /// Asserts that two occurrences of one callable keep their own footprint and their own edges.
    #[test]
    fn repeated_callables() {
        let first = system("run", 0, 0, "A", json!([]));
        let mut second = system(
            "run",
            0,
            1,
            "A",
            json!([{"occurrence": {"System": 0}, "origins": ["Implicit"]}]),
        );
        second["occurrence"]["conditions"] =
            json!([{"name": "B", "mutability": "Shared", "thread_safe": true}]);
        second["occurrence"]["footprint"] = json!([
            {"name": "A", "mutability": "Exclusive", "thread_safe": true},
            {"name": "B", "mutability": "Shared", "thread_safe": true},
        ]);

        let dataset =
            load(json!({"w": [batch(vec![first], vec![]), batch(vec![second], vec![])]})).unwrap();

        assert_eq!(system_facts(&dataset, 0).accesses.len(), 1);
        let second = system_facts(&dataset, 1);
        assert_eq!(second.accesses.len(), 2);
        assert_eq!(second.accesses[1].body, None);
        assert_eq!(second.accesses[1].condition, Some(Mutability::Shared));
        assert_eq!(predecessors(second), [OperationId::System(OccurrenceId(0))]);
        assert!(system_facts(&dataset, 0).predecessors.is_empty());
    }

    /// Asserts that a storage the system and its condition both borrow keeps both mutabilities.
    #[test]
    fn body_and_condition() {
        let mut both = system("s", 0, 0, "A", json!([]));
        both["borrow"][0]["mutability"] = json!("Shared");
        both["occurrence"]["conditions"] =
            json!([{"name": "A", "mutability": "Exclusive", "thread_safe": true}]);

        let dataset = load(json!({"w": [batch(vec![both], vec![])]})).unwrap();

        let access = &system_facts(&dataset, 0).accesses[0];
        assert_eq!(access.mutability, Mutability::Exclusive);
        assert_eq!(access.body, Some(Mutability::Shared));
        assert_eq!(access.condition, Some(Mutability::Exclusive));
    }

    /// Asserts that a reference to the second of two regroups of one set resolves to that regroup.
    #[test]
    fn repeated_regroups() {
        let reader = system(
            "read",
            0,
            0,
            "A",
            json!([{"occurrence": {"Regroup": 1}, "origins": ["Regroup"]}]),
        );

        let dataset = load(json!({"w": [
            batch(vec![], vec![regroup(0, "A", json!([]))]),
            batch(vec![], vec![regroup(1, "A", json!([]))]),
            batch(vec![reader], vec![]),
        ]}))
        .unwrap();

        assert_eq!(
            predecessors(system_facts(&dataset, 0)),
            [OperationId::Regroup(RegroupId(1))]
        );
        let regroup = dataset.facts.get(OperationId::Regroup(RegroupId(1)));
        assert_eq!(regroup.accesses[0].mutability, Mutability::Exclusive);
    }

    /// Asserts that a predecessor listed twice is merged, keeping every distinct origin once, sorted.
    #[test]
    fn several_origins() {
        let first = system("first", 0, 0, "A", json!([]));
        let second = system(
            "second",
            1,
            1,
            "A",
            json!([
                {"occurrence": {"System": 0}, "origins": ["Barrier", "Implicit"]},
                {"occurrence": {"System": 0}, "origins": [{"BeforeAll": "x"}, "Barrier"]},
            ]),
        );

        let dataset =
            load(json!({"w": [batch(vec![first], vec![]), batch(vec![second], vec![])]})).unwrap();

        let prerequisites = &system_facts(&dataset, 1).predecessors;
        assert_eq!(prerequisites.len(), 1);
        assert_eq!(
            prerequisites[0].origins,
            [
                OrderingOrigin::Implicit,
                OrderingOrigin::Barrier,
                OrderingOrigin::BeforeAll("x".to_string())
            ]
        );
    }

    /// Asserts that two systems with the same occurrence id reject the file.
    #[test]
    fn duplicate_system_id() {
        let details = rejection(json!({"w": [batch(
            vec![system("a", 0, 3, "A", json!([])), system("b", 1, 3, "B", json!([]))],
            vec![],
        )]}));

        assert!(details.contains("Workload w"));
        assert!(details.contains("occurrence id 3"));
    }

    /// Asserts that two regroups with the same occurrence id reject the file.
    #[test]
    fn duplicate_regroup_id() {
        rejection(json!({"w": [
            batch(vec![], vec![regroup(0, "A", json!([]))]),
            batch(vec![], vec![regroup(0, "B", json!([]))]),
        ]}));
    }

    /// Asserts that a reference to a missing occurrence rejects the file.
    #[test]
    fn dangling() {
        let details = rejection(json!({"w": [batch(
            vec![system("a", 0, 0, "A", json!([{"occurrence": {"System": 9}, "origins": ["Implicit"]}]))],
            vec![],
        )]}));

        assert!(details.contains("system 9"));
    }

    /// Asserts that a system reference whose number only matches a regroup rejects the file.
    #[test]
    fn wrong_kind() {
        let details = rejection(json!({"w": [
            batch(vec![], vec![regroup(4, "A", json!([]))]),
            batch(vec![system("a", 0, 0, "A", json!([{"occurrence": {"System": 4}, "origins": ["Regroup"]}]))], vec![]),
        ]}));

        assert!(details.contains("only matches a regroup"));
    }

    /// Asserts that an operation having to run before itself rejects the file.
    #[test]
    fn self_reference() {
        rejection(json!({"w": [batch(
            vec![system("a", 0, 0, "A", json!([{"occurrence": {"System": 0}, "origins": ["Implicit"]}]))],
            vec![],
        )]}));
    }

    /// Asserts that a reference only matching an occurrence of another workload rejects the file.
    #[test]
    fn cross_workload() {
        let details = rejection(json!({
            "v": [batch(vec![system("a", 0, 5, "A", json!([]))], vec![])],
            "w": [batch(vec![system("b", 0, 0, "A", json!([{"occurrence": {"System": 5}, "origins": ["Implicit"]}]))], vec![])],
        }));

        assert!(details.contains("Workload w"));
    }

    /// Asserts that a predecessor without any origin rejects the file.
    #[test]
    fn no_origin() {
        rejection(json!({"w": [
            batch(vec![system("a", 0, 0, "A", json!([]))], vec![]),
            batch(vec![system("b", 1, 1, "A", json!([{"occurrence": {"System": 0}, "origins": []}]))], vec![]),
        ]}));
    }

    /// Asserts that a system without occurrence information, like older versions export, rejects the file.
    #[test]
    fn missing_occurrence() {
        let mut old = system("s", 0, 0, "A", json!([]));
        old.as_object_mut().unwrap().remove("occurrence");
        let workloads = json!({"w": {"name": "w", "batches_info": [batch(vec![old], vec![])]}});

        let error = crate::import::load_dataset(workloads.to_string().as_bytes()).unwrap_err();

        assert!(matches!(error, ImportError::UnsupportedFormat(_)));
    }

    /// Asserts that the bundled example loads.
    #[test]
    fn square_eater() {
        let bytes = include_bytes!("square_eater_workloads.json");

        crate::import::load_dataset(bytes).unwrap();
    }
}
