//! Fuzzy search over workloads, systems, and components.
//!
//! Queries match as abbreviations: every query character has to appear in order, gaps allowed,
//! so `mvpl` finds `move_player`. There is no query syntax and no spelling correction.

use crate::model::{ComponentId, Dataset, SystemId, WorkloadId};
use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32String};
use std::cmp::Reverse;

/// Searches each kind of record of a dataset independently.
///
/// Build it once per dataset, each search then reuses the precomputed strings and matcher.
#[derive(Debug)]
pub struct Search {
    matcher: Matcher,
    workloads: SearchIndex,
    systems: SearchIndex,
    components: SearchIndex,
}

impl Search {
    pub fn new(dataset: &Dataset) -> Search {
        Search {
            matcher: Matcher::new(Config::DEFAULT),
            workloads: SearchIndex::new(
                dataset
                    .workloads
                    .iter()
                    .map(|workload| (workload.label.as_str(), workload.name.as_str())),
            ),
            systems: SearchIndex::new(
                dataset
                    .systems
                    .iter()
                    .map(|system| (system.label.as_str(), system.name.as_str())),
            ),
            components: SearchIndex::new(
                dataset
                    .components
                    .iter()
                    .map(|component| (component.label.as_str(), component.name.as_str())),
            ),
        }
    }

    /// The workloads matching `query`, best first. See [`SearchIndex::search`] for the order.
    pub fn workloads(&mut self, query: &str) -> Vec<WorkloadId> {
        let indices = self.workloads.search(query, &mut self.matcher);

        indices.into_iter().map(WorkloadId).collect()
    }

    /// The systems matching `query`, best first. See [`SearchIndex::search`] for the order.
    pub fn systems(&mut self, query: &str) -> Vec<SystemId> {
        let indices = self.systems.search(query, &mut self.matcher);

        indices.into_iter().map(SystemId).collect()
    }

    /// The components matching `query`, best first. See [`SearchIndex::search`] for the order.
    pub fn components(&mut self, query: &str) -> Vec<ComponentId> {
        let indices = self.components.search(query, &mut self.matcher);

        indices.into_iter().map(ComponentId).collect()
    }
}

/// The searchable strings of one kind of record.
#[derive(Debug)]
pub struct SearchIndex {
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    label: Utf32String,
    full_name: Utf32String,
    /// Compared with the lowercased query to find exact label matches.
    lowercase_label: String,
}

/// How well an entry matches, better ranks sort first.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    ExactLabel,
    Fuzzy(Reverse<u16>),
}

impl SearchIndex {
    /// Indexes `(label, full name)` pairs.
    ///
    /// Equal matches keep the order of `records`, so pass them alphabetically.
    pub fn new<'a>(records: impl IntoIterator<Item = (&'a str, &'a str)>) -> SearchIndex {
        let entries = records
            .into_iter()
            .map(|(label, full_name)| Entry {
                label: Utf32String::from(label),
                full_name: Utf32String::from(full_name),
                lowercase_label: label.to_lowercase(),
            })
            .collect();

