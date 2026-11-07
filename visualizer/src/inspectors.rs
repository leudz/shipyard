//! Inspectors and the global map: the records inspectors show, their histories, and the
//! stacking order of all of them.
//!
//! Inspecting a record always goes to the single unpinned inspector, so pinned inspectors keep
//! what they show until they're closed. The map is always pinned, and there's at most one.

use crate::model::{ComponentId, SystemId};

/// How many records an inspector's history keeps, older ones are dropped.
pub const HISTORY_LIMIT: usize = 20;

/// A record an inspector can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Record {
    System(SystemId),
    Component(ComponentId),
}

/// Identifies an inspector or the map while it's open, views can use it as a key.
///
/// Ids grow with each opened panel, so sorting by id gives the opening order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InspectorId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspector {
    pub id: InspectorId,
    /// Pinned inspectors keep their record when another one is inspected.
    pub pinned: bool,
    /// The records this inspector showed, oldest first. Never empty.
    history: Vec<Record>,
    /// Which record of `history` is shown.
    current: usize,
}

impl Inspector {
    /// The record shown.
    pub fn record(&self) -> Record {
        self.history[self.current]
    }

    pub fn can_go_back(&self) -> bool {
        self.current > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.current + 1 < self.history.len()
    }

    /// The shown record's position in the history, counting from 1, and the history's length.
    pub fn position(&self) -> (usize, usize) {
        (self.current + 1, self.history.len())
    }

    fn new(id: InspectorId, record: Record) -> Inspector {
        Inspector {
            id,
            pinned: false,
            history: vec![record],
            current: 0,
        }
    }

    /// Shows `record`, adding it to the history unless it's already shown.
    fn show(&mut self, record: Record) {
        if record == self.record() {
            return;
        }

        // Like in a browser, going somewhere new after Back drops the records we went back from.
        self.history.truncate(self.current + 1);
        self.history.push(record);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }

        self.current = self.history.len() - 1;
    }
}

/// What can be done to one open panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorAction {
    /// Draws it over the other panels.
    BringToFront,
    /// Inspectors only, the map has no history.
    Back,
    Forward,
    /// Pins an inspector, or closes it when it's already pinned. The map is always pinned.
    TogglePin,
    Close,
}

/// An open panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenPanel {
    Inspector(Inspector),
    /// The global map, listing every system and component.
    Map(InspectorId),
}

impl OpenPanel {
    pub fn id(&self) -> InspectorId {
        match self {
            OpenPanel::Inspector(inspector) => inspector.id,
            OpenPanel::Map(id) => *id,
        }
    }

    /// The inspector, `None` for the map.
    pub fn inspector(&self) -> Option<&Inspector> {
        match self {
            OpenPanel::Inspector(inspector) => Some(inspector),
            OpenPanel::Map(_) => None,
        }
    }
}

/// The open inspectors and map, at most one unpinned inspector and one map.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Inspectors {
    /// Back to front: later panels are drawn over earlier ones.
    open: Vec<OpenPanel>,
    /// The id the next opened panel gets.
    next_id: u64,
}

impl Inspectors {
    /// The open panels, back to front.
    pub fn open(&self) -> &[OpenPanel] {
        &self.open
    }

    /// Shows `record` in the unpinned inspector, opening one if needed, and brings it to the front.
    ///
    /// When an inspector already shows `record`, pinned or not, it's brought to the front instead.
    pub fn inspect(&mut self, record: Record) {
        let showing = self.position(|inspector| inspector.record() == record);
        if let Some(index) = showing {
            self.bring_to_front(index);
            return;
        }

        let Some(index) = self.position(|inspector| !inspector.pinned) else {
            let id = self.next_id();
            self.open
                .push(OpenPanel::Inspector(Inspector::new(id, record)));

            return;
        };

        if let OpenPanel::Inspector(inspector) = &mut self.open[index] {
            inspector.show(record);
        }
        self.bring_to_front(index);
    }

    /// Opens the map, or brings it to the front when it's already open.
    pub fn open_map(&mut self) {
        let open = self
            .open
            .iter()
            .position(|panel| matches!(panel, OpenPanel::Map(_)));

        match open {
            Some(index) => self.bring_to_front(index),
            None => {
                let id = self.next_id();
                self.open.push(OpenPanel::Map(id));
            }
        }
    }

