//! How every record relates to the highlighted one, computed once per highlight from the
//! dataset's indexes so views only look relations up.

use crate::inspectors::Record;
use crate::model::{ComponentId, Dataset, SystemId};
use shipyard::borrow::Mutability;

/// How a record relates to the highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// It's the highlighted record.
    Target,
    /// A system borrowing the highlighted component exclusively, or a component the highlighted
    /// system borrows exclusively.
    Exclusive,
    Shared,
    /// Another record of the target's kind: systems while a system is highlighted, components
    /// while a component is.
    SameKind,
    /// A record of the other kind with no borrow between it and the target.
    Unrelated,
}

/// The relation of every system and component to `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relations {
    pub target: Record,
    /// Indexed by [`SystemId`].
    systems: Vec<Relation>,
    /// Indexed by [`ComponentId`].
    components: Vec<Relation>,
}

impl Relations {
    pub fn new(dataset: &Dataset, target: Record) -> Relations {
        let mut systems = vec![Relation::Unrelated; dataset.systems.len()];
        let mut components = vec![Relation::Unrelated; dataset.components.len()];

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
            }
        }

        Relations {
            target,
            systems,
            components,
        }
    }

    pub fn system(&self, system: SystemId) -> Relation {
        self.systems[system.0]
    }

    pub fn component(&self, component: ComponentId) -> Relation {
        self.components[component.0]
    }

    pub fn record(&self, record: Record) -> Relation {
        match record {
            Record::System(system) => self.system(system),
            Record::Component(component) => self.component(component),
        }
    }
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
        ], "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 0},
        {"name": "System(reader)", "borrow": [
            {"name": "A", "mutability": "Shared", "thread_safe": true}
        ], "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 1},
        {"name": "System(idle)", "borrow": [],
            "conflict": null, "after": [], "after_all": [], "before_all": [], "unique_id": 2}
    ]]}]}}"#;

    fn dataset() -> Dataset {
        Dataset::new(&parse_workloads(DATASET).unwrap())
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

    /// Asserts that a shared borrow doesn't hide an exclusive one of the same storage.
    #[test]
    fn exclusive_wins() {
        let mut relation = Relation::Unrelated;

        merge(&mut relation, Mutability::Exclusive);
        merge(&mut relation, Mutability::Shared);

        assert_eq!(relation, Relation::Exclusive);
    }
}
