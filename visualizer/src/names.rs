//! Readable labels for exported names, and the order we display them in.
//!
//! Full names are identity and search data, labels are only presentation.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

/// Storage types shipyard wraps around component and unique names.
const STORAGE_WRAPPERS: [&str; 2] = [
    "shipyard::sparse_set::SparseSet<",
    "shipyard::unique::UniqueStorage<",
];

/// Gives each full name the shortest readable label no other name in `full_names` shares.
///
/// Labels start with the last segment of every path, `System(a::b::run<c::D>)` becomes `run<D>`,
/// and keep more segments while they collide.
/// Equal full names share their label with a ` #1`, ` #2`... suffix, following their order in `full_names`.
pub fn unique_labels(full_names: &[&str]) -> Vec<String> {
    let distinct: BTreeSet<&str> = full_names.iter().copied().collect();
    let labels = distinct_labels(distinct.into_iter().collect());

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for &full_name in full_names {
        *counts.entry(full_name).or_default() += 1;
    }

    let mut numbers: HashMap<&str, usize> = HashMap::new();
    full_names
        .iter()
        .map(|&full_name| {
            let label = &labels[full_name];
            if counts[full_name] == 1 {
                return label.clone();
            }

            let number = numbers.entry(full_name).or_default();
            *number += 1;
            format!("{label} #{number}")
        })
        .collect()
}

/// Gives each of the distinct `full_names` the shortest label none of the others share.
fn distinct_labels(full_names: Vec<&str>) -> HashMap<&str, String> {
    let candidates: Vec<Vec<String>> = full_names
        .iter()
        .map(|full_name| label_candidates(full_name))
        .collect();
    let mut chosen = vec![0; full_names.len()];

    // We lengthen every colliding label by one step at a time, until none collide or none can grow.
    // All colliding groups grow together, so the result doesn't depend on the map's iteration order.
    loop {
        let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, &candidate) in chosen.iter().enumerate() {
            groups
                .entry(&candidates[index][candidate])
                .or_default()
                .push(index);
        }

        let mut grew = false;
        for members in groups.values().filter(|members| members.len() > 1) {
            for &index in members {
                if chosen[index] + 1 < candidates[index].len() {
                    chosen[index] += 1;
                    grew = true;
                }
            }
        }

        if !grew {
            break;
        }
    }

    // No collision is left: a label that can't grow is its full name, and the names are distinct.
    // So any label colliding with it is a shorter form of another name, which could still grow.
    full_names
        .into_iter()
        .zip(candidates.into_iter().zip(chosen))
        .map(|(full_name, (mut candidates, candidate))| {
            (full_name, candidates.swap_remove(candidate))
        })
        .collect()
}

/// What kind of storage the component `full_name` names, as we show it to users.
pub fn storage_kind(full_name: &str) -> &'static str {
    let [sparse_set, unique] = STORAGE_WRAPPERS;

    if full_name.starts_with(sparse_set) {
        "Component storage"
    } else if full_name.starts_with(unique) {
        "Unique storage"
    } else {
        "Storage"
    }
}

/// Orders labels the way the preview does: case-insensitively, comparing digit runs as numbers.
///
/// Labels that only differ by case, or by leading zeros, fall back to their exact text,
/// which makes the order total.
pub fn label_cmp(a: &str, b: &str) -> Ordering {
    natural_cmp(a, b).then_with(|| a.cmp(b))
}

/// Every label we can give `full_name`, from shortest to longest.
///
/// We first grow the name without its wrappers, then fall back to the full name.
/// The last candidate is the full name itself, so different names always end up with different labels.
fn label_candidates(full_name: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();

    for source in [unwrap_name(full_name), full_name] {
        for depth in 1.. {
            let label = shorten(source, depth);

            // Once a depth adds nothing every path is complete, deeper ones won't add anything either.
            if candidates.last() == Some(&label) {
                break;
            }

            candidates.push(label);
        }
    }

    candidates
}

/// Removes the `System(...)`/`Workload(...)` wrapper of labels and the storage around component names.
fn unwrap_name(full_name: &str) -> &str {
    let mut name = full_name;

    for prefix in ["System(", "Workload("] {
        if let Some(inner) = name.strip_prefix(prefix).and_then(|n| n.strip_suffix(')')) {
            name = inner;
        }
    }

    for prefix in STORAGE_WRAPPERS {
        if let Some(inner) = name.strip_prefix(prefix).and_then(|n| n.strip_suffix('>')) {
            name = inner;
        }
    }

    name
}

/// Shortens every path in `name`, generic arguments included, to its last `depth` segments.
fn shorten(name: &str, depth: usize) -> String {
    let mut label = String::with_capacity(name.len());
    let mut path_start = None;

    for (index, c) in name.char_indices() {
        if is_path_char(c) {
            path_start.get_or_insert(index);
            continue;
        }

        if let Some(start) = path_start.take() {
            label.push_str(path_suffix(&name[start..index], depth));
        }
        label.push(c);
    }

    if let Some(start) = path_start {
        label.push_str(path_suffix(&name[start..], depth));
    }

    label
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '{' | '}')
}

