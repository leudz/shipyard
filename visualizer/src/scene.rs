//! The workload scene: the selected workload's batches and system tiles, with the search panel
//! and zoom controls floating above them.
//!
//! The browser lays out each tile's text, [`layout_workload`] decides where tiles go from their
//! measured heights, and the [`Camera`] pans and zooms the result with a CSS transform.
//!
//! Panning changes the camera on every pointer move, so only the components reading the camera
//! render again. The tiles depend on the layout, which only changes with the zoom.

use crate::floating::{click_point, Openings};
use crate::inspector_ui::InspectorLayer;
use crate::search_ui::SearchPanel;
use crate::{count, dom};
use dioxus::html::geometry::WheelDelta;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use visualizer::camera::{self, Camera, ZOOM_STEP};
use visualizer::highlight::{Relation, Relations};
use visualizer::inspectors::Record;
use visualizer::layout::{layout_workload, BatchLayout, WorkloadLayout, TILE_WIDTH};
use visualizer::model::{Dataset, OccurrenceId, Workload, WorkloadId};
use visualizer::overview;
use visualizer::panels::{self, Point};
use visualizer::state::{Action, AppState, Loaded};

/// The hidden copy of the tiles we measure.
const MEASURE_ID: &str = "tile-measure";
/// The workload's region, focused when it has no tile to focus.
const WORKLOAD_ID: &str = "workload";
/// The area panning and zooming the workload, it captures the pointer while dragging.
const SURFACE_ID: &str = "scene-surface";

/// Where the workload's top edge is in the scene, below the search panel and workload heading.
const WORKLOAD_TOP: f64 = 190.0;
/// The navigation panel's height until it's measured.
const DEFAULT_NAVIGATION_HEIGHT: f64 = 96.0;
/// The room kept between the workload and the navigation panel, the panel's bottom offset
/// included.
const NAVIGATION_GAP: f64 = 40.0;
/// How tall an overview bar gets: a base height, plus up to the extra for the largest batch.
const BAR_BASE_HEIGHT: f64 = 16.0;
const BAR_EXTRA_HEIGHT: f64 = 10.0;
/// The narrowest bar showing its system count. Narrower bars leave it to their name and tooltip.
const COUNT_MIN_WIDTH: f64 = 28.0;
/// The room a count takes per digit: digits are about 6.6 pixels wide in the bars' 11px
/// monospace, plus some breathing room.
const COUNT_DIGIT_WIDTH: f64 = 8.0;
/// The narrowest the mark of the part of the workload on screen gets, so it stays visible.
const VIEWPORT_MIN_WIDTH: f64 = 2.0;
/// How many system names a batch's tooltip lists, the others are counted.
const BATCH_TOOLTIP_NAMES: usize = 12;
/// The slider's steps, from the workload's start to its end.
const SLIDER_STEPS: f64 = 1000.0;
/// The space between a tile and its tooltip.
const TOOLTIP_GAP: f64 = 5.0;
/// The widest a tooltip gets, `20rem` in the styles.
const TOOLTIP_MAX_WIDTH: f64 = 320.0;
/// The space kept between a tooltip and the scene's edges.
const TOOLTIP_MARGIN: f64 = 14.0;
/// How fast Ctrl+wheel zooms, per pixel of scrolling.
const WHEEL_ZOOM_SPEED: f64 = 0.0017;
/// How many pixels a wheel "line" scrolls.
const LINE_HEIGHT: f64 = 16.0;

/// Where the scene is in the window, in CSS pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SceneRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl SceneRect {
    pub fn size(self) -> panels::Size {
        panels::Size {
            width: self.width,
            height: self.height,
        }
    }

    /// Converts a client position to the scene's coordinates.
    pub fn to_scene(self, client: Point) -> Point {
        Point {
            x: client.x - self.left,
            y: client.y - self.top,
        }
    }
}

/// A pan in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    pointer: i32,
    /// The pointer's x when the drag started.
    start: f64,
    /// The camera's x when the drag started.
    camera_x: f64,
}

/// How every record relates to the highlight, `None` without one. Provided by the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlighting(pub Memo<Option<Relations>>);

/// The id of the element keyboard focus should move to when entering the scene.
///
/// That's the selected tile when the selected workload shows it, otherwise its first tile, or
/// the workload itself when it has no tiles. `None` without a workload.
pub fn entry_id(loaded: &Loaded) -> Option<String> {
    let workload = loaded.workspace.selected_workload?;
    let dataset = &loaded.dataset;

    if let Some(selected) = loaded.workspace.selected_occurrence {
        if dataset.occurrence(selected).workload == workload {
            return Some(tile_id(selected));
        }
    }

    let first = dataset
        .workload(workload)
        .batches
        .iter()
        .flat_map(|batch| batch.occurrences())
        .next();

    Some(first.map_or(WORKLOAD_ID.to_string(), tile_id))
}

