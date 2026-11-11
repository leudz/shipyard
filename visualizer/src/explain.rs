//! What explains a link: why its blocked operation couldn't run in the batch of its blocker.
//!
//! Every reason the dataset has is listed, each in a few words with a sentence for details.

use crate::conflicts::{is_all_storages, Reason};
use crate::facts::Access;
use crate::model::{ComponentId, Dataset, OperationId};
use crate::names::unique_labels;
use shipyard::borrow::Mutability;
use shipyard::scheduler::info::{OrderingOrigin, RegroupTrigger};

/// What a link's tooltip shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// The whole explanation in one sentence, naming both ends.
    pub summary: String,
    pub blocker: End,
    pub blocked: End,
    /// The storages both ends borrow in ways they can't share.
    pub borrows: Vec<BorrowRow>,
    /// Why the blocked one is ordered after the blocker.
    pub ordering: Vec<Tag>,
    /// Both need the calling thread.
    pub thread: Option<Tag>,
}

/// One end of a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct End {
    /// The system's label, or `Regroup` and its storages.
    pub name: String,
    /// A regroup's storages' labels, empty for systems.
    pub storages: Vec<String>,
    pub is_regroup: bool,
    pub batch: usize,
    /// What a regroup does, see [`regroup_purpose`].
    pub purpose: Option<&'static str>,
}

/// A storage both ends borrow, and how each does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowRow {
    /// One storage, or two when an end borrows `AllStorages`: the blocker's then the blocked one's.
    pub storages: Vec<Storage>,
    /// How both ends borrow it, in a sentence.
    pub detail: String,
    pub blocker: Vec<Mark>,
    pub blocked: Vec<Mark>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    pub label: String,
    /// The component it is, to inspect it. `None` for storages no component has.
    pub component: Option<ComponentId>,
}

/// One way an end borrows a storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub mutability: Mutability,
    /// The borrow is the condition's, not the system's.
    pub condition: bool,
}

impl Mark {
    /// `EX` or `SH`, as the highlight marks borrows.
    pub fn label(self) -> &'static str {
        match self.mutability {
            Mutability::Exclusive => "EX",
            Mutability::Shared => "SH",
        }
    }
}

/// A few words, and the sentence they stand for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub short: String,
    pub detail: String,
}

impl Tag {
    fn new(short: impl Into<String>, detail: impl Into<String>) -> Tag {
        Tag {
            short: short.into(),
            detail: detail.into(),
        }
    }
}

/// Explains why `blocked` couldn't run alongside `blocker`, with `reasons` from
/// [`crate::conflicts::ConflictGraph::reasons`].
pub fn explain(
    dataset: &Dataset,
    blocker: OperationId,
    blocked: OperationId,
    reasons: &[Reason],
) -> Explanation {
    let summary = format!(
        "{} can't run alongside {}",
        dataset.operation_label(blocked),
        dataset.operation_label(blocker)
    );

    let mut borrows = Vec::new();
    let mut ordering = Vec::new();
    let mut thread = None;
    for reason in reasons {
        match reason {
            Reason::Borrow {
                blocker: blocker_access,
                blocked: blocked_access,
            } => borrows.push(borrow_row(dataset, blocker_access, blocked_access)),
            Reason::Ordering(origin) => ordering.push(ordering_tag(origin)),
            Reason::ThreadSlot => {
                thread = Some(Tag::new(
                    "calling thread",
                    "Both need the calling thread, a batch only has one slot for it.",
                ))
            }
        }
    }

    Explanation {
        summary,
        blocker: end(dataset, blocker),
        blocked: end(dataset, blocked),
        borrows,
        ordering,
        thread,
    }
}

fn end(dataset: &Dataset, operation: OperationId) -> End {
    let (_, batch) = dataset.location(operation);

    match operation {
        OperationId::System(occurrence) => End {
            name: dataset
                .system(dataset.occurrence(occurrence).system)
                .label
                .clone(),
            storages: Vec::new(),
            is_regroup: false,
            batch,
            purpose: None,
        },
        OperationId::Regroup(regroup) => End {
            name: format!("Regroup {}", dataset.regroup_label(regroup)),
            storages: dataset
                .regroup(regroup)
                .borrows
                .iter()
                .map(|borrow| dataset.component(borrow.component).label.clone())
                .collect(),
            is_regroup: true,
            batch,
            purpose: Some(regroup_purpose(dataset.regroup(regroup).trigger)),
        },
    }
}

