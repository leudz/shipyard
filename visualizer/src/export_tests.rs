//! The producer/consumer contract, checked with exports shipyard actually writes.
//!
//! `conflict_workloads.json` is committed so a change on either side shows up as a failing test.
//! Regenerate it after an intended producer change with
//! `cargo test -p visualizer --lib --no-default-features -- --ignored regenerate`.

use crate::conflicts::Reason;
use crate::import::load_dataset;
use crate::model::{Dataset, OperationId, WorkloadId};
use shipyard::scheduler::info::OrderingOrigin;
use shipyard::scheduler::{IntoSequentialWorkload, SystemModificator, WorkloadModificator};
use shipyard::{Component, View, ViewMut, Workload, World};
use std::collections::BTreeSet;

const FIXTURE: &[u8] = include_bytes!("conflict_workloads.json");
const FIXTURE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/conflict_workloads.json");

struct A;
impl Component for A {
    type Tracking = shipyard::track::Untracked;
}
struct B;
impl Component for B {
    type Tracking = shipyard::track::Untracked;
}
/// Not grouped, so its workload has no regroup between its systems.
struct C;
impl Component for C {
    type Tracking = shipyard::track::Untracked;
}

fn write_a(_: ViewMut<'_, A>) {}
fn read_a(_: View<'_, A>) {}
fn read_b(_: View<'_, B>) {}
fn write_c(_: ViewMut<'_, C>) {}
fn read_c(_: View<'_, C>) {}
fn first() {}
fn second() {}
fn third() {}

/// Every workload of the fixture, in a world grouping A and B so regroups get planned.
///
/// - `pair`: a writer, a barrier, then a reader. One pair, two ordering origins and a borrow.
/// - `labels`: `second` is `after_all(first)`, then a barrier before `third`.
/// - `sequential`: three systems in a sequential workload.
/// - `regroups`: a writer and a reader of A, the writer again, then a reader of B, which is in A's
///   group. Entry and pending regroups go between them.
fn export() -> Vec<u8> {
    let world = World::builder().with_group::<(A, B)>().build();

    Workload::new("pair")
        .with_system(write_c)
        .with_barrier()
        .with_system(read_c)
        .add_to_world(&world)
        .unwrap();
    Workload::new("labels")
        .with_system(first)
        .with_system(second.after_all(first))
        .with_barrier()
        .with_system(third)
        .add_to_world(&world)
        .unwrap();
    (first, second, third)
        .into_sequential_workload()
        .rename("sequential")
        .add_to_world(&world)
        .unwrap();
    Workload::new("regroups")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_a)
        .with_system(read_b)
        .add_to_world(&world)
        .unwrap();

    serde_json::to_vec_pretty(&world.workloads_info()).unwrap()
}

fn workload(dataset: &Dataset, label: &str) -> WorkloadId {
    let index = dataset
        .workloads
        .iter()
        .position(|workload| workload.label.contains(label))
        .unwrap();

    WorkloadId(index)
}

/// Every link of `workload` as `(blocker label, blocked label)`, labels including the batch.
fn links(dataset: &Dataset, workload: WorkloadId) -> BTreeSet<(String, String)> {
    dataset
        .conflicts
        .edges()
        .filter(|((blocker, _), _)| dataset.location(*blocker).0 == workload)
        .map(|((blocker, blocked), _)| {
            (
                dataset.operation_label(blocker),
                dataset.operation_label(blocked),
            )
        })
        .collect()
}

/// The reasons of the link from the operation labelled `blocker` to the one labelled `blocked`.
fn reasons<'a>(dataset: &'a Dataset, blocker: &str, blocked: &str) -> &'a [Reason] {
    dataset
        .conflicts
        .edges()
        .find(|((from, to), _)| {
            dataset.operation_label(*from) == blocker && dataset.operation_label(*to) == blocked
        })
        .map(|(_, reasons)| reasons)
        .unwrap_or_else(|| panic!("no link from {blocker} to {blocked}"))
}

fn fixture() -> Dataset {
    load_dataset(FIXTURE).unwrap()
}

/// Rewrites the committed fixture from the current producer.
#[test]
#[ignore = "writes the fixture, run it after an intended producer change"]
fn regenerate() {
    std::fs::write(FIXTURE_PATH, export()).unwrap();
}

/// Asserts that the committed fixture still describes what the producer exports today:
/// the same links between the same operations.
#[test]
fn fixture_matches_producer() {
    let fixture = fixture();
    let current = load_dataset(&export()).unwrap();

    for label in ["pair", "labels", "sequential", "regroups"] {
        assert_eq!(
            links(&fixture, workload(&fixture, label)),
            links(&current, workload(&current, label)),
            "{label}"
        );
    }
}

/// Asserts that one pair keeps its borrow conflict and both of its ordering origins.
#[test]
fn several_reasons() {
    let dataset = fixture();

    let reasons = reasons(&dataset, "write_c (batch 0)", "read_c (batch 1)");

    assert!(reasons
        .iter()
        .any(|reason| matches!(reason, Reason::Borrow { .. })));
    assert!(reasons.contains(&Reason::Ordering(OrderingOrigin::Implicit)));
    assert!(reasons.contains(&Reason::Ordering(OrderingOrigin::Barrier)));
}

/// Asserts that a label edge and a barrier edge come through with their origins.
#[test]
fn label_and_barrier() {
    let dataset = fixture();

    let after_all = reasons(&dataset, "first (batch 0)", "second (batch 1)");
    let barrier = reasons(&dataset, "second (batch 1)", "third (batch 2)");

    assert!(matches!(
        after_all,
        [Reason::Ordering(OrderingOrigin::AfterAll(_))]
    ));
    assert_eq!(barrier, [Reason::Ordering(OrderingOrigin::Barrier)]);
}

/// Asserts that consecutive children of a sequential workload are linked by a sequential origin.
#[test]
fn sequential() {
    let dataset = fixture();
    let sequential = workload(&dataset, "sequential");

    let links = links(&dataset, sequential);

    assert_eq!(links.len(), 2);
    let ((blocker, blocked), reasons) = dataset
        .conflicts
        .edges()
        .find(|((blocker, _), _)| dataset.location(*blocker).0 == sequential)
        .unwrap();
    assert_eq!(dataset.location(blocked).1, dataset.location(blocker).1 + 1);
    assert_eq!(reasons, [Reason::Ordering(OrderingOrigin::Sequential)]);
}

/// Asserts that the entry and pending regroups of one set are distinct operations, linked to the
/// exact occurrences around them, and that the two appearances of `write_a` keep their own links.
#[test]
fn regroups() {
    let dataset = fixture();
    let regroups = workload(&dataset, "regroups");

    let in_workload: Vec<_> = dataset
        .regroups
        .iter()
        .filter(|regroup| regroup.workload == regroups)
        .collect();
    assert!(in_workload.len() >= 2);
    assert_ne!(in_workload[0].batch, in_workload[1].batch);
    assert_ne!(in_workload[0].trigger, in_workload[1].trigger);

    let writers: Vec<_> = dataset
        .occurrences
        .iter()
        .enumerate()
        .filter(|(_, occurrence)| {
            occurrence.workload == regroups && dataset.system(occurrence.system).label == "write_a"
        })
        .map(|(index, _)| OperationId::System(crate::model::OccurrenceId(index)))
        .collect();
    assert_eq!(writers.len(), 2);
    let first = dataset.conflicts.blockers(writers[0]);
    let second = dataset.conflicts.blockers(writers[1]);
    assert_ne!(first, second);
    // The first write waits for the entry regroup, through the export's own regroup reference.
    let [OperationId::Regroup(entry)] = first[..] else {
        panic!("expected the entry regroup, got {first:?}");
    };
    let reasons = dataset.conflicts.reasons(first[0], writers[0]).unwrap();
    assert!(reasons.contains(&Reason::Ordering(OrderingOrigin::Regroup)));
    assert_eq!(
        dataset.regroup(entry).trigger,
        shipyard::scheduler::info::RegroupTrigger::Entry
    );
}