        SearchIndex { entries }
    }

    /// The indices of the records matching `query`, ignoring case.
    ///
    /// Records whose label is the query come first, then the others by how well their label or
    /// full name matches. Equal matches keep their alphabetical order.
    /// An empty query lists every record alphabetically.
    pub fn search(&self, query: &str, matcher: &mut Matcher) -> Vec<usize> {
        let query = query.trim();
        if query.is_empty() {
            return (0..self.entries.len()).collect();
        }

        // We match the query as one literal atom, so characters like `^` or `!` have no special meaning.
        let atom = Atom::new(
            query,
            CaseMatching::Ignore,
            Normalization::Never,
            AtomKind::Fuzzy,
            false,
        );
        let lowercase_query = query.to_lowercase();

        let mut matches: Vec<(Rank, usize)> = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.lowercase_label == lowercase_query {
                matches.push((Rank::ExactLabel, index));
                continue;
            }

            // A record is listed once, with whichever of its names matches best.
            let label_score = atom.score(entry.label.slice(..), matcher);
            let full_name_score = atom.score(entry.full_name.slice(..), matcher);
            if let Some(score) = label_score.max(full_name_score) {
                matches.push((Rank::Fuzzy(Reverse(score)), index));
            }
        }

        // Indices follow the records' order, so they break ties between equal ranks.
        matches.sort();

        matches.into_iter().map(|(_, index)| index).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Searches `records`, given as `(label, full name)`, and returns the matching labels in order.
    fn search<'a>(records: &[(&'a str, &str)], query: &str) -> Vec<&'a str> {
        let index = SearchIndex::new(records.iter().copied());
        let mut matcher = Matcher::new(Config::DEFAULT);

        index
            .search(query, &mut matcher)
            .into_iter()
            .map(|index| records[index].0)
            .collect()
    }

    /// Asserts that a query matches as an abbreviation, with gaps between its characters.
    #[test]
    fn abbreviation() {
        let records = [
            ("move_player", "System(game::move_player)"),
            ("render", "System(game::render)"),
        ];

        assert_eq!(search(&records, "mvpl"), ["move_player"]);
    }

    /// Asserts that a full module path finds the record whose full name has it, not ones only
    /// sharing the label.
    #[test]
    fn full_path() {
        let records = [
            ("physics::step", "System(game::physics::step)"),
            ("render::step", "System(game::render::step)"),
        ];

        assert_eq!(search(&records, "game::physics::step"), ["physics::step"]);
    }

    /// Asserts that matching ignores the case of both the query and the names.
    #[test]
    fn case() {
        let records = [("MovePlayer", "System(game::MovePlayer)")];

        assert_eq!(search(&records, "moveplayer"), ["MovePlayer"]);
        assert_eq!(search(&records, "MOVEPLAYER"), ["MovePlayer"]);
    }

    /// Asserts that a record whose label is the query comes before other matches, even ones
    /// listed before it with the same score.
    #[test]
    fn exact_first() {
        // A prefix match scores as high as an exact one, only the exact-label rule puts `pos` first.
        let records = [
            ("Position", "SparseSet<game::Position>"),
            ("pos", "SparseSet<game::pos>"),
        ];

        assert_eq!(search(&records, "POS"), ["pos", "Position"]);
    }

    /// Asserts that records matching equally keep their alphabetical order.
    #[test]
    fn ties() {
        let records = [
            ("a::step", "System(a::step)"),
            ("b::step", "System(b::step)"),
        ];

        assert_eq!(search(&records, "step"), ["a::step", "b::step"]);
    }

    /// Asserts that an empty or blank query lists every record alphabetically.
    #[test]
    fn empty() {
        let records = [("a", "System(a)"), ("b", "System(b)")];

        assert_eq!(search(&records, ""), ["a", "b"]);
        assert_eq!(search(&records, "  "), ["a", "b"]);
    }

    /// Asserts that a record matching through both its label and full name is listed once.
    #[test]
    fn once() {
        let records = [("move", "System(game::move)")];

        assert_eq!(search(&records, "move"), ["move"]);
    }

    /// Asserts that distinct records sharing a short name are all found.
    #[test]
    fn same_short_name() {
        let records = [
            ("run #1", "System(game::run)"),
            ("run #2", "System(game::run)"),
        ];

        assert_eq!(search(&records, "run"), ["run #1", "run #2"]);
    }

    /// Asserts that a query no record matches finds nothing.
    #[test]
    fn no_match() {
        let records = [("render", "System(game::render)")];

        assert!(search(&records, "xyz").is_empty());
    }

    /// Asserts that each kind of record is searched separately, with ids into the dataset.
    #[test]
    fn dataset() {
        let bytes = include_bytes!("square_eater_workloads.json");
        let dataset = Dataset::new(&crate::import::parse_workloads(bytes).unwrap()).unwrap();
        let mut search = Search::new(&dataset);

        let systems = search.systems("mvpl");
        let components = search.components("squagum");

        assert_eq!(dataset.system(systems[0]).label, "move_player");
        assert_eq!(dataset.component(components[0]).label, "Squagum");
        assert!(search.workloads("mvpl").is_empty());
    }
}