/// Fills the window below the header.
#[component]
pub fn Scene(state: Signal<AppState>) -> Element {
    // Its height decides how many tiles fit in a column, its width what Fit fits into.
    let mut scene_rect = use_signal(|| None::<SceneRect>);
    let mut scene_element = use_signal(|| None::<Rc<MountedData>>);
    let openings = use_context_provider(|| Openings::new(Signal::new(None)));
    // The relations only change with the highlight, not with every pan.
    let highlight = use_memo(move || {
        let app = state.read();
        app.loaded
            .as_ref()
            .and_then(|loaded| loaded.workspace.highlight)
    });
    let relations = use_memo(move || {
        let target = highlight()?;
        let app = state.peek();

        Some(Relations::new(&app.loaded.as_ref()?.dataset, target))
    });
    use_context_provider(|| Highlighting(relations));

    let app = state.read();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };

    rsx! {
        div {
            class: "scene",
            onmounted: move |event| scene_element.set(Some(event.data())),
            // The size comes with the event. The position changes when the header wraps, which
            // resizes the scene too, so we ask for it then.
            onresize: move |event| async move {
                let Ok(size) = event.get_border_box_size() else {
                    return;
                };
                // On desktop, an element leaving the scene, like a closed inspector, reports a
                // last empty size that reaches us despite its handler stopping propagation. The
                // scene is never empty while it's shown, so an empty size isn't ours.
                if size.width <= 0.0 || size.height <= 0.0 {
                    return;
                }
                let previous = scene_rect.peek().unwrap_or_default();
                scene_rect.set(Some(SceneRect {
                    width: size.width,
                    height: size.height,
                    ..previous
                }));

                let Some(element) = scene_element() else {
                    return;
                };
                let Ok(rect) = element.get_client_rect().await else {
                    return;
                };
                let current = scene_rect.peek().unwrap_or_default();
                let (left, top) = (rect.origin.x, rect.origin.y);
                if (current.left, current.top) != (left, top) {
                    scene_rect.set(Some(SceneRect { left, top, ..current }));
                }
            },

            // The search panel comes first, matching what's on screen.
            SearchPanel { state }

            match loaded.workspace.selected_workload {
                // A new dataset or workload starts with fresh measurements.
                Some(workload) => rsx! {
                    WorkloadView {
                        key: "{loaded.generation}-{workload.0}",
                        state,
                        workload,
                        scene_rect,
                    }
                },
                None => rsx! {
                    p { class: "scene-note", "This export has no workloads." }
                },
            }

            // After the workload, inspectors are read after what they inspect.
            InspectorLayer {
                key: "{loaded.generation}",
                state,
                scene_rect,
                openings,
            }
        }
    }
}

