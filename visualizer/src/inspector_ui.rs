//! The inspectors: what a system borrows and where it runs, and which systems borrow a component.
//!
//! Every relationship is a button changing the app's state, links would suggest a destination
//! outside the app.

use crate::dom;
use crate::floating::{
    click_point, close, focus_id, use_opening_focus, FloatingPanel, Openings, ReturnTo,
};
use crate::map_ui::MapPanel;
use crate::scene::{Highlighting, SceneRect};
use dioxus::prelude::*;
use shipyard::borrow::Mutability;
use std::collections::HashMap;
use visualizer::inspectors::{Inspector, InspectorAction, OpenPanel, Record};
use visualizer::model::{ComponentId, Dataset, OccurrenceId, SystemId};
use visualizer::names::storage_kind;
use visualizer::panels::preferred_width;
use visualizer::state::{Action, AppState};

/// The open inspectors and map of the loaded dataset.
///
/// Key it by dataset generation, the panels read the dataset without subscribing to it.
#[component]
pub fn InspectorLayer(
    state: Signal<AppState>,
    scene_rect: Signal<Option<SceneRect>>,
    openings: Openings,
) -> Element {
    let panels = use_memo(move || {
        let app = state.read();
        app.loaded
            .as_ref()
            .map(|loaded| loaded.workspace.inspectors.clone())
            .unwrap_or_default()
    });
    let return_to = use_opening_focus(state, openings);

    // The DOM keeps the opening order and z-index shows the front order. Bringing a panel to the
    // front then never moves elements, which would lose their focus.
    let panels = panels.read();
    let front = panels.open().len().saturating_sub(1);
    let mut by_opening: Vec<(usize, &OpenPanel)> = panels.open().iter().enumerate().collect();
    by_opening.sort_by_key(|(_, panel)| panel.id());

    rsx! {
        div { class: "inspectors",
            for (index, (layer, panel)) in by_opening.into_iter().enumerate() {
                // The key goes on the loop's own node, Dioxus doesn't see one inside the `match`.
                // Without it, closing a panel would hand its state, like its place, to the next one.
                Fragment { key: "{panel.id().0}",
                    match panel {
                        OpenPanel::Inspector(inspector) => rsx! {
                            InspectorPanel {
                                state,
                                inspector: inspector.clone(),
                                layer,
                                in_front: layer == front,
                                index,
                                scene_rect,
                                openings,
                                return_to,
                            }
                        },
                        OpenPanel::Map(id) => rsx! {
                            MapPanel {
                                state,
                                id: *id,
                                layer,
                                in_front: layer == front,
                                index,
                                scene_rect,
                                openings,
                                return_to,
                            }
                        },
                    }
                }
            }
        }
    }
}

/// One inspector: its pin control, history and record, in a floating panel.
///
/// `layer` is its place in the front order, `index` in the opening order.
#[component]
fn InspectorPanel(
    mut state: Signal<AppState>,
    inspector: Inspector,
    layer: usize,
    in_front: bool,
    index: usize,
    scene_rect: Signal<Option<SceneRect>>,
    openings: Openings,
    return_to: Signal<ReturnTo>,
) -> Element {
    let id = inspector.id;
    let record = inspector.record();
    // The body's scroll position for each record it showed, restored when going Back to one.
    let mut scroll_tops = use_signal(HashMap::<Record, f64>::new);

    let body_id = format!("inspector-{}-body", id.0);
    {
        let body_id = body_id.clone();
        use_effect(use_reactive!(|record| {
            let top = scroll_tops.peek().get(&record).copied().unwrap_or(0.0);
            dom::scroll_top_soon(&body_id, top);
        }));
    }

    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;

    let (kind, title, label, name) = match record {
        Record::System(system) => {
            let system = dataset.system(system);
            ("system", "System", &system.label, &system.name)
        }
        Record::Component(component) => {
            let component = dataset.component(component);
            ("component", "Component", &component.label, &component.name)
        }
    };
    let (position, length) = inspector.position();
    let mut act = move |action| state.write().update(Action::Inspector(id, action));
    let Highlighting(relations) = use_context::<Highlighting>();
    let target = relations.read().as_ref().map(|relations| relations.target);
    let highlighted = target == Some(record);

    rsx! {
        FloatingPanel {
            state,
            id,
            kind,
            title,
            // Named like "System a::run".
            name: "{title} {label}",
            noun: "inspector",
            width: preferred_width(label),
            layer,
            in_front,
            index,
            scene_rect,
            openings,
            return_to,
            actions: rsx! {
                button {
                    class: "button",
                    r#type: "button",
                    aria_pressed: if highlighted { "true" } else { "false" },
                    onclick: move |_| state.write().update(Action::ToggleHighlight(record)),
                    "Highlight"
                }
                if inspector.pinned {
                    // Unpinning closes, the unpinned inspector might show something else.
                    button {
                        class: "button",
                        r#type: "button",
                        aria_label: "Unpin and close",
                        onclick: move |_| close(state, return_to, id, InspectorAction::TogglePin),
                        "Unpin"
                    }
                } else {
                    button {
                        class: "button",
                        r#type: "button",
                        onclick: move |_| act(InspectorAction::TogglePin),
                        "Pin"
                    }
                }
            },

            nav { class: "inspector-history", aria_label: "Inspector history",
                button {
                    class: "button",
                    r#type: "button",
                    disabled: !inspector.can_go_back(),
                    aria_label: "Back in this inspector",
                    onclick: move |_| act(InspectorAction::Back),
                    "← Back"
                }
                button {
                    class: "button",
                    r#type: "button",
                    disabled: !inspector.can_go_forward(),
                    aria_label: "Forward in this inspector",
                    onclick: move |_| act(InspectorAction::Forward),
                    "Forward →"
                }
                span { "{position} / {length}" }
            }

            // The same element whatever the record, so rerenders keep its scroll position and the
            // heading stays focused.
            div {
                id: body_id,
                class: "inspector-body",
                onscroll: move |event| {
                    scroll_tops.write().insert(record, event.data().scroll_top());
                },
                h2 { id: focus_id(id), tabindex: "-1", "{label}" }
                p { class: "full-name", "{name}" }

                match record {
                    Record::System(system) => system_details(state, openings, target, dataset, system),
                    Record::Component(component) => component_details(state, openings, target, dataset, component),
                }
            }
        }
    }
}

