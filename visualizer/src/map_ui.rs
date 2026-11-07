//! The global map: every component and grouped system of the dataset, alphabetically, with a
//! highlight toggle and an inspect button for each.

use crate::floating::{click_point, focus_id, FloatingPanel, Openings, ReturnTo};
use crate::scene::{access_mark, Highlighting, SceneRect};
use dioxus::prelude::*;
use visualizer::highlight::Relation;
use visualizer::inspectors::{InspectorId, Record};
use visualizer::model::{ComponentId, Dataset, SystemId};
use visualizer::state::{Action, AppState};

/// The id of the search panel's button opening the map.
pub const MAP_LAUNCH_ID: &str = "map-launch";

/// How many records a list column holds before the list gets another column, up to three.
const COLUMN_LENGTH: usize = 10;
const MAX_COLUMNS: usize = 3;
/// How wide a list column is: a wider list gets another column once one more fits, a list narrower
/// than a column scrolls sideways. Once a list has all its columns, they stretch to fill it.
const COLUMN_WIDTH: f64 = 200.0;
/// The space between two columns, from `main.css`.
const COLUMN_GAP: f64 = 8.0;
/// What a list needs beside its columns: its side padding and its items', from `main.css`, and
/// the room every list keeps for a scrollbar.
const LIST_EXTRA: f64 = 24.0 + 20.0;
/// What the map needs beside its lists: its borders and the line between the lists.
const MAP_EXTRA: f64 = 4.0;