/// The heading, batches, tiles, and zoom controls of `workload`.
///
/// It's keyed by dataset and workload, so the dataset and workload it reads never change while
/// it's mounted. That's why it can read them without subscribing to the state.
#[component]
fn WorkloadView(
    state: Signal<AppState>,
    workload: WorkloadId,
    scene_rect: Signal<Option<SceneRect>>,
) -> Element {
    // Tile heights, in the workload's display order. Tiles have a fixed width, so neither zoom
    // nor the window size changes them.
    let mut tile_heights = use_signal(|| None::<Vec<f64>>);
    // The tiles under the pointer and focused. We show the full name of the hovered one, or else
    // the focused one.
    let hovered = use_signal(|| None::<OccurrenceId>);
    let focused = use_signal(|| None::<OccurrenceId>);
    let mut drag = use_signal(|| None::<Drag>);

    let camera = use_memo(move || {
        state
            .read()
            .loaded
            .as_ref()
            .map_or_else(Camera::default, |loaded| loaded.workspace.camera)
    });
    let zoom = use_memo(move || camera().zoom);
    let framing = use_memo(move || {
        let app = state.read();
        app.loaded
            .as_ref()
            .and_then(|loaded| loaded.workspace.framing)
    });
    let selected = use_memo(move || {
        let app = state.read();
        app.loaded
            .as_ref()
            .and_then(|loaded| loaded.workspace.selected_occurrence)
    });
    let measured = use_memo(move || {
        let tile_heights = tile_heights.read();
        let app = state.peek();
        let (Some(tile_heights), Some(loaded)) = (tile_heights.as_ref(), app.loaded.as_ref())
        else {
            return None;
        };

        Some(measured_heights(
            loaded.dataset.workload(workload),
            tile_heights,
        ))
    });
    // The bottom panel's height, the workload's columns stop above it.
    let navigation_height = use_signal(|| DEFAULT_NAVIGATION_HEIGHT);
    let layout = use_memo(move || {
        let measured = measured.read();
        let app = state.peek();
        let (Some(measured), Some(size), Some(loaded)) =
            (measured.as_ref(), scene_rect(), app.loaded.as_ref())
        else {
            return None;
        };

        Some(layout_at(
            loaded.dataset.workload(workload),
            measured,
            size.height - navigation_height(),
            zoom(),
        ))
    });

    // An inspector asked to show an occurrence, we pan to its batch once we know where it is.
    use_effect(move || {
        let Some(occurrence) = framing() else {
            return;
        };
        let Some(size) = scene_rect() else {
            return;
        };
        let framed = {
            let layout = layout.read();
            let found = layout
                .iter()
                .flat_map(|layout| &layout.batches)
                .find_map(|batch| {
                    let tile = batch
                        .tiles
                        .iter()
                        .find(|tile| tile.occurrence == occurrence)?;
                    Some((batch.bounds, tile.bounds))
                });
            let Some((batch, tile)) = found else {
                return;
            };

            // A batch with many columns can push the tile past the right edge.
            camera
                .peek()
                .framing(batch.x)
                .revealing(tile.x, tile.right(), size.width)
        };

        state.write().update(Action::SetCamera(framed));
    });

    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;
    let workload_record = dataset.workload(workload);
    let system_count: usize = workload_record
        .batches
        .iter()
        .map(|batch| batch.occurrences().count())
        .sum();

    rsx! {
        // Focusable from script only, so leaving the search fields has somewhere to land.
        section {
            id: WORKLOAD_ID,
            class: "workload",
            tabindex: "-1",
            aria_labelledby: "workload-title",

            header { class: "workload-heading",
                span { "Workload" }
                h2 { id: "workload-title", "{workload_record.label}" }
                small {
                    "{count(workload_record.batches.len(), \"batch\")} · {count(system_count, \"system\")}"
                }
            }

            if workload_record.batches.is_empty() {
                p { class: "scene-note", "This workload has no batches." }
            }

            // The hidden copy renders every tile at its final width, so the browser wraps the
            // labels exactly as it will in the scene. We read the heights once it's in the document.
            if tile_heights.read().is_none() {
                div {
                    id: MEASURE_ID,
                    class: "tile-measure",
                    aria_hidden: "true",
                    onmounted: move |_| async move {
                        tile_heights.set(Some(dom::child_heights(MEASURE_ID).await));
                    },
                    for occurrence in workload_record.batches.iter().flat_map(|batch| batch.occurrences()) {
                        // The same element as in the scene, so the browser styles it the same.
                        button {
                            key: "{occurrence.0}",
                            class: "system-tile",
                            r#type: "button",
                            tabindex: "-1",
                            style: "width: {TILE_WIDTH}px;",
                            {tile_content(dataset, occurrence)}
                        }
                    }
                }
            }

            // Wheel and drag gestures only apply over the workload, the floating panels are
            // outside this element.
            div {
                id: SURFACE_ID,
                class: if drag.read().is_some() { "scene-surface panning" } else { "scene-surface" },
                onwheel: move |event| {
                    event.prevent_default();
                    let Some(size) = scene_rect() else {
                        return;
                    };

                    let (dx, dy) = wheel_pixels(event.delta(), size.width);
                    let current = camera();
                    let modifiers = event.modifiers();
                    let next = if modifiers.contains(Modifiers::CONTROL)
                        || modifiers.contains(Modifiers::META)
                    {
                        let anchor = event.client_coordinates().x - size.left;
                        current.zoomed(current.zoom * (-dy * WHEEL_ZOOM_SPEED).exp(), anchor)
                    } else {
                        // Vertical wheels and horizontal trackpad swipes both pan.
                        let distance = if dx.abs() > dy.abs() { dx } else { dy };
                        current.panned(-distance)
                    };
                    state.write().update(Action::SetCamera(next));
                },
                onpointerdown: move |event| {
                    if event.trigger_button() != Some(MouseButton::Primary) {
                        return;
                    }

                    let pointer = event.pointer_id();
                    drag.set(Some(Drag {
                        pointer,
                        start: event.client_coordinates().x,
                        camera_x: camera().x,
                    }));
                    dom::capture_pointer(SURFACE_ID, pointer);
                },
                onpointermove: move |event| {
                    let Some(current) = drag() else {
                        return;
                    };
                    if current.pointer != event.pointer_id() {
                        return;
                    }

                    let next = Camera {
                        x: current.camera_x + event.client_coordinates().x - current.start,
                        ..camera()
                    };
                    state.write().update(Action::SetCamera(next));
                },
                onpointerup: move |_| drag.set(None),
                onpointercancel: move |_| drag.set(None),
                onlostpointercapture: move |_| drag.set(None),

                World {
                    state,
                    layout,
                    camera,
                    selected,
                    hovered,
                    focused,
                    scene_rect,
                }
            }
        }

        TileTooltip {
            state,
            hovered,
            focused,
            layout,
            camera,
            scene_rect,
        }
        Navigation {
            state,
            workload,
            camera,
            measured,
            layout,
            scene_rect,
            navigation_height,
        }
    }
}

