//! Why an operation couldn't run in the batch right before its own.
//!
//! We compare each operation with every operation of the previous batch, using the scheduling facts
//! of [`crate::facts`]. Every incompatibility is kept, not only the first one the scheduler found.

use crate::facts::{Access, OperationFacts};
use crate::model::{Dataset, OperationId};
use shipyard::borrow::Mutability;
use std::collections::{BTreeMap, HashMap};

/// Why an operation couldn't share a batch with an operation of the previous batch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// Both need the calling thread, a batch only has one slot running there.
    ThreadSlot,
    /// These two borrows can't happen at the same time.
    ///
    /// Either they're of the same storage and one is exclusive, or one is `AllStorages`.
    Borrow { blocker: Access, blocked: Access },
}

/// Every adjacent-batch blocker of a dataset, keyed by `(blocker, blocked)`.
#[derive(Debug, Default)]
pub struct ConflictGraph {
    edges: BTreeMap<(OperationId, OperationId), Vec<Reason>>,
    /// The blockers of each blocked operation, sorted.
    incoming: HashMap<OperationId, Vec<OperationId>>,
}

impl ConflictGraph {
    /// Compares every operation with the operations of the batch before its own, in every workload.
    pub fn new(dataset: &Dataset) -> ConflictGraph {
        let mut graph = ConflictGraph::default();

        for workload in &dataset.workloads {
            for pair in workload.batches.windows(2) {
                let [previous, batch] = pair else {
                    unreachable!("windows of two");
                };

                for blocked in batch.operations() {
                    for blocker in previous.operations() {
                        let reasons =
                            reasons(dataset.facts.get(blocker), dataset.facts.get(blocked));

                        graph.add(blocker, blocked, reasons);
                    }
                }
            }
        }

        graph
    }

    /// Adds `reasons` to the edge from `blocker` to `blocked`, creating it if needed.
    ///
    /// Nothing happens without reasons.
    pub(crate) fn add(&mut self, blocker: OperationId, blocked: OperationId, reasons: Vec<Reason>) {
        if reasons.is_empty() {
            return;
        }

        let edge = self.edges.entry((blocker, blocked)).or_default();
        if edge.is_empty() {
            let blockers = self.incoming.entry(blocked).or_default();
            let position = blockers
                .binary_search(&blocker)
                .unwrap_or_else(|position| position);
            blockers.insert(position, blocker);
        }

        edge.extend(reasons);
        edge.sort();
        edge.dedup();
    }

    /// Why `blocked` couldn't run alongside `blocker`, `None` if nothing is known.
    pub fn reasons(&self, blocker: OperationId, blocked: OperationId) -> Option<&[Reason]> {
        self.edges.get(&(blocker, blocked)).map(Vec::as_slice)
    }

    /// The operations of the previous batch `blocked` couldn't run alongside, sorted.
    pub fn blockers(&self, blocked: OperationId) -> &[OperationId] {
        self.incoming.get(&blocked).map_or(&[], Vec::as_slice)
    }

    /// Every `(blocker, blocked)` pair with its reasons, sorted by pair.
    pub fn edges(&self) -> impl Iterator<Item = ((OperationId, OperationId), &[Reason])> {
        self.edges
            .iter()
            .map(|(&pair, reasons)| (pair, reasons.as_slice()))
    }
}

/// Every reason `blocked` can't run at the same time as `blocker`, sorted.
fn reasons(blocker: &OperationFacts, blocked: &OperationFacts) -> Vec<Reason> {
    let mut reasons = Vec::new();

    // Two operations needing the calling thread can't share a batch, whatever they borrow.
    if blocker.thread_bound && blocked.thread_bound {
        reasons.push(Reason::ThreadSlot);
    }

    for blocked_access in &blocked.accesses {
        for blocker_access in &blocker.accesses {
            if are_incompatible(blocker_access, blocked_access) {
                reasons.push(Reason::Borrow {
                    blocker: blocker_access.clone(),
                    blocked: blocked_access.clone(),
                });
            }
        }
    }

    reasons.sort();
    reasons.dedup();

    reasons
}