/// The global map, in a floating panel.
#[component]
pub fn MapPanel(
    state: Signal<AppState>,
    id: InspectorId,
    layer: usize,
    in_front: bool,
    index: usize,
    scene_rect: Signal<Option<SceneRect>>,
    openings: Openings,
    return_to: Signal<ReturnTo>,
) -> Element {
    // The lists' width, `None` until it's measured.
    let mut body_width = use_signal(|| None::<f64>);
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;
    let component_columns = column_count(dataset.components.len());
    let system_columns = column_count(dataset.systems.len());
    // Opens with room for every column.
    let width = list_width(component_columns) + list_width(system_columns) + MAP_EXTRA;

    // Side by side while both lists fit a column, each wider with its columns. A narrower map
    // stacks them, each taking half the height.
    let stacked = body_width().is_some_and(|width| width < 2.0 * list_width(1));
    let grid = if stacked {
        "grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);"
            .to_string()
    } else {
        format!(
            "grid-template-columns: minmax(0, {component_columns}fr) minmax(0, {system_columns}fr); grid-template-rows: minmax(0, 1fr);"
        )
    };

    rsx! {
        FloatingPanel {
            state,
            id,
            kind: "map",
            title: "Global map",
            name: "Global map",
            noun: "map",
            width,
            layer,
            in_front,
            index,
            scene_rect,
            openings,
            return_to,
            actions: rsx! {
                span { class: "map-pinned", "Always pinned" }
            },

            div {
                class: "map-body",
                style: grid,
                onresize: move |event| {
                    // The desktop renderer bubbles resize events, the scene would take our size as
                    // its own.
                    event.stop_propagation();
                    let Ok(size) = event.get_border_box_size() else {
                        return;
                    };
                    if *body_width.peek() != Some(size.width) {
                        body_width.set(Some(size.width));
                    }
                },

                MapList {
                    state,
                    openings,
                    heading_id: focus_id(id),
                    kind: MapKind::Components,
                    max_columns: component_columns,
                }
                MapList {
                    state,
                    openings,
                    heading_id: format!("map-{}-systems", id.0),
                    kind: MapKind::Systems,
                    max_columns: system_columns,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MapKind {
    Components,
    Systems,
}

/// A row of the map.
struct Row {
    record: Record,
    label: String,
    name: String,
}

/// One of the map's lists, scrolling on its own, with as many columns as fit, up to
/// `max_columns`.
///
/// It only renders again when the highlight changes, the dataset doesn't while it's mounted.
/// Rows show how they relate to the highlight: the target, a borrow and its access, or dimmed
/// when unrelated.
#[component]
fn MapList(
    mut state: Signal<AppState>,
    openings: Openings,
    heading_id: String,
    kind: MapKind,
    max_columns: usize,
) -> Element {
    // How many columns fit the list, `None` until it's measured.
    let mut fitting = use_signal(|| None::<usize>);
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let Highlighting(relations) = use_context::<Highlighting>();
    let relations = relations.read();
    let rows = rows(&loaded.dataset, kind);
    let title = match kind {
        MapKind::Components => "Components",
        MapKind::Systems => "Systems",
    };
    let columns = fitting().unwrap_or(max_columns);
    let columns_width = columns as f64 * (COLUMN_WIDTH + COLUMN_GAP) - COLUMN_GAP;
    // With every column it can have, a list stretches them, there's no other one to wait for.
    // Both properties every time: Dioxus updates the style declaration by declaration, so one we
    // stop writing would keep its last value.
    let width = if columns == max_columns {
        "auto".to_string()
    } else {
        format!("{columns_width}px")
    };

    rsx! {
        section {
            class: "map-list",
            aria_labelledby: heading_id.clone(),
            h2 { id: heading_id, tabindex: "-1",
                "{title}"
                span { class: "map-count", " {rows.len()}" }
            }
            div {
                class: "map-items",
                onresize: move |event| {
                    // The desktop renderer bubbles resize events, the scene would take our size as
                    // its own.
                    event.stop_propagation();
                    let Ok(size) = event.get_content_box_size() else {
                        return;
                    };
                    // Only a change of column count renders the rows again.
                    let fit = ((size.width + COLUMN_GAP) / (COLUMN_WIDTH + COLUMN_GAP)).floor() as usize;
                    let fit = fit.clamp(1, max_columns);
                    if *fitting.peek() != Some(fit) {
                        fitting.set(Some(fit));
                    }
                },

                // One list flowing down its columns, so reading order stays alphabetical. Until it
                // has all its columns, the list is as wide as them, so they keep their width and
                // the rest of the list stays empty.
                ul { style: "column-count: {columns}; width: {width}; min-width: {columns_width}px;",
                    for (index, row) in rows.into_iter().enumerate() {
                        {
                            let record = row.record;
                            let relation = relations.as_ref().map(|relations| relations.record(record));
                            let highlighted = relation == Some(Relation::Target);
                            let mark = relation.and_then(access_mark);
                            let inspect_id = format!("map-{title}-{index}");
                            let access = match relation {
                                Some(Relation::Exclusive) => ", exclusive access",
                                Some(Relation::Shared) => ", shared access",
                                _ => "",
                            };

                            rsx! {
                                li { key: "{index}", class: "map-row",
                                    button {
                                        class: map_class(relation),
                                        r#type: "button",
                                        aria_pressed: if highlighted { "true" } else { "false" },
                                        aria_label: "Highlight {row.label}{access}",
                                        title: "{row.name}",
                                        "aria-description": "{row.name}",
                                        onclick: move |_| state.write().update(Action::ToggleHighlight(record)),
                                        span { class: "map-label", "{row.label}" }
                                        if let Some(mark) = mark {
                                            small { aria_hidden: "true", "{mark}" }
                                        }
                                    }
                                    button {
                                        id: inspect_id.clone(),
                                        class: "map-inspect",
                                        r#type: "button",
                                        aria_label: "Inspect {row.label}",
                                        title: "Inspect",
                                        onclick: move |event| {
                                            let pointer = click_point(&event);
                                            let action = Action::Inspect(record);
                                            openings.open(state, action, pointer, Some(inspect_id.clone()));
                                        },
                                        {magnifier()}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The records of `kind`, sorted by label like the dataset keeps them.
fn rows(dataset: &Dataset, kind: MapKind) -> Vec<Row> {
    match kind {
        MapKind::Components => dataset
            .components
            .iter()
            .enumerate()
            .map(|(index, component)| Row {
                record: Record::Component(ComponentId(index)),
                label: component.label.clone(),
                name: component.name.clone(),
            })
            .collect(),
        MapKind::Systems => dataset
            .systems
            .iter()
            .enumerate()
            .map(|(index, system)| Row {
                record: Record::System(SystemId(index)),
                label: system.label.clone(),
                name: system.name.clone(),
            })
            .collect(),
    }
}

/// A map row's classes for its relation to the highlight.
fn map_class(relation: Option<Relation>) -> &'static str {
    match relation {
        Some(Relation::Target) => "map-select target",
        Some(Relation::Exclusive) => "map-select exclusive",
        Some(Relation::Shared) => "map-select shared",
        Some(Relation::Unrelated) => "map-select dimmed",
        Some(Relation::SameKind) | None => "map-select",
    }
}

/// How many columns a list of `count` records gets at most.
fn column_count(count: usize) -> usize {
    count.div_ceil(COLUMN_LENGTH).clamp(1, MAX_COLUMNS)
}

/// How wide a list with `columns` columns is, padding and scrollbar included.
fn list_width(columns: usize) -> f64 {
    columns as f64 * (COLUMN_WIDTH + COLUMN_GAP) - COLUMN_GAP + LIST_EXTRA
}

/// A magnifying glass, for the inspect buttons.
fn magnifier() -> Element {
    rsx! {
        svg {
            width: "15",
            height: "15",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            circle { cx: "11", cy: "11", r: "7" }
            path { d: "M16.5 16.5 21 21" }
        }
    }
}