    /// Applies `action` to the panel `id`, if it's still open.
    pub fn update(&mut self, id: InspectorId, action: InspectorAction) {
        let Some(index) = self.open.iter().position(|panel| panel.id() == id) else {
            return;
        };

        match action {
            InspectorAction::BringToFront => self.bring_to_front(index),
            InspectorAction::Close => {
                self.open.remove(index);
            }
            InspectorAction::Back | InspectorAction::Forward | InspectorAction::TogglePin => {
                self.update_inspector(index, action);
            }
        }
    }

    /// Applies an inspector-only action to the panel at `index`, nothing for the map.
    fn update_inspector(&mut self, index: usize, action: InspectorAction) {
        let OpenPanel::Inspector(inspector) = &mut self.open[index] else {
            return;
        };

        match action {
            InspectorAction::Back => {
                if inspector.can_go_back() {
                    inspector.current -= 1;
                }
            }
            InspectorAction::Forward => {
                if inspector.can_go_forward() {
                    inspector.current += 1;
                }
            }
            // Unpinning closes it: another unpinned inspector may be open, and there can only be one.
            InspectorAction::TogglePin if inspector.pinned => {
                self.open.remove(index);
            }
            InspectorAction::TogglePin => inspector.pinned = true,
            InspectorAction::BringToFront | InspectorAction::Close => {}
        }
    }

    /// The index of the first inspector matching `predicate`.
    fn position(&self, predicate: impl Fn(&Inspector) -> bool) -> Option<usize> {
        self.open
            .iter()
            .position(|panel| panel.inspector().is_some_and(&predicate))
    }

    fn next_id(&mut self) -> InspectorId {
        let id = InspectorId(self.next_id);
        self.next_id += 1;

        id
    }