/// Returns `true` if the scheduler can't let both borrows happen at the same time.
///
/// Storages are compared by full name, deserialized storage ids are all equal.
fn are_incompatible(a: &Access, b: &Access) -> bool {
    // `AllStorages` can reach every storage, it conflicts with any other borrow.
    if is_all_storages(&a.storage) || is_all_storages(&b.storage) {
        return true;
    }

    let either_exclusive =
        a.mutability == Mutability::Exclusive || b.mutability == Mutability::Exclusive;

    a.storage == b.storage && either_exclusive
}

/// Returns `true` if `storage` is the full name of `AllStorages`.
pub fn is_all_storages(storage: &str) -> bool {
    storage == std::any::type_name::<shipyard::all_storages::AllStorages>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::add_occurrences;
    use crate::model::{OccurrenceId, RegroupId};
    use serde_json::{json, Value};

    /// A system borrowing `borrows`, each `(storage, "Shared" or "Exclusive")`.
    fn system(name: &str, borrows: &[(&str, &str)]) -> Value {
        let borrows: Vec<Value> = borrows
            .iter()
            .map(|(storage, mutability)| {
                json!({"name": storage, "mutability": mutability, "thread_safe": true})
            })
            .collect();

        json!({
            "name": name,
            "borrow": borrows,
            "conflict": null,
            "after": [],
            "after_all": [],
            "before_all": [],
            "unique_id": 0,
        })
    }

    fn regroup(storage: &str) -> Value {
        json!({
            "borrow": [{"name": storage, "mutability": "Exclusive", "thread_safe": true}],
            "trigger": "Pending",
        })
    }

    fn batch(systems: Vec<Value>) -> Value {
        json!({"systems": [null, systems]})
    }

    fn thread_bound_batch(system: Value) -> Value {
        json!({"systems": [system, []]})
    }

    fn regroup_batch(regroups: Vec<Value>) -> Value {
        json!({"systems": [null, []], "regroups": regroups})
    }

    fn load(workloads: Value) -> Dataset {
        let workloads = workloads
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, batches)| {
                let mut batches = batches.as_array().unwrap().clone();
                add_occurrences(&mut batches);
                let workload = json!({"name": name, "batches_info": batches});

                (name.clone(), workload)
            })
            .collect();

        Dataset::new(&serde_json::from_value(Value::Object(workloads)).unwrap()).unwrap()
    }

    fn sys(occurrence: usize) -> OperationId {
        OperationId::System(OccurrenceId(occurrence))
    }

    fn reg(regroup: usize) -> OperationId {
        OperationId::Regroup(RegroupId(regroup))
    }

    /// The storages of each `Borrow` reason, as `(blocker, blocked)`.
    fn borrowed(reasons: &[Reason]) -> Vec<(&str, &str)> {
        reasons
            .iter()
            .filter_map(|reason| match reason {
                Reason::Borrow { blocker, blocked } => {
                    Some((blocker.storage.as_str(), blocked.storage.as_str()))
                }
                Reason::ThreadSlot => None,
            })
            .collect()
    }

    /// Asserts that a system blocked by two systems of the previous batch lists both.
    #[test]
    fn multiple_blockers() {
        let dataset = load(json!({"w": [
            batch(vec![system("a", &[("A", "Exclusive")]), system("b", &[("B", "Exclusive")])]),
            batch(vec![system("c", &[("A", "Shared"), ("B", "Shared")])]),
        ]}));

        assert_eq!(dataset.conflicts.blockers(sys(2)), [sys(0), sys(1)]);
    }

    /// Asserts that a pair incompatible on two storages keeps both, not only the first.
    #[test]
    fn multiple_storages() {
        let dataset = load(json!({"w": [
            batch(vec![system("a", &[("A", "Exclusive"), ("B", "Exclusive")])]),
            batch(vec![system("b", &[("A", "Shared"), ("B", "Shared")])]),
        ]}));

        let reasons = dataset.conflicts.reasons(sys(0), sys(1)).unwrap();
        assert_eq!(borrowed(reasons), [("A", "A"), ("B", "B")]);
    }

    /// Asserts that two shared borrows of a storage don't block.
    #[test]
    fn shared_shared() {
        let dataset = load(json!({"w": [
            batch(vec![system("a", &[("A", "Shared")])]),
            batch(vec![system("b", &[("A", "Shared")])]),
        ]}));

        assert_eq!(dataset.conflicts.edges().count(), 0);
    }

    /// Asserts that an exclusive borrow blocks a shared one, with both mutabilities kept.
    #[test]
    fn exclusive_shared() {
        let dataset = load(json!({"w": [
            batch(vec![system("a", &[("A", "Exclusive")])]),
            batch(vec![system("b", &[("A", "Shared")])]),
        ]}));

        let reasons = dataset.conflicts.reasons(sys(0), sys(1)).unwrap();
        let [Reason::Borrow { blocker, blocked }] = reasons else {
            panic!("expected one borrow reason");
        };
        assert_eq!(blocker.mutability, Mutability::Exclusive);
        assert_eq!(blocked.mutability, Mutability::Shared);
    }

    /// Asserts that two exclusive borrows of a storage block.
    #[test]
    fn exclusive_exclusive() {
        let dataset = load(json!({"w": [
            batch(vec![system("a", &[("A", "Exclusive")])]),
            batch(vec![system("b", &[("A", "Exclusive")])]),
        ]}));

        assert!(dataset.conflicts.reasons(sys(0), sys(1)).is_some());
    }

    /// Asserts that `AllStorages` blocks a borrow of another storage, even a shared one.
    #[test]
    fn all_storages() {
        let all = std::any::type_name::<shipyard::all_storages::AllStorages>();

        let dataset = load(json!({"w": [
            batch(vec![system("a", &[(all, "Shared")])]),
            batch(vec![system("b", &[("B", "Shared")])]),
        ]}));

        let reasons = dataset.conflicts.reasons(sys(0), sys(1)).unwrap();
        assert_eq!(borrowed(reasons), [(all, "B")]);
    }

    /// Asserts that two thread-bound systems with disjoint borrows block each other for the slot.
    #[test]
    fn thread_slot() {
        let dataset = load(json!({"w": [
            thread_bound_batch(system("a", &[("A", "Shared")])),
            thread_bound_batch(system("b", &[("B", "Shared")])),
        ]}));

        assert_eq!(
            dataset.conflicts.reasons(sys(0), sys(1)),
            Some(&[Reason::ThreadSlot][..])
        );
    }

    /// Asserts that a thread-bound system doesn't block a system that can run on any thread.
    #[test]
    fn thread_bound_alone() {
        let dataset = load(json!({"w": [
            thread_bound_batch(system("a", &[("A", "Shared")])),
            batch(vec![system("b", &[("B", "Shared")])]),
        ]}));

        assert_eq!(dataset.conflicts.edges().count(), 0);
    }

    /// Asserts that a storage only a condition borrows still blocks, described as the condition's borrow.
    #[test]
    fn condition_only() {
        let mut writer = system("a", &[("A", "Exclusive")]);
        writer["occurrence"] = json!({
            "id": 0, "conditions": [], "thread_bound": false, "after": [],
            "footprint": [{"name": "A", "mutability": "Exclusive", "thread_safe": true}],
        });
        let mut guarded = system("b", &[]);
        guarded["occurrence"] = json!({
            "id": 1, "thread_bound": false, "after": [],
            "conditions": [{"name": "A", "mutability": "Shared", "thread_safe": true}],
            "footprint": [{"name": "A", "mutability": "Shared", "thread_safe": true}],
        });

        let dataset = load(json!({"w": [batch(vec![writer]), batch(vec![guarded])]}));

        let reasons = dataset.conflicts.reasons(sys(0), sys(1)).unwrap();
        let [Reason::Borrow { blocked, .. }] = reasons else {
            panic!("expected one borrow reason");
        };
        assert_eq!(blocked.body, None);
        assert_eq!(blocked.condition, Some(Mutability::Shared));
    }

    /// Asserts that a regroup blocks a system borrowing from its set.
    #[test]
    fn regroup_blocks_system() {
        let dataset = load(json!({"w": [
            regroup_batch(vec![regroup("A")]),
            batch(vec![system("a", &[("A", "Shared")])]),
        ]}));

        assert_eq!(dataset.conflicts.blockers(sys(0)), [reg(0)]);
    }

    /// Asserts that a shared borrow in the previous batch blocks a regroup, which borrows exclusively.
    #[test]
    fn system_blocks_regroup() {
        let dataset = load(json!({"w": [
            batch(vec![system("a", &[("A", "Shared")])]),
            regroup_batch(vec![regroup("A")]),
        ]}));

        assert_eq!(dataset.conflicts.blockers(reg(0)), [sys(0)]);
    }

    /// Asserts that two regroups of one set in consecutive batches block each other.
    #[test]
    fn regroup_blocks_regroup() {
        let dataset = load(json!({"w": [
            regroup_batch(vec![regroup("A")]),
            regroup_batch(vec![regroup("A")]),
        ]}));

        assert_eq!(dataset.conflicts.blockers(reg(1)), [reg(0)]);
    }

    /// Asserts that only the batch right before counts: nothing across a batch, an empty batch,
    /// or another workload, and nothing for the first batch.
    #[test]
    fn adjacent_only() {
        let dataset = load(json!({
            "v": [batch(vec![system("a", &[("A", "Exclusive")])])],
            "w": [
                batch(vec![system("b", &[("A", "Exclusive")])]),
                batch(vec![system("c", &[("C", "Exclusive")])]),
                batch(vec![system("d", &[("A", "Exclusive")])]),
                batch(vec![]),
                batch(vec![system("e", &[("A", "Exclusive")])]),
            ],
        }));

        assert_eq!(dataset.conflicts.edges().count(), 0);
    }

    /// Asserts that a real export's conflict, a writer then a reader, comes out as one borrow edge.
    #[test]
    fn real_export() {
        use shipyard::{Component, View, ViewMut, World};

        struct A;
        impl Component for A {
            type Tracking = shipyard::track::Untracked;
        }

        fn write(_: ViewMut<'_, A>) {}
        fn read(_: View<'_, A>) {}

        let world = World::new();
        shipyard::Workload::new("w")
            .with_system(write)
            .with_system(read)
            .add_to_world(&world)
            .unwrap();
        let json = serde_json::to_vec(&world.workloads_info()).unwrap();

        let dataset = crate::import::load_dataset(&json).unwrap();

        let edges: Vec<_> = dataset.conflicts.edges().collect();
        assert_eq!(edges.len(), 1);
        let ((blocker, blocked), reasons) = edges[0];
        assert_eq!(dataset.location(blocker).1, 0);
        assert_eq!(dataset.location(blocked).1, 1);
        assert_eq!(reasons.len(), 1);
    }

    /// Asserts that a real system borrowing `AllStorages` is blocked by a writer with a disjoint storage.
    #[test]
    fn real_all_storages() {
        use shipyard::{AllStoragesViewMut, Component, ViewMut, World};

        struct A;
        impl Component for A {
            type Tracking = shipyard::track::Untracked;
        }

        fn write(_: ViewMut<'_, A>) {}
        fn all(_: AllStoragesViewMut<'_>) {}

        let world = World::new();
        shipyard::Workload::new("w")
            .with_system(write)
            .with_system(all)
            .add_to_world(&world)
            .unwrap();
        let json = serde_json::to_vec(&world.workloads_info()).unwrap();

        let dataset = crate::import::load_dataset(&json).unwrap();

        let edges: Vec<_> = dataset.conflicts.edges().collect();
        assert_eq!(edges.len(), 1);
        assert!(dataset.facts.get(edges[0].0 .1).thread_bound);
    }
}
