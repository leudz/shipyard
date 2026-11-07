//! The component, system, and workload search fields.
//!
//! Each field is a combobox: focus stays in the text input while the arrow keys move through its
//! popup listbox, and the input points screen readers at the active result.

use crate::floating::{click_point, Openings};
use crate::import_ui::OPEN_FILE_ID;
use crate::map_ui::MAP_LAUNCH_ID;
use crate::{count, dom, scene};
use dioxus::prelude::*;
use visualizer::inspectors::Record;
use visualizer::names::storage_kind;
use visualizer::panels::Point;
use visualizer::state::{Action, AppState, Loaded, SearchKind};

/// Describes the fields' keyboard behavior, which differs from regular Tab navigation.
const HINT_ID: &str = "search-hint";

/// The fields in display order.
const KINDS: [SearchKind; 3] = [
    SearchKind::Components,
    SearchKind::Systems,
    SearchKind::Workloads,
];

/// The search fields, with the results of the field being searched below them.
#[component]
pub fn SearchPanel(state: Signal<AppState>) -> Element {
    // Which field shows its results, and which of them Enter activates.
    // They're only about the popup, so they stay local while the queries live in the workspace.
    let open = use_signal(|| None::<SearchKind>);
    let active = use_signal(|| 0);
    let openings = use_context::<Openings>();

    rsx! {
        nav { class: "panel search", aria_label: "Search",
            p { id: HINT_ID, class: "visually-hidden",
                "Tab moves between the search fields. Press Escape twice to leave them."
            }
            div { class: "search-fields",
                // First in the DOM so Tab reaches it before the fields, which keep Tab to
                // themselves. The styles draw it at the end of the row, like the preview.
                button {
                    id: MAP_LAUNCH_ID,
                    class: "button map-launch",
                    r#type: "button",
                    onclick: move |event| {
                        let pointer = click_point(&event);
                        openings.open(state, Action::OpenMap, pointer, Some(MAP_LAUNCH_ID.to_string()));
                    },
                    "Global map"
                }
                for kind in KINDS {
                    SearchField {
                        key: "{kind:?}",
                        state,
                        kind,
                        open,
                        active,
                    }
                }
            }
            for kind in KINDS {
                SearchResults {
                    key: "{kind:?}",
                    state,
                    kind,
                    open,
                    active,
                }
            }
        }
    }
}

/// A labelled text input with combobox semantics.
#[component]
fn SearchField(
    state: Signal<AppState>,
    kind: SearchKind,
    open: Signal<Option<SearchKind>>,
    active: Signal<usize>,
) -> Element {
    let openings = use_context::<Openings>();
    let app = state.read();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };

    let (title, noun) = names(kind);
    let (query, record_count) = query_and_count(loaded, kind);
    // With no results there is no listbox to show, the status line says why instead.
    let active_option = (open() == Some(kind))
        .then(|| active_index(active(), result_count(loaded, kind)))
        .flatten();
    let expanded = active_option.is_some();

    rsx! {
        label { class: "search-field",
            "{title}"
            input {
                id: input_id(kind),
                r#type: "text",
                role: "combobox",
                aria_autocomplete: "list",
                aria_expanded: if expanded { "true" } else { "false" },
                aria_controls: listbox_id(kind),
                aria_describedby: HINT_ID,
                aria_activedescendant: active_option.map(|index| option_id(kind, index)),
                autocomplete: "off",
                spellcheck: "false",
                placeholder: "Search {count(record_count, noun)}",
                value: "{query}",
                onfocus: move |_| show_results(open, active, kind),
                // Picking a result closes the list but keeps focus here, so clicking the field
                // again doesn't fire `focus`. A click on a closed field reopens it instead, one on
                // an open field only moves the caret.
                onclick: move |_| {
                    if open() != Some(kind) {
                        show_results(open, active, kind);
                    }
                },
                onblur: move |_| {
                    if open() == Some(kind) {
                        open.set(None);
                    }
                },
                oninput: move |event| {
                    state
                        .write()
                        .update(Action::Search {
                            kind,
                            query: event.value(),
                        });
                    // The results changed, so we start again from the best one.
                    show_results(open, active, kind);
                },
                onkeydown: move |event| on_key(event, state, openings, kind, open, active),
            }
        }
    }
}