    fn bring_to_front(&mut self, index: usize) {
        let panel = self.open.remove(index);
        self.open.push(panel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Record = Record::System(SystemId(0));
    const B: Record = Record::System(SystemId(1));
    const C: Record = Record::Component(ComponentId(0));

    /// The record each open inspector shows, back to front.
    fn records(inspectors: &Inspectors) -> Vec<Record> {
        let open = inspectors.open().iter();
        open.filter_map(OpenPanel::inspector)
            .map(Inspector::record)
            .collect()
    }

    /// The inspector at `index`, back to front.
    fn nth(inspectors: &Inspectors, index: usize) -> &Inspector {
        inspectors.open()[index].inspector().unwrap()
    }

    /// The id of the inspector at `index`, back to front.
    fn id(inspectors: &Inspectors, index: usize) -> InspectorId {
        inspectors.open()[index].id()
    }

    /// Inspectors that inspected `records` in order.
    fn inspected(records: &[Record]) -> Inspectors {
        let mut inspectors = Inspectors::default();
        for &record in records {
            inspectors.inspect(record);
        }

        inspectors
    }

    /// Asserts that inspecting a record opens an unpinned inspector showing it.
    #[test]
    fn first() {
        let inspectors = inspected(&[A]);

        assert_eq!(records(&inspectors), [A]);
        assert!(!nth(&inspectors, 0).pinned);
        assert_eq!(nth(&inspectors, 0).position(), (1, 1));
    }

    /// Asserts that inspecting another record reuses the unpinned inspector and adds to its history.
    #[test]
    fn reuse() {
        let inspectors = inspected(&[A, B]);

        assert_eq!(records(&inspectors), [B]);
        assert_eq!(nth(&inspectors, 0).position(), (2, 2));
    }

    /// Asserts that systems and components share one history.
    #[test]
    fn mixed() {
        let mut inspectors = inspected(&[A, C, B]);
        let inspector = id(&inspectors, 0);

        inspectors.update(inspector, InspectorAction::Back);
        assert_eq!(records(&inspectors), [C]);

        inspectors.update(inspector, InspectorAction::Back);
        assert_eq!(records(&inspectors), [A]);
    }

    /// Asserts that inspecting the shown record again doesn't add it to the history.
    #[test]
    fn duplicate() {
        let inspectors = inspected(&[A, A]);

        assert_eq!(nth(&inspectors, 0).position(), (1, 1));
    }

    /// Asserts that a record shown earlier in the history is added again, not jumped back to.
    #[test]
    fn revisit() {
        let inspectors = inspected(&[A, B, A]);

        assert_eq!(records(&inspectors), [A]);
        assert_eq!(nth(&inspectors, 0).position(), (3, 3));
    }

    /// Asserts that Back and Forward move through the history and stop at its ends.
    #[test]
    fn back_forward() {
        let mut inspectors = inspected(&[A, B]);
        let inspector = id(&inspectors, 0);

        inspectors.update(inspector, InspectorAction::Forward);
        assert_eq!(records(&inspectors), [B]);

        inspectors.update(inspector, InspectorAction::Back);
        inspectors.update(inspector, InspectorAction::Back);
        assert_eq!(records(&inspectors), [A]);
        assert!(!nth(&inspectors, 0).can_go_back());
        assert!(nth(&inspectors, 0).can_go_forward());

        inspectors.update(inspector, InspectorAction::Forward);
        assert_eq!(records(&inspectors), [B]);
        assert!(!nth(&inspectors, 0).can_go_forward());
    }

    /// Asserts that inspecting after Back drops the records after the shown one.
    #[test]
    fn branch() {
        let mut inspectors = inspected(&[A, B]);
        inspectors.update(id(&inspectors, 0), InspectorAction::Back);

        inspectors.inspect(C);

        assert_eq!(records(&inspectors), [C]);
        assert_eq!(nth(&inspectors, 0).position(), (2, 2));
        assert!(!nth(&inspectors, 0).can_go_forward());
    }

    /// Asserts that the history keeps only the latest records.
    #[test]
    fn limit() {
        let records: Vec<_> = (0..HISTORY_LIMIT + 5)
            .map(|index| Record::System(SystemId(index)))
            .collect();
        let mut inspectors = inspected(&records);
        let inspector = id(&inspectors, 0);

        for _ in 0..HISTORY_LIMIT {
            inspectors.update(inspector, InspectorAction::Back);
        }

        assert_eq!(nth(&inspectors, 0).position(), (1, HISTORY_LIMIT));
        assert_eq!(nth(&inspectors, 0).record(), Record::System(SystemId(5)));
    }

    /// Asserts that a pinned inspector keeps its record and the next record opens a new inspector.
    #[test]
    fn pin_then_inspect() {
        let mut inspectors = inspected(&[A]);
        inspectors.update(id(&inspectors, 0), InspectorAction::TogglePin);

        inspectors.inspect(B);
        inspectors.inspect(C);

        assert_eq!(records(&inspectors), [A, C]);
        assert!(nth(&inspectors, 0).pinned);
        assert!(!nth(&inspectors, 1).pinned);
    }

    /// Asserts that inspecting a pinned inspector's record brings it to the front, leaving the
    /// unpinned inspector and its history alone.
    #[test]
    fn pinned_duplicate() {
        let mut inspectors = inspected(&[A]);
        inspectors.update(id(&inspectors, 0), InspectorAction::TogglePin);
        inspectors.inspect(B);

        inspectors.inspect(A);

        assert_eq!(records(&inspectors), [B, A]);
        assert!(nth(&inspectors, 1).pinned);
        assert_eq!(nth(&inspectors, 0).position(), (1, 1));
    }

    /// Asserts that a record a pinned inspector went Back from opens in the unpinned inspector,
    /// only the shown record counts.
    #[test]
    fn pinned_history_duplicate() {
        let mut inspectors = inspected(&[A, B]);
        let pinned = id(&inspectors, 0);
        inspectors.update(pinned, InspectorAction::TogglePin);
        inspectors.update(pinned, InspectorAction::Back);

        inspectors.inspect(B);

        assert_eq!(records(&inspectors), [A, B]);
        assert!(!nth(&inspectors, 1).pinned);
    }

    /// Asserts that unpinning closes the inspector instead of leaving two unpinned ones.
    #[test]
    fn unpin_closes() {
        let mut inspectors = inspected(&[A]);
        let pinned = id(&inspectors, 0);
        inspectors.update(pinned, InspectorAction::TogglePin);
        inspectors.inspect(B);

        inspectors.update(pinned, InspectorAction::TogglePin);

        assert_eq!(records(&inspectors), [B]);
        assert!(!nth(&inspectors, 0).pinned);
    }

    /// Asserts that a pinned inspector still moves through its own history.
    #[test]
    fn pinned_history() {
        let mut inspectors = inspected(&[A, B]);
        let pinned = id(&inspectors, 0);
        inspectors.update(pinned, InspectorAction::TogglePin);

        inspectors.update(pinned, InspectorAction::Back);

        assert_eq!(records(&inspectors), [A]);
    }

    /// Asserts that inspecting brings the unpinned inspector in front of the pinned ones.
    #[test]
    fn inspect_to_front() {
        let mut inspectors = inspected(&[A]);
        inspectors.update(id(&inspectors, 0), InspectorAction::TogglePin);
        inspectors.inspect(B);
        inspectors.update(id(&inspectors, 0), InspectorAction::BringToFront);
        assert_eq!(records(&inspectors), [B, A]);

        inspectors.inspect(C);

        assert_eq!(records(&inspectors), [A, C]);
    }

    /// Asserts that inspecting the shown record still brings its inspector to the front.
    #[test]
    fn duplicate_to_front() {
        let mut inspectors = inspected(&[A]);
        inspectors.update(id(&inspectors, 0), InspectorAction::TogglePin);
        inspectors.inspect(B);
        inspectors.update(id(&inspectors, 0), InspectorAction::BringToFront);

        inspectors.inspect(B);

        assert_eq!(records(&inspectors), [A, B]);
        assert_eq!(nth(&inspectors, 1).position(), (1, 1));
    }

    /// Asserts that bringing an inspector to the front keeps the others in order.
    #[test]
    fn to_front() {
        let mut inspectors = Inspectors::default();
        for record in [A, B, C] {
            inspectors.inspect(record);
            inspectors.update(
                id(&inspectors, inspectors.open().len() - 1),
                InspectorAction::TogglePin,
            );
        }

        inspectors.update(id(&inspectors, 0), InspectorAction::BringToFront);

        assert_eq!(records(&inspectors), [B, C, A]);
    }

    /// Asserts that closing removes only that inspector, and the next record opens a new one.
    #[test]
    fn close() {
        let mut inspectors = inspected(&[A]);
        let first = id(&inspectors, 0);
        inspectors.update(first, InspectorAction::TogglePin);
        inspectors.inspect(B);

        inspectors.update(id(&inspectors, 1), InspectorAction::Close);
        inspectors.inspect(C);

        assert_eq!(records(&inspectors), [A, C]);
        assert_eq!(nth(&inspectors, 1).position(), (1, 1));
        assert_ne!(id(&inspectors, 1), first);
    }

    /// Asserts that every opened inspector gets a new id, even after others closed.
    #[test]
    fn ids() {
        let mut inspectors = inspected(&[A]);
        let first = id(&inspectors, 0);
        inspectors.update(first, InspectorAction::Close);

        inspectors.inspect(A);

        assert_ne!(id(&inspectors, 0), first);
    }

    /// Asserts that actions on a closed inspector change nothing.
    #[test]
    fn closed() {
        let mut inspectors = inspected(&[A, B]);
        let closed = id(&inspectors, 0);
        inspectors.update(closed, InspectorAction::Close);
        inspectors.inspect(C);
        let before = inspectors.clone();

        for action in [
            InspectorAction::BringToFront,
            InspectorAction::Back,
            InspectorAction::TogglePin,
            InspectorAction::Close,
        ] {
            inspectors.update(closed, action);
        }

        assert_eq!(inspectors, before);
    }
    /// Whether the panel at `index` is the map.
    fn is_map(inspectors: &Inspectors, index: usize) -> bool {
        matches!(inspectors.open()[index], OpenPanel::Map(_))
    }

    /// Asserts that the map opens in front, beside the inspectors.
    #[test]
    fn map_opens() {
        let mut inspectors = inspected(&[A]);

        inspectors.open_map();

        assert_eq!(inspectors.open().len(), 2);
        assert!(is_map(&inspectors, 1));
    }

    /// Asserts that opening the map again brings it to the front instead of opening another.
    #[test]
    fn map_once() {
        let mut inspectors = Inspectors::default();
        inspectors.open_map();
        let map = id(&inspectors, 0);
        inspectors.inspect(A);

        inspectors.open_map();

        assert_eq!(inspectors.open().len(), 2);
        assert!(is_map(&inspectors, 1));
        assert_eq!(id(&inspectors, 1), map);
    }

    /// Asserts that inspecting never shows a record in the map, it opens an inspector beside it.
    #[test]
    fn map_not_reused() {
        let mut inspectors = Inspectors::default();
        inspectors.open_map();

        inspectors.inspect(A);
        inspectors.inspect(B);

        assert!(is_map(&inspectors, 0));
        assert_eq!(records(&inspectors), [B]);
    }

    /// Asserts that the map ignores history and pin actions, and still closes.
    #[test]
    fn map_actions() {
        let mut inspectors = Inspectors::default();
        inspectors.open_map();
        let map = id(&inspectors, 0);
        let before = inspectors.clone();

        for action in [
            InspectorAction::Back,
            InspectorAction::Forward,
            InspectorAction::TogglePin,
        ] {
            inspectors.update(map, action);
        }
        assert_eq!(inspectors, before);

        inspectors.update(map, InspectorAction::Close);
        assert!(inspectors.open().is_empty());
    }
}