/// What a regroup does, depending on why the workload added it.
pub fn regroup_purpose(trigger: RegroupTrigger) -> &'static str {
    match trigger {
        RegroupTrigger::Entry => "Restores the group after changes made outside the workload.",
        RegroupTrigger::Pending => "Restores the group after exclusive borrows.",
    }
}

fn borrow_row(dataset: &Dataset, blocker: &Access, blocked: &Access) -> BorrowRow {
    // Both accesses are of the same storage, except when one is `AllStorages`.
    let storages: Vec<Storage> = if blocker.storage == blocked.storage {
        vec![storage(dataset, &blocker.storage)]
    } else {
        vec![
            storage(dataset, &blocker.storage),
            storage(dataset, &blocked.storage),
        ]
    };
    let labels: Vec<&str> = storages
        .iter()
        .map(|storage| storage.label.as_str())
        .collect();
    let labels = labels.join(", ");
    let all_storages = if is_all_storages(&blocker.storage) || is_all_storages(&blocked.storage) {
        " AllStorages borrows every storage."
    } else {
        ""
    };
    let detail = format!(
        "{labels}: {} by the blocker, {} by the blocked one.{all_storages}",
        access(blocker),
        access(blocked)
    );

    BorrowRow {
        storages,
        detail,
        blocker: marks(blocker),
        blocked: marks(blocked),
    }
}