/// Moves the batches by the camera.
#[component]
fn World(
    state: Signal<AppState>,
    layout: Memo<Option<WorkloadLayout>>,
    camera: Memo<Camera>,
    selected: Memo<Option<OccurrenceId>>,
    hovered: Signal<Option<OccurrenceId>>,
    focused: Signal<Option<OccurrenceId>>,
    scene_rect: Signal<Option<SceneRect>>,
) -> Element {
    let Camera { x, zoom } = camera();
    let layout_size = layout
        .read()
        .as_ref()
        .map(|layout| (layout.width, layout.height));
    let Some((width, height)) = layout_size else {
        return rsx! {};
    };

    rsx! {
        div {
            class: "scene-world",
            style: "transform: translate({x}px, {WORKLOAD_TOP}px) scale({zoom}); width: {width}px; height: {height}px;",
            // Only the transform above changes while panning, the batches don't render again.
            Batches {
                state,
                layout,
                camera,
                selected,
                hovered,
                focused,
                scene_rect,
            }
        }
    }
}

/// The batches and tiles of the layout.
#[component]
fn Batches(
    mut state: Signal<AppState>,
    layout: Memo<Option<WorkloadLayout>>,
    camera: Memo<Camera>,
    selected: Memo<Option<OccurrenceId>>,
    mut hovered: Signal<Option<OccurrenceId>>,
    mut focused: Signal<Option<OccurrenceId>>,
    scene_rect: Signal<Option<SceneRect>>,
) -> Element {
    let openings = use_context::<Openings>();
    let Highlighting(relations) = use_context::<Highlighting>();
    let relations = relations.read();
    let layout = layout.read();
    let Some(layout) = layout.as_ref() else {
        return rsx! {};
    };
    let selected = selected();
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;

    rsx! {
        for (index, batch) in layout.batches.iter().enumerate() {
            section {
                key: "{index}",
                class: "batch",
                role: "group",
                aria_labelledby: "batch-{index}-title",
                // Every batch spans the tallest one, so their separators line up.
                style: "left: {batch.bounds.x}px; width: {batch.bounds.width}px; height: {layout.height}px;",

                header { class: "batch-header",
                    h3 { id: "batch-{index}-title", "Batch {index}" }
                    span { "{count(batch.tiles.len(), \"system\")}" }
                }

                // Tiles come in display order, so reading and Tab order follow execution
                // order even across columns.
                for tile in &batch.tiles {
                    {
                        let occurrence = tile.occurrence;
                        let bounds = tile.bounds;
                        let is_selected = selected == Some(occurrence);
                        let relation = relations.as_ref().map(|relations| {
                            relations.system(dataset.occurrence(occurrence).system)
                        });
                        let full_name = &dataset.system(dataset.occurrence(occurrence).system).name;

                        rsx! {
                            button {
                                key: "{occurrence.0}",
                                id: tile_id(occurrence),
                                class: tile_class(is_selected, relation),
                                r#type: "button",
                                aria_label: tile_name(dataset, occurrence, relation),
                                "aria-description": "{full_name}",
                                aria_current: is_selected.then_some("true"),
                                style: "left: {bounds.x - batch.bounds.x}px; top: {bounds.y - batch.bounds.y}px; width: {bounds.width}px; height: {bounds.height}px;",
                                onclick: move |event| {
                                    let pointer = click_point(&event);
                                    let action = Action::SelectOccurrence(occurrence);
                                    openings.open(state, action, pointer, Some(tile_id(occurrence)));
                                },
                                // Dragging from a tile doesn't pan, the press is for the tile.
                                onpointerdown: move |event| event.stop_propagation(),
                                onmouseenter: move |_| hovered.set(Some(occurrence)),
                                onmouseleave: move |_| clear(hovered, occurrence),
                                onfocus: move |_| {
                                    focused.set(Some(occurrence));

                                    // Tab can reach tiles outside the screen, we pan to show them.
                                    let Some(size) = *scene_rect.peek() else {
                                        return;
                                    };
                                    let current = *camera.peek();
                                    let revealed = current.revealing(bounds.x, bounds.right(), size.width);
                                    if revealed != current {
                                        state.write().update(Action::SetCamera(revealed));
                                    }
                                },
                                onblur: move |_| clear(focused, occurrence),
                                {tile_content(dataset, occurrence)}
                                if let Some(mark) = relation.and_then(access_mark) {
                                    span { class: "tile-access", aria_hidden: "true", "{mark}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Clears `tile`, unless a tile other than `occurrence` replaced it since.
fn clear(mut tile: Signal<Option<OccurrenceId>>, occurrence: OccurrenceId) {
    if *tile.peek() == Some(occurrence) {
        tile.set(None);
    }
}

/// Shows the full name of the hovered or focused tile above it.
///
/// It's outside the zoomed world, so it stays readable at any zoom.
#[component]
fn TileTooltip(
    state: Signal<AppState>,
    hovered: Signal<Option<OccurrenceId>>,
    focused: Signal<Option<OccurrenceId>>,
    layout: Memo<Option<WorkloadLayout>>,
    camera: Memo<Camera>,
    scene_rect: Signal<Option<SceneRect>>,
) -> Element {
    let Some(occurrence) = hovered().or(focused()) else {
        return rsx! {};
    };
    let layout = layout.read();
    let Some(tile) = layout
        .iter()
        .flat_map(|layout| &layout.batches)
        .flat_map(|batch| &batch.tiles)
        .find(|tile| tile.occurrence == occurrence)
    else {
        return rsx! {};
    };
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };

    let occurrence = loaded.dataset.occurrence(occurrence);
    let system = loaded.dataset.system(occurrence.system);
    let camera = camera();
    let bottom = WORKLOAD_TOP + tile.bounds.y * camera.zoom - TOOLTIP_GAP;
    // We line the tooltip up with the tile's left edge, or its right edge when a full width
    // tooltip wouldn't fit on screen that way. We write both sides every time: Dioxus updates the
    // style declaration by declaration, so the side we stop writing would keep its last value.
    let left = camera.to_screen(tile.bounds.x);
    let scene_width = scene_rect().map_or(f64::INFINITY, |size| size.width);
    let horizontal = if left + TOOLTIP_MAX_WIDTH > scene_width - TOOLTIP_MARGIN {
        let right = scene_width - camera.to_screen(tile.bounds.right());
        format!("left: auto; right: {}px;", right.max(TOOLTIP_MARGIN))
    } else {
        format!("left: {}px; right: auto;", left.max(TOOLTIP_MARGIN))
    };

    rsx! {
        // Screen readers get the full name as the tile's description instead.
        div {
            class: "tile-tooltip",
            aria_hidden: "true",
            style: "{horizontal} top: {bottom}px;",
            "{system.name}"
            if occurrence.thread_bound {
                " · Thread-bound slot"
            }
        }
    }
}

/// What's highlighted, with a button to stop, and whether it's in the shown workload.
///
/// It's a status, so screen readers announce a new highlight once, without reading every tile.
#[component]
fn HighlightSummary(mut state: Signal<AppState>, workload: WorkloadId) -> Element {
    let Highlighting(relations) = use_context::<Highlighting>();
    let relations = relations.read();
    let app = state.peek();

    let summary = relations
        .as_ref()
        .zip(app.loaded.as_ref())
        .map(|(relations, loaded)| {
            let dataset = &loaded.dataset;
            let (kind, label) = match relations.target {
                Record::System(system) => ("System", &dataset.system(system).label),
                Record::Component(component) => ("Component", &dataset.component(component).label),
            };
            // Tiles of this workload marked by the highlight.
            let marked = dataset
                .workload(workload)
                .batches
                .iter()
                .flat_map(|batch| batch.occurrences())
                .filter(|&occurrence| {
                    let relation = relations.system(dataset.occurrence(occurrence).system);
                    matches!(
                        relation,
                        Relation::Target | Relation::Exclusive | Relation::Shared
                    )
                })
                .count();

            (relations.target, kind, label.clone(), marked)
        });

    rsx! {
        div { class: "highlight-summary", role: "status",
            if let Some((target, kind, label, marked)) = summary {
                span { "Highlight · {kind} · " b { "{label}" } }
                if matches!(target, Record::Component(_)) {
                    span { class: "legend-exclusive", "EX exclusive" }
                    span { class: "legend-shared", "SH shared" }
                }
                if marked == 0 {
                    span { class: "highlight-absent", "Not in this workload" }
                } else {
                    span { "{count(marked, \"tile\")} here" }
                }
                button {
                    class: "button",
                    r#type: "button",
                    aria_label: "Clear highlight",
                    onclick: move |_| state.write().update(Action::ToggleHighlight(target)),
                    "×"
                }
            } else {
                // Always a line, so highlighting doesn't change the panel's height and reflow
                // the workload above it.
                span { class: "highlight-absent", "Nothing highlighted" }
            }
        }
    }
}

/// The bottom panel: the highlight and the zoom buttons, then the batch overview.
///
/// It measures itself into `navigation_height`, which the layout keeps free.
#[component]
fn Navigation(
    mut state: Signal<AppState>,
    workload: WorkloadId,
    camera: Memo<Camera>,
    measured: Memo<Option<HashMap<OccurrenceId, f64>>>,
    layout: Memo<Option<WorkloadLayout>>,
    scene_rect: Signal<Option<SceneRect>>,
    mut navigation_height: Signal<f64>,
) -> Element {
    let percent = (camera().zoom * 100.0).round();

    let fit = move |_| {
        let (Some(size), Some(measured)) = (scene_rect(), measured()) else {
            return;
        };

        let fitted = {
            let app = state.peek();
            let Some(loaded) = &app.loaded else {
                return;
            };
            let workload = loaded.dataset.workload(workload);

            let height = size.height - *navigation_height.peek();
            camera::fit(size.width, |zoom| {
                layout_at(workload, &measured, height, zoom).width
            })
        };
        state.write().update(Action::SetCamera(fitted));
    };

    rsx! {
        nav {
            class: "panel navigation",
            aria_label: "Workload navigation",
            onresize: move |event| {
                // The desktop renderer bubbles resize events, the scene would take our size as
                // its own.
                event.stop_propagation();
                let Ok(size) = event.get_border_box_size() else {
                    return;
                };
                if *navigation_height.peek() != size.height {
                    navigation_height.set(size.height);
                }
            },

            div { class: "navigation-header",
            HighlightSummary { state, workload }
            div { class: "zoom-controls", role: "group", aria_label: "Workload zoom",
                button {
                    class: "button",
                    r#type: "button",
                    aria_label: "Zoom out",
                    onclick: move |_| zoom_to(state, camera, scene_rect, camera().zoom / ZOOM_STEP),
                    "−"
                }
                button {
                    class: "button",
                    r#type: "button",
                    aria_label: "Zoom {percent}%, reset to 100%",
                    onclick: move |_| zoom_to(state, camera, scene_rect, 1.0),
                    "{percent}%"
                }
                button {
                    class: "button",
                    r#type: "button",
                    aria_label: "Zoom in",
                    onclick: move |_| zoom_to(state, camera, scene_rect, camera().zoom * ZOOM_STEP),
                    "+"
                }
                button {
                    class: "button",
                    r#type: "button",
                    onclick: fit,
                    "Fit"
                }
            }
            }
            Overview {
                state,
                workload,
                camera,
                layout,
                scene_rect,
            }
        }
    }
}

/// One bar per batch, to frame it; then a slider across the workload, next to a reminder of how
/// to read it.
///
/// The strip shows the whole workload through a lens following the camera, see
/// [`overview::lens`]: batches on screen get wide bars, the first or last one at the workload's
/// ends, the others get narrower the further they are. A mark under the bars shows the part of the workload on screen. Bars
/// show the batch's tile count by their height, and in digits when they're wide enough, whether
/// it's on screen, and its tiles' relations to the highlight.
#[component]
fn Overview(
    mut state: Signal<AppState>,
    workload: WorkloadId,
    camera: Memo<Camera>,
    layout: Memo<Option<WorkloadLayout>>,
    scene_rect: Signal<Option<SceneRect>>,
) -> Element {
    let Highlighting(relations) = use_context::<Highlighting>();
    // The strip's width, `None` until it's measured.
    let mut strip_width = use_signal(|| None::<f64>);
    // The batch whose bar is hovered or focused, it shows its systems.
    let mut hovered = use_signal(|| None::<usize>);
    let relations = relations.read();
    let layout = layout.read();
    let Some(layout) = layout.as_ref() else {
        return rsx! {};
    };
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;
    let current = camera();
    let width = scene_rect().map_or(0.0, |rect| rect.width);
    let strip = strip_width().unwrap_or(0.0);
    // Nothing to draw until the strip is measured.
    let lens = overview::lens(layout, current, width, strip);

    let largest = layout
        .batches
        .iter()
        .map(|batch| batch.tiles.len())
        .max()
        .unwrap_or(0);
    // Batches are in order, so the ones on screen are a single run.
    let on_screen =
        |batch: &BatchLayout| current.shows(batch.bounds.x, batch.bounds.right(), width);
    let first_shown = layout.batches.iter().position(on_screen);
    let last_shown = layout.batches.iter().rposition(on_screen);
    let shown = match (first_shown, last_shown) {
        (Some(first), Some(last)) => first..last + 1,
        _ => 0..0,
    };
    let batch_count = layout.batches.len();
    let range = match (first_shown, last_shown) {
        (Some(first), Some(last)) if first == last => format!("Batch {first} of {batch_count}"),
        (Some(first), Some(last)) => format!("Batches {first}–{last} of {batch_count}"),
        _ if batch_count == 0 => "No batches".to_string(),
        _ => format!("None of {batch_count}"),
    };
    // As wide as the longest label this workload can show, so the slider doesn't move when the
    // label changes. Digits are all as wide as `ch`, letters are narrower.
    let range_width = format!("Batches {0}–{0} of {0}", batch_count)
        .chars()
        .count();
    let position = (current.position(layout.width, width) * SLIDER_STEPS).round();
    let layout_width = layout.width;

    rsx! {
        div {
            class: "overview",
            role: "group",
            aria_label: "Batches of {dataset.workload(workload).label}",
            onresize: move |event| {
                // The desktop renderer bubbles resize events, the scene would take our size as
                // its own.
                event.stop_propagation();
                let Ok(size) = event.get_border_box_size() else {
                    return;
                };
                if *strip_width.peek() != Some(size.width) {
                    strip_width.set(Some(size.width));
                }
            },

            if let Some(lens) = &lens {
                for (index, (batch, span)) in layout.batches.iter().zip(&lens.batches).enumerate() {
                    {
                        let bounds = batch.bounds;
                        let tiles = batch.tiles.len();
                        let relations = relations.as_ref();
                        let cue = batch_cue(dataset, relations, batch.tiles.iter().map(|tile| tile.occurrence));
                        let on_screen = shown.contains(&index);
                        // `max` keeps empty workloads and batches from dividing by zero.
                        let height = BAR_BASE_HEIGHT + BAR_EXTRA_HEIGHT * (tiles as f64 / largest.max(1) as f64).sqrt();
                        let left = span.left;
                        let span_width = span.width();
                        // Narrow bars leave their count to the name and the tooltip, rather than
                        // clipping it.
                        let digits = tiles.to_string().len() as f64;
                        let shows_count = span_width >= COUNT_MIN_WIDTH.max(digits * COUNT_DIGIT_WIDTH);
                        let place = if on_screen { ", on screen" } else { "" };
                        let name = format!("Batch {index}, {}{}{place}", count(tiles, "system"), cue.description);
                        let mut class = String::from("overview-batch");
                        if on_screen {
                            class.push_str(" on-screen");
                        }
                        class.push_str(cue.class);
                        // Bars in the strip's right half open their tooltip leftwards. Either way
                        // the tooltip gets at most the room left to the strip's edge, so it stays in
                        // the panel.
                        let anchor_right = 2.0 * left + span_width > strip;
                        let tooltip_room = if anchor_right { span.right } else { strip - left };
                        let mut leave = move || {
                            if *hovered.peek() == Some(index) {
                                hovered.set(None);
                            }
                        };

                        rsx! {
                            button {
                                key: "{index}",
                                class,
                                r#type: "button",
                                aria_label: name,
                                style: "left: {left}px; width: {span_width}px;",
                                onclick: move |_| {
                                    let framed = camera.peek().framing(bounds.x);
                                    state.write().update(Action::SetCamera(framed));
                                },
                                onmouseenter: move |_| hovered.set(Some(index)),
                                onmouseleave: move |_| leave(),
                                onfocus: move |_| hovered.set(Some(index)),
                                onblur: move |_| leave(),
                                span {
                                    class: "overview-bar",
                                    aria_hidden: "true",
                                    style: "height: {height}px; {cue.background}",
                                    if shows_count {
                                        "{tiles}"
                                    }
                                }
                                if hovered() == Some(index) {
                                    {batch_tooltip(dataset, batch, index, anchor_right, tooltip_room)}
                                }
                            }
                        }
                    }
                }
                if let Some(viewport) = lens.viewport {
                    {
                        // Wide enough to see, without leaving the strip.
                        let marker_width = viewport.width().max(VIEWPORT_MIN_WIDTH).min(strip);
                        let marker_left = viewport.left.min(strip - marker_width);

                        rsx! {
                            div {
                                class: "overview-viewport",
                                aria_hidden: "true",
                                style: "left: {marker_left}px; width: {marker_width}px;",
                            }
                        }
                    }
                }
            }
        }
        div { class: "overview-footer",
        label { class: "overview-position",
            span { class: "overview-range", style: "width: {range_width}ch;", "{range}" }
            input {
                r#type: "range",
                min: "0",
                max: "{SLIDER_STEPS}",
                value: "{position}",
                disabled: layout.batches.is_empty(),
                aria_label: "Workload position",
                aria_valuetext: "{range}",
                oninput: move |event| {
                    let Ok(value) = event.value().parse::<f64>() else {
                        return;
                    };
                    let moved = camera.peek().at_position(value / SLIDER_STEPS, layout_width, width);
                    state.write().update(Action::SetCamera(moved));
                },
            }
        }
        p { class: "navigation-status", "No ordering within a parallel batch" }
        }
    }
}

/// The tooltip above a batch's overview bar: its number and its systems, in display order.
///
/// It's for the pointer and the eyes, the bar's name already has the batch and its system count.
/// It opens rightwards from the bar's left edge, or leftwards from its right edge for
/// `anchor_right`, and gets at most `room` pixels.
fn batch_tooltip(
    dataset: &Dataset,
    batch: &BatchLayout,
    index: usize,
    anchor_right: bool,
    room: f64,
) -> Element {
    let tiles = batch.tiles.len();
    let more = tiles.saturating_sub(BATCH_TOOLTIP_NAMES);
    let class = if anchor_right {
        "batch-tooltip anchor-right"
    } else {
        "batch-tooltip"
    };

    rsx! {
        div { class, aria_hidden: "true", style: "max-width: min(20rem, {room}px);",
            div { class: "batch-tooltip-title", "Batch {index} · {count(tiles, \"system\")}" }
            for tile in batch.tiles.iter().take(BATCH_TOOLTIP_NAMES) {
                div { key: "{tile.occurrence.0}", {tile_content(dataset, tile.occurrence)} }
            }
            if more > 0 {
                div { class: "batch-tooltip-more", "+ {more} more" }
            }
        }
    }
}

/// How a batch shows the highlight on its overview bar.
struct BatchCue {
    /// Added to the bar's classes.
    class: &'static str,
    /// The bar's color, for matches.
    background: &'static str,
    /// Added to the bar's name.
    description: String,
}

/// The cue of a batch with `occurrences`: which of them the highlight marks, and how. A batch
/// with both exclusive and shared borrowers keeps both colors.
fn batch_cue(
    dataset: &Dataset,
    relations: Option<&Relations>,
    occurrences: impl Iterator<Item = OccurrenceId>,
) -> BatchCue {
    let none = BatchCue {
        class: "",
        background: "",
        description: String::new(),
    };
    let Some(relations) = relations else {
        return none;
    };

    let (mut target, mut exclusive, mut shared) = (0, 0, 0);
    for occurrence in occurrences {
        match relations.system(dataset.occurrence(occurrence).system) {
            Relation::Target => target += 1,
            Relation::Exclusive => exclusive += 1,
            Relation::Shared => shared += 1,
            Relation::SameKind | Relation::Unrelated => {}
        }
    }

    let background = match (target > 0, exclusive > 0, shared > 0) {
        (_, true, true) => "background: linear-gradient(90deg, var(--red) 50%, var(--blue) 50%);",
        (_, true, false) => "background: var(--red);",
        (_, false, true) => "background: var(--blue);",
        (true, false, false) => "background: var(--accent);",
        (false, false, false) => {
            return BatchCue {
                class: " muted",
                description: ", nothing highlighted".to_string(),
                ..none
            };
        }
    };
    let description = if target > 0 {
        format!(", {target} highlighted")
    } else {
        let mut parts = Vec::new();
        if exclusive > 0 {
            parts.push(format!("{exclusive} exclusive"));
        }
        if shared > 0 {
            parts.push(format!("{shared} shared"));
        }
        format!(", {}", parts.join(" and "))
    };

    BatchCue {
        class: " highlighted",
        background,
        description,
    }
}

/// Zooms around the middle of the scene, like the zoom buttons do.
fn zoom_to(
    mut state: Signal<AppState>,
    camera: Memo<Camera>,
    scene_rect: Signal<Option<SceneRect>>,
    zoom: f64,
) {
    let Some(size) = scene_rect() else {
        return;
    };

    let next = camera().zoomed(zoom, size.width / 2.0);
    state.write().update(Action::SetCamera(next));
}

/// Pairs the workload's occurrences, in display order, with the heights measured in that order.
fn measured_heights(workload: &Workload, tile_heights: &[f64]) -> HashMap<OccurrenceId, f64> {
    workload
        .batches
        .iter()
        .flat_map(|batch| batch.occurrences())
        .zip(tile_heights.iter().copied())
        .collect()
}

/// Lays out `workload` at `zoom` for a scene whose height above the navigation panel is
/// `scene_height`.
///
/// Zooming out shrinks everything on screen, so the columns can hold more scene pixels.
fn layout_at(
    workload: &Workload,
    measured: &HashMap<OccurrenceId, f64>,
    scene_height: f64,
    zoom: f64,
) -> WorkloadLayout {
    let available_height = (scene_height - WORKLOAD_TOP - NAVIGATION_GAP) / zoom;

    layout_workload(
        workload,
        |occurrence| measured.get(&occurrence).copied().unwrap_or(f64::NAN),
        available_height,
    )
}

/// A wheel event's movement in pixels.
fn wheel_pixels(delta: WheelDelta, page_width: f64) -> (f64, f64) {
    match delta {
        WheelDelta::Pixels(pixels) => (pixels.x, pixels.y),
        WheelDelta::Lines(lines) => (lines.x * LINE_HEIGHT, lines.y * LINE_HEIGHT),
        WheelDelta::Pages(pages) => (pages.x * page_width, pages.y * page_width),
    }
}

/// What a tile shows, the same in the scene and in the hidden copy we measure.
fn tile_content(dataset: &Dataset, occurrence: OccurrenceId) -> Element {
    let occurrence = dataset.occurrence(occurrence);
    let system = dataset.system(occurrence.system);

    rsx! {
        span { class: "tile-label",
            if occurrence.thread_bound {
                span { class: "tile-mark", "◆ " }
            }
            "{system.label}"
        }
    }
}

/// The tile's accessible name, with its batch since the same system can appear in several, and
/// its relation to the highlight.
fn tile_name(dataset: &Dataset, occurrence: OccurrenceId, relation: Option<Relation>) -> String {
    let occurrence = dataset.occurrence(occurrence);
    let label = &dataset.system(occurrence.system).label;
    let slot = if occurrence.thread_bound {
        ", thread-bound slot"
    } else {
        ""
    };
    let highlight = match relation {
        Some(Relation::Target) => ", highlighted",
        Some(Relation::Exclusive) => ", exclusive borrower",
        Some(Relation::Shared) => ", shared borrower",
        _ => "",
    };

    format!("{label}{slot}, batch {}{highlight}", occurrence.batch)
}

/// A tile's classes: selected, and its relation to the highlight. Unrelated tiles are dimmed but
/// stay reachable.
fn tile_class(selected: bool, relation: Option<Relation>) -> String {
    let highlight = match relation {
        None => "",
        Some(Relation::Target) => " target",
        Some(Relation::Exclusive) => " exclusive",
        Some(Relation::Shared) => " shared",
        Some(Relation::SameKind | Relation::Unrelated) => " dimmed",
    };
    let selected = if selected { " selected" } else { "" };

    format!("system-tile{selected}{highlight}")
}

/// The short access label of a borrow relation, `None` for other relations.
pub fn access_mark(relation: Relation) -> Option<&'static str> {
    match relation {
        Relation::Exclusive => Some("EX"),
        Relation::Shared => Some("SH"),
        _ => None,
    }
}

fn tile_id(occurrence: OccurrenceId) -> String {
    format!("tile-{}", occurrence.0)
}