/// The popup listbox of `kind`'s field, and a status line saying when nothing matches.
#[component]
fn SearchResults(
    state: Signal<AppState>,
    kind: SearchKind,
    open: Signal<Option<SearchKind>>,
    active: Signal<usize>,
) -> Element {
    let openings = use_context::<Openings>();
    let app = state.read();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };

    let (title, noun) = names(kind);
    let is_open = open() == Some(kind);
    // We only build the rows of the open field.
    let rows = if is_open {
        result_rows(loaded, kind)
    } else {
        Vec::new()
    };
    let active_index = active_index(active(), rows.len());
    let (query, _) = query_and_count(loaded, kind);
    let no_match = is_open && rows.is_empty();
    let hide_results = rows.is_empty();

    rsx! {
        div {
            id: listbox_id(kind),
            class: "search-results",
            role: "listbox",
            aria_label: "{title}",
            hidden: hide_results,
            // Clicking a result mustn't move focus out of the field, which would close the popup
            // before the click lands.
            onmousedown: move |event| event.prevent_default(),

            for (index, row) in rows.into_iter().enumerate() {
                div {
                    key: "{row.key}",
                    id: option_id(kind, index),
                    class: "search-result",
                    role: "option",
                    aria_selected: if active_index == Some(index) { "true" } else { "false" },
                    title: row.full_name,
                    onclick: move |event| {
                        activate(state, openings, kind, index, click_point(&event));
                        open.set(None);
                    },
                    span { "{row.label}" }
                    small { "{row.context}" }
                }
            }
        }
        // A status is announced when its text changes, so it stays mounted and only its text
        // comes and goes.
        p { class: "search-note", role: "status",
            if no_match {
                "No {noun}s match “{query}”."
            }
        }
    }
}

/// Moves through the results with the arrow keys, activates with Enter, and closes with Escape.
///
/// Tab and Shift+Tab cycle between the three fields, Escape on closed results leaves them.
/// Other keys keep their usual text editing behavior.
fn on_key(
    event: KeyboardEvent,
    state: Signal<AppState>,
    openings: Openings,
    kind: SearchKind,
    mut open: Signal<Option<SearchKind>>,
    mut active: Signal<usize>,
) {
    let key = event.key();

    if key == Key::Tab {
        event.prevent_default();
        let backward = event.modifiers().contains(Modifiers::SHIFT);
        // Moving focus closes this field's results and opens the next one's.
        dom::focus(&input_id(next_kind(kind, backward)));
        return;
    }

    // While the results are closed, ArrowDown opens them and Escape leaves the fields, since Tab
    // can't. We move focus into the scene, a blur would let the next Tab land back here.
    if open() != Some(kind) {
        match key {
            Key::ArrowDown => {
                event.prevent_default();
                show_results(open, active, kind);
            }
            Key::Escape => {
                event.prevent_default();
                let entry = state.read().loaded.as_ref().and_then(scene::entry_id);
                match entry {
                    Some(entry) => dom::focus_first(&[&entry, OPEN_FILE_ID]),
                    None => dom::focus(OPEN_FILE_ID),
                }
            }
            _ => {}
        }

        return;
    }

    let result_count = state
        .read()
        .loaded
        .as_ref()
        .map_or(0, |loaded| result_count(loaded, kind));
    // `active` can be past the end when the results shrank without a keystroke.
    let current = active_index(active(), result_count);

    match key {
        Key::ArrowDown | Key::ArrowUp => {
            // We keep the caret where it is, these keys would move it to an end of the text.
            event.prevent_default();
            let Some(current) = current else {
                return;
            };

            let next = if key == Key::ArrowDown {
                (current + 1).min(result_count - 1)
            } else {
                current.saturating_sub(1)
            };
            active.set(next);
            dom::scroll_into_view(&option_id(kind, next));
        }
        Key::Enter => {
            let Some(current) = current else {
                return;
            };

            event.prevent_default();
            activate(state, openings, kind, current, None);
            open.set(None);
        }
        Key::Escape => {
            event.prevent_default();
            open.set(None);
        }
        _ => {}
    }
}

/// The field after `kind`, or before it when `backward`, wrapping around.
fn next_kind(kind: SearchKind, backward: bool) -> SearchKind {
    let index = KINDS.iter().position(|&other| other == kind).unwrap();
    let step = if backward { KINDS.len() - 1 } else { 1 };

    KINDS[(index + step) % KINDS.len()]
}

