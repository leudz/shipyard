//! How every record relates to the highlighted one, computed once per highlight from the
//! dataset's indexes so views only look relations up.

use crate::inspectors::Record;
use crate::model::{ComponentId, Dataset, OperationId, RegroupId, SystemId, WorkloadId};
use shipyard::borrow::Mutability;

/// How a record relates to the highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// It's the highlighted record.
    Target,
    /// A system or regroup borrowing the highlighted component exclusively, or a component the
    /// highlighted system or regroup borrows exclusively.
    Exclusive,
    Shared,
    /// Another record of the target's kind: systems while a system is highlighted, components
    /// while a component is, regroups while a regroup is.
    SameKind,
    /// A record of the other kind with no borrow between it and the target.
    Unrelated,
}

/// The relation of every system, component and regroup to `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relations {
    pub target: Record,
    /// Indexed by [`SystemId`].
    systems: Vec<Relation>,
    /// Indexed by [`ComponentId`].
    components: Vec<Relation>,
    /// Indexed by [`RegroupId`].
    regroups: Vec<Relation>,
}

impl Relations {
    pub fn new(dataset: &Dataset, target: Record) -> Relations {
        let mut systems = vec![Relation::Unrelated; dataset.systems.len()];
        let mut components = vec![Relation::Unrelated; dataset.components.len()];
        let mut regroups = vec![Relation::Unrelated; dataset.regroups.len()];

        match target {
            Record::System(system) => {
                systems.fill(Relation::SameKind);
                systems[system.0] = Relation::Target;
                for borrow in &dataset.system(system).borrows {
                    merge(&mut components[borrow.component.0], borrow.mutability);
                }
            }
            Record::Component(component) => {
                components.fill(Relation::SameKind);
                components[component.0] = Relation::Target;
                for borrower in &dataset.component(component).borrowers {
                    merge(&mut systems[borrower.system.0], borrower.mutability);
                }
                // Regroups always borrow their storages exclusively.
                for regroup in &dataset.component(component).regroups {
                    merge(&mut regroups[regroup.0], Mutability::Exclusive);
                }
            }
            // Only this regroup: another regroup of the same storages is another record.
            Record::Regroup(regroup) => {
                regroups.fill(Relation::SameKind);
                regroups[regroup.0] = Relation::Target;
                for borrow in &dataset.regroup(regroup).borrows {
                    merge(&mut components[borrow.component.0], borrow.mutability);
                }
            }
        }

        Relations {
            target,
            systems,
            components,
            regroups,
        }
    }

    pub fn system(&self, system: SystemId) -> Relation {
        self.systems[system.0]
    }

    pub fn component(&self, component: ComponentId) -> Relation {
        self.components[component.0]
    }

    pub fn regroup(&self, regroup: RegroupId) -> Relation {
        self.regroups[regroup.0]
    }

    pub fn record(&self, record: Record) -> Relation {
        match record {
            Record::System(system) => self.system(system),
            Record::Component(component) => self.component(component),
            Record::Regroup(regroup) => self.regroup(regroup),
        }
    }

    /// The relation of the system or regroup `operation` runs.
    pub fn operation(&self, dataset: &Dataset, operation: OperationId) -> Relation {
        match operation {
            OperationId::System(occurrence) => self.system(dataset.occurrence(occurrence).system),
            OperationId::Regroup(regroup) => self.regroup(regroup),
        }
    }
}

/// The operations of `workload` whose conflict links are shown.
///
/// That's the selected operation when it's in `workload`, and every operation the highlight marks:
/// the highlighted system's appearances, a highlighted component's borrowers, a highlighted regroup.
/// Records only of the same kind as the target, or unrelated, aren't included.
pub fn link_roots(
    dataset: &Dataset,
    workload: WorkloadId,
    selected: Option<OperationId>,
    relations: Option<&Relations>,
) -> Vec<OperationId> {
    let mut roots = Vec::new();

    // A selection kept from another workload has no link here.
    if let Some(selected) = selected {
        if dataset.location(selected).0 == workload {
            roots.push(selected);
        }
    }

    if let Some(relations) = relations {
        let operations = dataset
            .workload(workload)
            .batches
            .iter()
            .flat_map(|batch| batch.operations());

        for operation in operations {
            let relation = relations.operation(dataset, operation);

            if matches!(
                relation,
                Relation::Target | Relation::Exclusive | Relation::Shared
            ) {
                roots.push(operation);
            }
        }
    }

    roots.sort();
    roots.dedup();

    roots
}