/// How an access borrows: its body's borrow, then its condition's.
fn marks(access: &Access) -> Vec<Mark> {
    let body = access.body.map(|mutability| Mark {
        mutability,
        condition: false,
    });
    let condition = access.condition.map(|mutability| Mark {
        mutability,
        condition: true,
    });

    match (body, condition) {
        (None, None) => vec![Mark {
            mutability: access.mutability,
            condition: false,
        }],
        (body, condition) => body.into_iter().chain(condition).collect(),
    }
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

/// The storage's component, labelled like it, or a short label for storages no component has.
fn storage(dataset: &Dataset, storage: &str) -> Storage {
    let component = dataset
        .components
        .iter()
        .position(|component| component.name == storage)
        .map(ComponentId);
    let label = if is_all_storages(storage) {
        "AllStorages".to_string()
    } else {
        match component {
            Some(component) => dataset.component(component).label.clone(),
            None => unique_labels(&[storage]).remove(0),
        }
    };

    Storage { label, component }
}

fn ordering_tag(origin: &OrderingOrigin) -> Tag {
    match origin {
        OrderingOrigin::Implicit => Tag::new(
            "added earlier",
            "Added earlier to the workload, with a borrow they can't share.",
        ),
        OrderingOrigin::Barrier => Tag::new("barrier", "A barrier separates them."),
        OrderingOrigin::BeforeAll(label) => Tag::new(
            format!("before_all {label}"),
            format!("The blocker is before_all {label}."),
        ),
        OrderingOrigin::AfterAll(label) => Tag::new(
            format!("after_all {label}"),
            format!("The blocked one is after_all {label}."),
        ),
        OrderingOrigin::Sequential => Tag::new(
            "sequential",
            "They're consecutive steps of a sequential workload.",
        ),
        OrderingOrigin::Before => {
            Tag::new("before", "The blocker's before list names the blocked one.")
        }
        OrderingOrigin::Regroup => Tag::new(
            "regroup",
            "The blocked one borrows storages the blocker regroups.",
        ),
        OrderingOrigin::Writes => Tag::new(
            "writes",
            "The blocked regroup moves storages the blocker may have changed.",
        ),
        OrderingOrigin::Borrows => Tag::new(
            "borrows",
            "The blocked regroup moves storages the blocker borrows.",
        ),
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

    fn shorts(tags: &[Tag]) -> Vec<&str> {
        tags.iter().map(|tag| tag.short.as_str()).collect()
    }

    fn details(tags: &[Tag]) -> Vec<&str> {
        tags.iter().map(|tag| tag.detail.as_str()).collect()
    }

    const EX: Mark = Mark {
        mutability: Mutability::Exclusive,
        condition: false,
    };
    const SH: Mark = Mark {
        mutability: Mutability::Shared,
        condition: false,
    };

    /// Asserts that a pair with a borrow conflict and two ordering origins lists all three, and
    /// names both ends.
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
            explanation.summary,
            "read (batch 1) can't run alongside write (batch 0)"
        );
        assert_eq!(
            (explanation.blocker.name.as_str(), explanation.blocker.batch),
            ("write", 0)
        );
        assert_eq!(
            (explanation.blocked.name.as_str(), explanation.blocked.batch),
            ("read", 1)
        );
        assert_eq!(
            explanation.borrows,
            [BorrowRow {
                storages: vec![Storage {
                    label: "A".to_string(),
                    component: Some(ComponentId(0)),
                }],
                detail: "A: exclusive by the blocker, shared by the blocked one.".to_string(),
                blocker: vec![EX],
                blocked: vec![SH],
            }]
        );
        assert_eq!(
            shorts(&explanation.ordering),
            ["added earlier", "before_all Label(first)"]
        );
        assert_eq!(
            details(&explanation.ordering),
            [
                "Added earlier to the workload, with a borrow they can't share.",
                "The blocker is before_all Label(first)."
            ]
        );
        assert_eq!(explanation.thread, None);
    }

    /// Asserts that a condition's borrow is marked as the condition's.
    #[test]
    fn condition_access() {
        let dataset = load(json!([
            {"systems": [null, [system("System(w::write)", 0, json!([storage("w::A", "Exclusive")]), json!([]), json!([]))]]},
            {"systems": [null, [system("System(w::guarded)", 1, json!([]), json!([storage("w::A", "Shared")]), json!([]))]]},
        ]));

        let explanation = explanation(&dataset, sys(0), sys(1));

        let row = &explanation.borrows[0];
        assert_eq!(
            row.blocked,
            [Mark {
                mutability: Mutability::Shared,
                condition: true
            }]
        );
        assert_eq!(
            row.detail,
            "A: exclusive by the blocker, shared in its condition by the blocked one."
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

        assert_eq!(explanation.thread.unwrap().short, "calling thread");
        assert!(explanation.borrows.is_empty());
    }

    /// Asserts that `AllStorages` is named next to the other storage, its detail saying it reaches
    /// every storage.
    #[test]
    fn all_storages() {
        let all = std::any::type_name::<shipyard::all_storages::AllStorages>();
        let dataset = load(json!([
            {"systems": [null, [system("System(w::all)", 0, json!([storage(all, "Exclusive")]), json!([]), json!([]))]]},
            {"systems": [null, [system("System(w::read)", 1, json!([storage("w::B", "Shared")]), json!([]), json!([]))]]},
        ]));

        let explanation = explanation(&dataset, sys(0), sys(1));

        let row = &explanation.borrows[0];
        let labels: Vec<&str> = row.storages.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["AllStorages", "B"]);
        assert_eq!(
            row.detail,
            "AllStorages, B: exclusive by the blocker, shared by the blocked one. AllStorages \
             borrows every storage."
        );
    }

    /// Asserts that regroup ends are named by their storages and say what they do, and the regroup origins
    /// worded without claiming changes happened.
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

        assert_eq!(first.blocker.name, "Regroup A");
        assert_eq!(first.blocker.storages, ["A"]);
        assert!(first.blocker.is_regroup);
        assert_eq!(
            first.blocker.purpose,
            Some("Restores the group after changes made outside the workload.")
        );
        assert_eq!(first.blocked.purpose, None);
        assert_eq!(
            details(&first.ordering),
            ["The blocked one borrows storages the blocker regroups."]
        );
        assert_eq!(
            second.blocked.purpose,
            Some("Restores the group after exclusive borrows.")
        );
        assert_eq!(
            details(&second.ordering),
            ["The blocked regroup moves storages the blocker may have changed."]
        );
    }
}