/// Opens `kind`'s results with the best one active.
fn show_results(mut open: Signal<Option<SearchKind>>, mut active: Signal<usize>, kind: SearchKind) {
    open.set(Some(kind));
    active.set(0);
}

/// Selects the workload, or inspects the system or component, at `index` in `kind`'s results.
///
/// `pointer` is where the result was clicked, `None` for Enter. Inspecting moves focus to the
/// inspector, leaving it comes back to the field.
fn activate(
    mut state: Signal<AppState>,
    openings: Openings,
    kind: SearchKind,
    index: usize,
    pointer: Option<Point>,
) {
    let action = {
        let app = state.read();
        let Some(loaded) = &app.loaded else {
            return;
        };

        let searches = &loaded.workspace.searches;
        match kind {
            SearchKind::Components => searches
                .components
                .results
                .get(index)
                .map(|&component| Action::Inspect(Record::Component(component))),
            SearchKind::Systems => searches
                .systems
                .results
                .get(index)
                .map(|&system| Action::Inspect(Record::System(system))),
            SearchKind::Workloads => searches
                .workloads
                .results
                .get(index)
                .map(|&workload| Action::SelectWorkload(workload)),
        }
    };

    match action {
        Some(action @ Action::Inspect(_)) => {
            openings.open(state, action, pointer, Some(input_id(kind)));
        }
        Some(action) => state.write().update(action),
        None => {}
    }
}

/// What a result shows.
struct Row<'a> {
    /// The record's index, which keeps its element across searches.
    key: usize,
    label: &'a str,
    full_name: &'a str,
    /// A short hint telling similar results apart.
    context: String,
}

fn result_rows(loaded: &Loaded, kind: SearchKind) -> Vec<Row<'_>> {
    let dataset = &loaded.dataset;
    let searches = &loaded.workspace.searches;

    match kind {
        SearchKind::Components => searches
            .components
            .results
            .iter()
            .map(|&id| {
                let component = dataset.component(id);

                Row {
                    key: id.0,
                    label: &component.label,
                    full_name: &component.name,
                    context: storage_kind(&component.name).to_string(),
                }
            })
            .collect(),
        SearchKind::Systems => searches
            .systems
            .results
            .iter()
            .map(|&id| {
                let system = dataset.system(id);

                Row {
                    key: id.0,
                    label: &system.label,
                    full_name: &system.name,
                    context: count(system.borrows.len(), "borrow"),
                }
            })
            .collect(),
        SearchKind::Workloads => searches
            .workloads
            .results
            .iter()
            .map(|&id| {
                let workload = dataset.workload(id);

                Row {
                    key: id.0,
                    label: &workload.label,
                    full_name: &workload.name,
                    context: count(workload.batches.len(), "batch"),
                }
            })
            .collect(),
    }
}

/// `kind`'s query, and how many records its field searches.
fn query_and_count(loaded: &Loaded, kind: SearchKind) -> (&str, usize) {
    let searches = &loaded.workspace.searches;
    let dataset = &loaded.dataset;

    match kind {
        SearchKind::Components => (&searches.components.query, dataset.components.len()),
        SearchKind::Systems => (&searches.systems.query, dataset.systems.len()),
        SearchKind::Workloads => (&searches.workloads.query, dataset.workloads.len()),
    }
}

fn result_count(loaded: &Loaded, kind: SearchKind) -> usize {
    let searches = &loaded.workspace.searches;

    match kind {
        SearchKind::Components => searches.components.results.len(),
        SearchKind::Systems => searches.systems.results.len(),
        SearchKind::Workloads => searches.workloads.results.len(),
    }
}

/// The active result, kept inside the results. `None` when there are none.
fn active_index(active: usize, result_count: usize) -> Option<usize> {
    (result_count > 0).then(|| active.min(result_count - 1))
}

/// The field's label and the noun of its records.
fn names(kind: SearchKind) -> (&'static str, &'static str) {
    match kind {
        SearchKind::Components => ("Components", "component"),
        SearchKind::Systems => ("Systems", "system"),
        SearchKind::Workloads => ("Workloads", "workload"),
    }
}

fn input_id(kind: SearchKind) -> String {
    format!("search-{}", names(kind).1)
}

fn listbox_id(kind: SearchKind) -> String {
    format!("search-{}-results", names(kind).1)
}

fn option_id(kind: SearchKind, index: usize) -> String {
    format!("search-{}-result-{index}", names(kind).1)
}
