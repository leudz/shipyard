//! The words explaining a link: why its blocked operation couldn't run in the batch of its blocker.
//!
//! Every reason the dataset has is listed, grouped by kind.

use crate::conflicts::{is_all_storages, Reason};
use crate::facts::Access;
use crate::model::{Dataset, OperationId};
use crate::names::unique_labels;
use shipyard::borrow::Mutability;
use shipyard::scheduler::info::{OrderingOrigin, RegroupTrigger};

/// What a link's tooltip says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// Names both ends.
    pub title: String,
    /// Kinds of reasons, only the ones with reasons, always in the same order.
    pub groups: Vec<Group>,
    /// What the ends are, when they're regroups.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub heading: &'static str,
    pub lines: Vec<String>,
}

/// Explains why `blocked` couldn't run alongside `blocker`, with `reasons` from
/// [`crate::conflicts::ConflictGraph::reasons`].
pub fn explain(
    dataset: &Dataset,
    blocker: OperationId,
    blocked: OperationId,
    reasons: &[Reason],
) -> Explanation {
    let title = format!(
        "{} can't run alongside {}",
        dataset.operation_label(blocked),
        dataset.operation_label(blocker)
    );

    let mut borrows = Vec::new();
    let mut ordering = Vec::new();
    let mut thread = Vec::new();
    for reason in reasons {
        match reason {
            Reason::Borrow {
                blocker: blocker_access,
                blocked: blocked_access,
            } => borrows.push(borrow_line(dataset, blocker_access, blocked_access)),
            Reason::Ordering(origin) => ordering.push(ordering_line(origin)),
            Reason::ThreadSlot => thread.push(
                "Both need the calling thread, a batch only has one slot for it.".to_string(),
            ),
        }
    }

    let groups = [
        ("Borrows", borrows),
        ("Ordering", ordering),
        ("Thread", thread),
    ]
    .into_iter()
    .filter(|(_, lines)| !lines.is_empty())
    .map(|(heading, lines)| Group { heading, lines })
    .collect();

    let mut notes = Vec::new();
    for operation in [blocker, blocked] {
        if let OperationId::Regroup(regroup) = operation {
            let label = dataset.operation_label(operation);
            let trigger = match dataset.regroup(regroup).trigger {
                RegroupTrigger::Entry => {
                    "an entry regroup, it prepares its storages for changes made outside the workload"
                }
                RegroupTrigger::Pending => {
                    "a pending regroup, placed after systems borrowing its storages exclusively"
                }
            };
            notes.push(format!("{label} is {trigger}."));
        }
    }

    Explanation {
        title,
        groups,
        notes,
    }
}

/// `A: exclusive by the blocker, shared in the blocked one's condition`.
fn borrow_line(dataset: &Dataset, blocker: &Access, blocked: &Access) -> String {
    // Both accesses are of the same storage, except when one is `AllStorages`.
    let storages = if blocker.storage == blocked.storage {
        storage_label(dataset, &blocker.storage)
    } else {
        format!(
            "{} and {}",
            storage_label(dataset, &blocker.storage),
            storage_label(dataset, &blocked.storage)
        )
    };

    format!(
        "{storages}: {} by the blocker, {} by the blocked one",
        access(blocker),
        access(blocked)
    )
}

/// How an access borrows, saying when it's a condition's.
fn access(access: &Access) -> String {
    let word = |mutability| match mutability {
        Mutability::Exclusive => "exclusive",
        Mutability::Shared => "shared",
    };

    match (access.body, access.condition) {
        (Some(body), None) => word(body).to_string(),
        (None, Some(condition)) => format!("{} in its condition", word(condition)),
        (Some(body), Some(condition)) => {
            format!("{}, {} in its condition", word(body), word(condition))
        }
        (None, None) => word(access.mutability).to_string(),
    }
}