/// Keeps the last `depth` segments of `path`.
///
/// Segments like `{{closure}}` don't count, they mean nothing without the function they're in.
fn path_suffix(path: &str, depth: usize) -> &str {
    let mut kept = 0;
    let mut segment_end = path.len();

    for (separator, _) in path.rmatch_indices("::") {
        let segment_start = separator + 2;
        if !path[segment_start..segment_end].starts_with("{{") {
            kept += 1;
        }

        if kept == depth {
            return &path[segment_start..];
        }

        segment_end = separator;
    }

    path
}

/// Compares case-insensitively, digit runs by numeric value.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();

    loop {
        let (Some(&a_char), Some(&b_char)) = (a.peek(), b.peek()) else {
            // One side ran out, the shorter one comes first.
            return a.peek().is_some().cmp(&b.peek().is_some());
        };

        let ordering = if a_char.is_ascii_digit() && b_char.is_ascii_digit() {
            let a_number = take_digits(&mut a);
            let b_number = take_digits(&mut b);

            number_cmp(&a_number, &b_number)
        } else {
            a.next();
            b.next();

            a_char.to_lowercase().cmp(b_char.to_lowercase())
        };

        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();

    while let Some(&c) = chars.peek() {
        if !c.is_ascii_digit() {
            break;
        }

        digits.push(c);
        chars.next();
    }

    digits
}

/// Compares two digit runs by value, without overflowing on long runs.
fn number_cmp(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');

    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(full_names: &[&str]) -> Vec<String> {
        unique_labels(full_names)
    }

    /// Asserts that a lone system keeps only the last segment, without its `System(...)` wrapper.
    #[test]
    fn system_wrapper() {
        assert_eq!(labels(&["System(square_eater::counters)"]), ["counters"]);
    }

    /// Asserts that a lone workload keeps only the last segment, without its `Workload(...)` wrapper.
    #[test]
    fn workload_wrapper() {
        assert_eq!(
            labels(&["Workload(engine::workloads::render_workload)"]),
            ["render_workload"]
        );
    }

    /// Asserts that component labels drop shipyard's storage types.
    #[test]
    fn storage_wrapper() {
        let full_names = [
            "shipyard::sparse_set::SparseSet<engine::light::Light>",
            "shipyard::unique::UniqueStorage<engine::gpu::Gpu>",
        ];

        assert_eq!(labels(&full_names), ["Light", "Gpu"]);
    }

    /// Asserts that every path in nested generic arguments is shortened.
    #[test]
    fn nested_generics() {
        let full_names = ["System(fsm::on_update<a::Hide, b::Wrapper<c::Inner>>)"];

        assert_eq!(labels(&full_names), ["on_update<Hide, Wrapper<Inner>>"]);
    }

    /// Asserts that same short names from different modules keep enough segments to tell them apart.
    #[test]
    fn same_short_name() {
        let full_names = [
            "System(engine::prepare_render::debug_lines::debug_lines)",
            "System(engine::render::debug_lines::debug_lines)",
            "System(engine::light::light)",
        ];

        let expected = [
            "prepare_render::debug_lines::debug_lines",
            "render::debug_lines::debug_lines",
            "light",
        ];
        assert_eq!(labels(&full_names), expected);
    }

    /// Asserts that a closure keeps the function it's defined in.
    #[test]
    fn closure() {
        let full_names = ["System(engine::npc::scorer_workload::{{closure}})"];

        assert_eq!(labels(&full_names), ["scorer_workload::{{closure}}"]);
    }

    /// Asserts that equal full names are numbered in input order.
    #[test]
    fn equal_names() {
        let full_names = ["System(a::run)", "System(a::run)", "System(b::other)"];

        assert_eq!(labels(&full_names), ["run #1", "run #2", "other"]);
    }

    /// Asserts that a unique and a sparse set of the same type get distinct labels.
    #[test]
    fn unique_and_sparse_set() {
        let full_names = [
            "shipyard::sparse_set::SparseSet<a::Light>",
            "shipyard::unique::UniqueStorage<a::Light>",
        ];

        assert_eq!(
            labels(&full_names),
            ["SparseSet<Light>", "UniqueStorage<Light>"]
        );
    }

    /// Asserts that digit runs compare by value.
    #[test]
    fn numeric_order() {
        assert_eq!(label_cmp("system2", "system10"), Ordering::Less);
    }

    /// Asserts that labels compare case-insensitively before falling back to their exact text.
    #[test]
    fn case_order() {
        assert_eq!(label_cmp("apple", "Banana"), Ordering::Less);
        assert_eq!(label_cmp("Apple", "apple"), Ordering::Less);
    }

    /// Asserts that a prefix comes before the longer label.
    #[test]
    fn prefix_order() {
        assert_eq!(label_cmp("run", "run_all"), Ordering::Less);
    }
    /// Asserts that sparse sets, uniques, and other storages each get their kind.
    #[test]
    fn storage_kinds() {
        assert_eq!(
            storage_kind("shipyard::sparse_set::SparseSet<game::Position>"),
            "Component storage"
        );
        assert_eq!(
            storage_kind("shipyard::unique::UniqueStorage<game::Camera>"),
            "Unique storage"
        );
        assert_eq!(storage_kind("shipyard::entities::Entities"), "Storage");
    }
}