/// The storages `system` borrows, grouped by access, then where it runs.
fn system_details(
    state: Signal<AppState>,
    openings: Openings,
    target: Option<Record>,
    dataset: &Dataset,
    system: SystemId,
) -> Element {
    let system = dataset.system(system);
    let group = |mutability| -> Vec<Chip> {
        system
            .borrows
            .iter()
            .filter(|borrow| borrow.mutability == mutability)
            .map(|borrow| {
                let component = dataset.component(borrow.component);
                Chip {
                    record: Record::Component(borrow.component),
                    label: component.label.clone(),
                    name: component.name.clone(),
                }
            })
            .collect()
    };

    rsx! {
        if system.borrows.is_empty() {
            p { class: "inspector-note", "Borrows no storage." }
        }
        {chip_group(state, openings, target, Mutability::Exclusive, "Exclusive", group(Mutability::Exclusive))}
        {chip_group(state, openings, target, Mutability::Shared, "Shared", group(Mutability::Shared))}

        h3 { "Workload occurrences" }
        {occurrence_list(state, dataset, &system.occurrences)}
    }
}

/// The category of `component`, then the systems borrowing it, grouped by access.
fn component_details(
    state: Signal<AppState>,
    openings: Openings,
    target: Option<Record>,
    dataset: &Dataset,
    component: ComponentId,
) -> Element {
    let component = dataset.component(component);
    let group = |mutability| -> Vec<Chip> {
        component
            .borrowers
            .iter()
            .filter(|borrower| borrower.mutability == mutability)
            .map(|borrower| {
                let system = dataset.system(borrower.system);
                Chip {
                    record: Record::System(borrower.system),
                    label: system.label.clone(),
                    name: system.name.clone(),
                }
            })
            .collect()
    };

    rsx! {
        p { class: "inspector-note", "{storage_kind(&component.name)}" }
        {chip_group(state, openings, target, Mutability::Exclusive, "Exclusive borrowers", group(Mutability::Exclusive))}
        {chip_group(state, openings, target, Mutability::Shared, "Shared borrowers", group(Mutability::Shared))}
    }
}

/// A related record, inspected when clicked.
struct Chip {
    record: Record,
    label: String,
    /// The full name, the label can be shortened.
    name: String,
}

/// A heading with the number of chips, then the chips, or nothing without chips.
fn chip_group(
    state: Signal<AppState>,
    openings: Openings,
    target: Option<Record>,
    mutability: Mutability,
    heading: &str,
    chips: Vec<Chip>,
) -> Element {
    if chips.is_empty() {
        return rsx! {};
    }

    let access = match mutability {
        Mutability::Exclusive => "chip exclusive",
        Mutability::Shared => "chip shared",
    };

    rsx! {
        h3 { "{heading} · {chips.len()}" }
        ul { class: "chips",
            for chip in chips {
                li {
                    button {
                        // The highlighted record's chip stands out.
                        class: if target == Some(chip.record) { format!("{access} target") } else { access.to_string() },
                        r#type: "button",
                        title: "{chip.name}",
                        "aria-description": "{chip.name}",
                        // Chips can disappear with the record they're in, the inspector keeps
                        // returning focus to what opened it.
                        onclick: move |event| {
                            let pointer = click_point(&event);
                            openings.open(state, Action::Inspect(chip.record), pointer, None);
                        },
                        "{chip.label}"
                    }
                }
            }
        }
    }
}

/// `occurrences` grouped by workload, one button per occurrence to show it in the scene.
///
/// `occurrences` must be sorted by workload, like [`System::occurrences`](visualizer::model::System::occurrences).
fn occurrence_list(
    mut state: Signal<AppState>,
    dataset: &Dataset,
    occurrences: &[OccurrenceId],
) -> Element {
    let by_workload = occurrences
        .chunk_by(|a, b| dataset.occurrence(*a).workload == dataset.occurrence(*b).workload);

    rsx! {
        ul { class: "occurrences",
            for occurrences in by_workload {
                {
                    let workload = &dataset.workload(dataset.occurrence(occurrences[0]).workload).label;

                    rsx! {
                        li {
                            span { class: "occurrence-workload", "{workload}" }
                            for &id in occurrences {
                                {
                                    let occurrence = dataset.occurrence(id);
                                    let batch = occurrence.batch;
                                    // The visible text leaves the workload to the list item, a
                                    // button read on its own needs it.
                                    let name = if occurrence.thread_bound {
                                        format!("{workload}, batch {batch}, thread-bound slot")
                                    } else {
                                        format!("{workload}, batch {batch}")
                                    };

                                    rsx! {
                                        button {
                                            class: "button occurrence",
                                            r#type: "button",
                                            aria_label: name,
                                            onclick: move |_| state.write().update(Action::ShowOccurrence(id)),
                                            if occurrence.thread_bound {
                                                span { class: "tile-mark", "◆ " }
                                            }
                                            "Batch {batch}"
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
}