/// The storage's component label, or a short label for storages no component has.
fn storage_label(dataset: &Dataset, storage: &str) -> String {
    if is_all_storages(storage) {
        return "AllStorages (every storage)".to_string();
    }

    let component = dataset
        .components
        .iter()
        .find(|component| component.name == storage);

    match component {
        Some(component) => component.label.clone(),
        None => unique_labels(&[storage]).remove(0),
    }
}

fn ordering_line(origin: &OrderingOrigin) -> String {
    match origin {
        OrderingOrigin::Implicit => {
            "Added earlier to the workload, with a borrow they can't share.".to_string()
        }
        OrderingOrigin::Barrier => "A barrier separates them.".to_string(),
        OrderingOrigin::BeforeAll(label) => format!("The blocker is before_all {label}."),
        OrderingOrigin::AfterAll(label) => format!("The blocked one is after_all {label}."),
        OrderingOrigin::Sequential => {
            "They're consecutive steps of a sequential workload.".to_string()
        }
        OrderingOrigin::Before => "The blocker's before list names the blocked one.".to_string(),
        OrderingOrigin::Regroup => {
            "The blocked one borrows storages the blocker regroups.".to_string()
        }
        OrderingOrigin::Writes => {
            "The blocked regroup moves storages the blocker may have changed.".to_string()
        }
        OrderingOrigin::Borrows => {
            "The blocked regroup moves storages the blocker borrows.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::add_occurrences;
    use crate::model::{OccurrenceId, RegroupId};
    use serde_json::{json, Value};

    fn load(batches: Value) -> Dataset {
        let mut batches = batches.as_array().unwrap().clone();
        add_occurrences(&mut batches);
        let workloads = json!({"w": {"name": "w", "batches_info": batches}});

        Dataset::new(&serde_json::from_value(workloads).unwrap()).unwrap()
    }

    /// A system with occurrence `id`, borrowing `borrow` and whose condition borrows `condition`.
    fn system(name: &str, id: usize, borrow: Value, condition: Value, after: Value) -> Value {
        let mut footprint: Vec<Value> = borrow.as_array().unwrap().clone();
        footprint.extend(condition.as_array().unwrap().iter().cloned());

        json!({
            "name": name, "borrow": borrow, "conflict": null,
            "after": [], "after_all": [], "before_all": [], "unique_id": id,
            "occurrence": {
                "id": id, "conditions": condition, "footprint": footprint,
                "thread_bound": false, "after": after,
            },
        })
    }

    fn storage(name: &str, mutability: &str) -> Value {
        json!({"name": name, "mutability": mutability, "thread_safe": true})
    }

    fn sys(index: usize) -> OperationId {
        OperationId::System(OccurrenceId(index))
    }

    fn explanation(dataset: &Dataset, blocker: OperationId, blocked: OperationId) -> Explanation {
        let reasons = dataset.conflicts.reasons(blocker, blocked).unwrap();

        explain(dataset, blocker, blocked, reasons)
    }

    fn lines<'a>(explanation: &'a Explanation, heading: &str) -> Vec<&'a str> {
        explanation
            .groups
            .iter()
            .find(|group| group.heading == heading)
            .map(|group| group.lines.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Asserts that a pair with a borrow conflict and two ordering origins lists all three, grouped,
    /// and names both ends.
    #[test]
    fn several_origins() {
        let dataset = load(json!([
            {"systems": [null, [system("System(w::write)", 0, json!([storage("w::A", "Exclusive")]), json!([]), json!([]))]]},
            {"systems": [null, [system("System(w::read)", 1, json!([storage("w::A", "Shared")]), json!([]), json!([
                {"occurrence": {"System": 0}, "origins": ["Implicit", {"BeforeAll": "Label(first)"}]},
            ]))]]},
        ]));

        let explanation = explanation(&dataset, sys(0), sys(1));

        assert_eq!(
            explanation.title,
            "read (batch 1) can't run alongside write (batch 0)"
        );
        assert_eq!(
            lines(&explanation, "Borrows"),
            ["A: exclusive by the blocker, shared by the blocked one"]
        );
        assert_eq!(
            lines(&explanation, "Ordering"),
            [
                "Added earlier to the workload, with a borrow they can't share.",
                "The blocker is before_all Label(first)."
            ]
        );
        assert!(explanation.notes.is_empty());
    }

    /// Asserts that a condition's borrow is described as the condition's.
    #[test]
    fn condition_access() {
        let dataset = load(json!([
            {"systems": [null, [system("System(w::write)", 0, json!([storage("w::A", "Exclusive")]), json!([]), json!([]))]]},
            {"systems": [null, [system("System(w::guarded)", 1, json!([]), json!([storage("w::A", "Shared")]), json!([]))]]},
        ]));

        let explanation = explanation(&dataset, sys(0), sys(1));

        assert_eq!(
            lines(&explanation, "Borrows"),
            ["A: exclusive by the blocker, shared in its condition by the blocked one"]
        );
    }

    /// Asserts that two systems competing for the calling thread get a thread reason.
    #[test]
    fn thread_slot() {
        let bound = |name: &str, storage_name: &str| {
            json!({
                "name": name, "borrow": [{"name": storage_name, "mutability": "Shared", "thread_safe": false}],
                "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 0,
            })
        };
        let dataset = load(json!([
            {"systems": [bound("System(w::a)", "w::A"), []]},
            {"systems": [bound("System(w::b)", "w::B"), []]},
        ]));

        let explanation = explanation(&dataset, sys(0), sys(1));

        assert_eq!(
            lines(&explanation, "Thread"),
            ["Both need the calling thread, a batch only has one slot for it."]
        );
        assert!(lines(&explanation, "Borrows").is_empty());
    }

    /// Asserts that `AllStorages` is named as reaching every storage, next to the other storage.
    #[test]
    fn all_storages() {
        let all = std::any::type_name::<shipyard::all_storages::AllStorages>();
        let dataset = load(json!([
            {"systems": [null, [system("System(w::all)", 0, json!([storage(all, "Exclusive")]), json!([]), json!([]))]]},
            {"systems": [null, [system("System(w::read)", 1, json!([storage("w::B", "Shared")]), json!([]), json!([]))]]},
        ]));

        let explanation = explanation(&dataset, sys(0), sys(1));

        assert_eq!(
            lines(&explanation, "Borrows"),
            ["AllStorages (every storage) and B: exclusive by the blocker, shared by the blocked one"]
        );
    }

    /// Asserts that regroup ends are described by their trigger, and the regroup origins worded
    /// without claiming changes happened.
    #[test]
    fn regroups() {
        let regroup = |id: usize, trigger: &str, after: Value| {
            json!({
                "borrow": [storage("w::A", "Exclusive")], "trigger": trigger,
                "occurrence": {"id": id, "thread_bound": false, "after": after},
            })
        };
        let dataset = load(json!([
            {"systems": [null, []], "regroups": [regroup(0, "Entry", json!([]))]},
            {"systems": [null, [system("System(w::write)", 0, json!([storage("w::A", "Exclusive")]), json!([]),
                json!([{"occurrence": {"Regroup": 0}, "origins": ["Regroup"]}]))]]},
            {"systems": [null, []], "regroups": [regroup(1, "Pending",
                json!([{"occurrence": {"System": 0}, "origins": ["Writes"]}]))]},
        ]));
        let (entry, writer, pending) = (
            OperationId::Regroup(RegroupId(0)),
            sys(0),
            OperationId::Regroup(RegroupId(1)),
        );

        let first = explanation(&dataset, entry, writer);
        let second = explanation(&dataset, writer, pending);

        assert_eq!(
            lines(&first, "Ordering"),
            ["The blocked one borrows storages the blocker regroups."]
        );
        assert!(first.notes[0].contains("an entry regroup"));
        assert_eq!(
            lines(&second, "Ordering"),
            ["The blocked regroup moves storages the blocker may have changed."]
        );
        assert!(second.notes[0].contains("a pending regroup"));
    }
}