/// Records a borrow with `mutability`: exclusive wins over shared, when a system borrows the same
/// storage both ways.
fn merge(relation: &mut Relation, mutability: Mutability) {
    *relation = match (*relation, mutability) {
        (Relation::Exclusive, _) | (_, Mutability::Exclusive) => Relation::Exclusive,
        _ => Relation::Shared,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::parse_workloads;

    /// `writer` borrows A exclusively and B shared, `reader` borrows A shared, `idle` nothing.
    const DATASET: &[u8] = br#"{"w": {"name": "w", "batches_info": [{"systems": [null, [
        {"name": "System(writer)", "borrow": [
            {"name": "A", "mutability": "Exclusive", "thread_safe": true},
            {"name": "B", "mutability": "Shared", "thread_safe": true}
        ], "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 0,
        "occurrence": {"id": 0, "conditions": [], "footprint": [], "thread_bound": false, "after": []}},
        {"name": "System(reader)", "borrow": [
            {"name": "A", "mutability": "Shared", "thread_safe": true}
        ], "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 1,
        "occurrence": {"id": 1, "conditions": [], "footprint": [], "thread_bound": false, "after": []}},
        {"name": "System(idle)", "borrow": [],
            "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 2,
            "occurrence": {"id": 2, "conditions": [], "footprint": [], "thread_bound": false, "after": []}}
    ]], "regroups": []}]}}"#;

    fn dataset() -> Dataset {
        Dataset::new(&parse_workloads(DATASET).unwrap()).unwrap()
    }

    fn system(dataset: &Dataset, label: &str) -> SystemId {
        SystemId(
            dataset
                .systems
                .iter()
                .position(|system| system.label == label)
                .unwrap(),
        )
    }

    fn component(dataset: &Dataset, name: &str) -> ComponentId {
        ComponentId(
            dataset
                .components
                .iter()
                .position(|component| component.name == name)
                .unwrap(),
        )
    }

    /// Asserts that a highlighted component marks its exclusive and shared borrowers, and leaves
    /// the other systems unrelated.
    #[test]
    fn component_borrowers() {
        let dataset = dataset();
        let a = component(&dataset, "A");

        let relations = Relations::new(&dataset, Record::Component(a));

        assert_eq!(
            relations.system(system(&dataset, "writer")),
            Relation::Exclusive
        );
        assert_eq!(
            relations.system(system(&dataset, "reader")),
            Relation::Shared
        );
        assert_eq!(
            relations.system(system(&dataset, "idle")),
            Relation::Unrelated
        );
        assert_eq!(relations.component(a), Relation::Target);
        assert_eq!(
            relations.component(component(&dataset, "B")),
            Relation::SameKind
        );
    }

    /// Asserts that a highlighted system marks the components it borrows by access, and the
    /// other systems as the same kind.
    #[test]
    fn system_borrows() {
        let dataset = dataset();
        let writer = system(&dataset, "writer");

        let relations = Relations::new(&dataset, Record::System(writer));

        assert_eq!(relations.system(writer), Relation::Target);
        assert_eq!(
            relations.system(system(&dataset, "idle")),
            Relation::SameKind
        );
        assert_eq!(
            relations.component(component(&dataset, "A")),
            Relation::Exclusive
        );
        assert_eq!(
            relations.component(component(&dataset, "B")),
            Relation::Shared
        );
    }

    /// Asserts that a system borrowing nothing relates to no component.
    #[test]
    fn no_borrows() {
        let dataset = dataset();

        let relations = Relations::new(&dataset, Record::System(system(&dataset, "idle")));

        assert_eq!(
            relations.component(component(&dataset, "A")),
            Relation::Unrelated
        );
        assert_eq!(
            relations.component(component(&dataset, "B")),
            Relation::Unrelated
        );
    }

    /// Workload `w`: a regroup of A and Z, a system reading A, then a regroup of A and Z again.
    const REGROUPS: &[u8] = br#"{"w": {"name": "w", "batches_info": [
        {"systems": [null, []], "regroups": [{"borrow": [
            {"name": "A", "mutability": "Exclusive", "thread_safe": true},
            {"name": "Z", "mutability": "Exclusive", "thread_safe": true}
        ], "trigger": "Entry", "occurrence": {"id": 0, "thread_bound": false, "after": []}}]},
        {"systems": [null, [{"name": "System(reader)", "borrow": [
            {"name": "A", "mutability": "Shared", "thread_safe": true}
        ], "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 0,
        "occurrence": {"id": 0, "conditions": [], "footprint": [
            {"name": "A", "mutability": "Shared", "thread_safe": true}
        ], "thread_bound": false, "after": []}}]], "regroups": []},
        {"systems": [null, []], "regroups": [{"borrow": [
            {"name": "A", "mutability": "Exclusive", "thread_safe": true},
            {"name": "Z", "mutability": "Exclusive", "thread_safe": true}
        ], "trigger": "Pending", "occurrence": {"id": 1, "thread_bound": false, "after": []}}]}
    ]}}"#;

    fn regrouped() -> Dataset {
        Dataset::new(&parse_workloads(REGROUPS).unwrap()).unwrap()
    }

    fn regroup(index: usize) -> OperationId {
        OperationId::Regroup(RegroupId(index))
    }

    /// Asserts that a storage only regroups borrow marks those regroups as exclusive borrowers.
    #[test]
    fn regroup_only_component() {
        let dataset = regrouped();
        let z = component(&dataset, "Z");

        let relations = Relations::new(&dataset, Record::Component(z));

        assert_eq!(relations.regroup(RegroupId(0)), Relation::Exclusive);
        assert_eq!(relations.regroup(RegroupId(1)), Relation::Exclusive);
        assert_eq!(
            relations.system(system(&dataset, "reader")),
            Relation::Unrelated
        );
    }

    /// Asserts that a highlighted regroup is the only target, even with another regroup of the
    /// same storages, and marks its storages as exclusive.
    #[test]
    fn regroup_target() {
        let dataset = regrouped();

        let relations = Relations::new(&dataset, Record::Regroup(RegroupId(1)));

        assert_eq!(relations.regroup(RegroupId(1)), Relation::Target);
        assert_eq!(relations.regroup(RegroupId(0)), Relation::SameKind);
        assert_eq!(
            relations.component(component(&dataset, "A")),
            Relation::Exclusive
        );
        assert_eq!(
            relations.system(system(&dataset, "reader")),
            Relation::Unrelated
        );
    }

    /// Asserts that a highlighted component's roots are its system and regroup borrowers.
    #[test]
    fn component_roots() {
        let dataset = regrouped();
        let relations = Relations::new(&dataset, Record::Component(component(&dataset, "A")));

        let roots = link_roots(&dataset, WorkloadId(0), None, Some(&relations));

        assert_eq!(
            roots,
            [
                OperationId::System(crate::model::OccurrenceId(0)),
                regroup(0),
                regroup(1)
            ]
        );
    }

    /// Asserts that the same kind as the target isn't a root: a highlighted regroup is the only one.
    #[test]
    fn same_kind_roots() {
        let dataset = regrouped();
        let relations = Relations::new(&dataset, Record::Regroup(RegroupId(1)));

        let roots = link_roots(&dataset, WorkloadId(0), None, Some(&relations));

        assert_eq!(roots, [regroup(1)]);
    }

    /// Asserts that the selection is a root once, even when the highlight marks it too.
    #[test]
    fn selection_root() {
        let dataset = regrouped();
        let relations = Relations::new(&dataset, Record::Regroup(RegroupId(1)));

        let roots = link_roots(&dataset, WorkloadId(0), Some(regroup(1)), Some(&relations));

        assert_eq!(roots, [regroup(1)]);
    }

    /// Asserts that a shared borrow doesn't hide an exclusive one of the same storage.
    #[test]
    fn exclusive_wins() {
        let mut relation = Relation::Unrelated;

        merge(&mut relation, Mutability::Exclusive);
        merge(&mut relation, Mutability::Shared);

        assert_eq!(relation, Relation::Exclusive);
    }
}
