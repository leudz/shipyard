//! Floating panels, the inspectors and the global map: placed next to the click that opened them,
//! moved by their title, resized by their corner, and kept on screen when the scene shrinks.
//!
//! Their position is only view state, it isn't part of the dataset or the app state.

use crate::dom;
use crate::import_ui::OPEN_FILE_ID;
use crate::scene::{self, SceneRect};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use std::collections::HashMap;
use visualizer::inspectors::{InspectorAction, InspectorId};
use visualizer::panels::{Panel, Point};
use visualizer::state::{Action, AppState};

/// How far one arrow key press moves or resizes a panel, and with Shift.
const KEY_STEP: f64 = 16.0;
const SHIFT_KEY_STEP: f64 = 64.0;

/// How a panel opening about to be dispatched was started, for the panel that shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Opening {
    /// Where the click was, in client coordinates. `None` for keyboard activations.
    pointer: Option<Point>,
    /// The element focus returns to when leaving the panel. `None` keeps the panel's previous
    /// one, for chips inside inspectors.
    return_to: Option<String>,
}

/// Panels opened from the interface go through this, provided by the scene as context.
///
/// Opening moves focus into the panel, which places itself next to the click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Openings(Signal<Option<Opening>>);

impl Openings {
    pub fn new(signal: Signal<Option<Opening>>) -> Openings {
        Openings(signal)
    }

    /// Dispatches `action`, which brings a panel to the front: an inspection, a tile selection,
    /// or opening the map.
    ///
    /// `pointer` is where it was clicked, see [`click_point`], and `return_to` the element focus
    /// returns to when leaving the panel.
    pub fn open(
        mut self,
        mut state: Signal<AppState>,
        action: Action,
        pointer: Option<Point>,
        return_to: Option<String>,
    ) {
        self.0.set(Some(Opening { pointer, return_to }));
        state.write().update(action);
    }

    /// Where the panel opening now should go, read once when it mounts.
    fn pointer(self) -> Option<Point> {
        self.0.peek().as_ref().and_then(|opening| opening.pointer)
    }
}

/// Where `event` clicked, or `None` when a key activated the element: browsers give keyboard
/// clicks client coordinates 0, 0.
pub fn click_point(event: &MouseEvent) -> Option<Point> {
    let client = event.client_coordinates();
    let pointer = Point {
        x: client.x,
        y: client.y,
    };

    (pointer.x != 0.0 || pointer.y != 0.0).then_some(pointer)
}

/// Moves focus to the front panel after an [`Openings::open`], and remembers where it returns.
///
/// Call it once, from the component rendering the panels.
pub fn use_opening_focus(state: Signal<AppState>, openings: Openings) -> Signal<ReturnTo> {
    let mut return_to = use_signal(ReturnTo::new);

    // The opened panel rendered meanwhile, reading the opening to place itself.
    use_effect(move || {
        let Some(opening) = openings.0() else {
            return;
        };
        let mut opening_signal = openings.0;
        opening_signal.set(None);

        let front = {
            let app = state.peek();
            let loaded = app.loaded.as_ref();
            loaded.and_then(|loaded| {
                loaded
                    .workspace
                    .inspectors
                    .open()
                    .last()
                    .map(|panel| panel.id())
            })
        };
        let Some(front) = front else {
            return;
        };

        if let Some(element) = opening.return_to {
            return_to.write().insert(front, element);
        }
        dom::focus_soon(&focus_id(front));
    });

    return_to
}

/// The element each panel returns focus to.
pub type ReturnTo = HashMap<InspectorId, String>;

/// The element of panel `id` focus moves to when it opens, its heading.
pub fn focus_id(id: InspectorId) -> String {
    format!("panel-{}-title", id.0)
}

/// What a pointer drag on a panel's handle changes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum GestureKind {
    Move,
    Resize,
}

/// A drag in progress on a handle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Gesture {
    kind: GestureKind,
    pointer: i32,
    /// The pointer's client position when the drag started.
    start: Point,
    /// The panel when the drag started, with its height for resizes.
    panel: Panel,
}

/// A floating panel: a title to move it by, `actions` and a close button beside it, the
/// `children`, and a resize handle in the corner.
///
/// `layer` is its place in the front order, `index` in the opening order. `kind` styles it,
/// `title` is what its header shows, `name` its accessible name and `noun` how its handles name
/// it. It opens `width` wide.
///
/// Pressing anywhere in it or moving keyboard focus into it brings it to the front. Escape
/// returns focus to what opened it.
#[component]
pub fn FloatingPanel(
    mut state: Signal<AppState>,
    id: InspectorId,
    kind: &'static str,
    title: &'static str,
    name: String,
    noun: &'static str,
    width: f64,
    layer: usize,
    in_front: bool,
    index: usize,
    scene_rect: Signal<Option<SceneRect>>,
    openings: Openings,
    return_to: Signal<ReturnTo>,
    actions: Element,
    children: Element,
) -> Element {
    // Placed once, when it opens: next to the click that opened it, or cascading without one.
    let mut panel = use_signal(|| {
        let scene = scene_rect.peek().unwrap_or_default();

        match openings.pointer() {
            Some(pointer) => Panel::opened_at(scene.to_scene(pointer), width, scene.size()),
            None => Panel::cascaded(index, width, scene.size()),
        }
    });
    // Its height as rendered, where a resize of a panel as tall as its content starts.
    let mut rendered_height = use_signal(|| 0.0);
    let mut gesture = use_signal(|| None::<Gesture>);

    // The panel as placed, moved back inside the scene when the window shrank since.
    let scene = scene_rect().unwrap_or_default().size();
    let shown = panel().fitted(scene);
    // Both properties every time: Dioxus updates the style declaration by declaration, so one we
    // stop writing would keep its last value.
    let height = match shown.height {
        Some(height) => format!("height: {height}px; max-height: none;"),
        None => format!("height: auto; max-height: {}px;", shown.max_height(scene)),
    };
    let style = format!(
        "z-index: {layer}; left: {}px; top: {}px; width: {}px; {height}",
        shown.x, shown.y, shown.width
    );

    let grip_id = format!("panel-{}-grip", id.0);
    let handle_id = format!("panel-{}-resize", id.0);
    let mut act = move |action| state.write().update(Action::Inspector(id, action));
    let mut start = move |event: PointerEvent, kind: GestureKind, element: &str| {
        if event.trigger_button() != Some(MouseButton::Primary) {
            return;
        }

        let client = event.client_coordinates();
        let panel = match kind {
            GestureKind::Move => shown,
            GestureKind::Resize => Panel {
                height: Some(shown.height.unwrap_or(*rendered_height.peek())),
                ..shown
            },
        };
        gesture.set(Some(Gesture {
            kind,
            pointer: event.pointer_id(),
            start: Point {
                x: client.x,
                y: client.y,
            },
            panel,
        }));
        dom::capture_pointer(element, event.pointer_id());
    };
    let drag = move |event: PointerEvent| {
        let Some(current) = gesture() else {
            return;
        };
        if current.pointer != event.pointer_id() {
            return;
        }

        let client = event.client_coordinates();
        let dx = client.x - current.start.x;
        let dy = client.y - current.start.y;
        let start = current.panel;
        panel.set(match current.kind {
            GestureKind::Move => start.moved(dx, dy, *rendered_height.peek(), scene),
            GestureKind::Resize => start.resized(
                start.width + dx,
                start.height.unwrap_or_default() + dy,
                scene,
            ),
        });
    };
    let mut end = move || gesture.set(None);
    let moving = gesture().is_some_and(|gesture| gesture.kind == GestureKind::Move);

    rsx! {
        section {
            class: "panel floating",
            "data-kind": kind,
            aria_label: name,
            style,
            onresize: move |event| {
                // The desktop renderer bubbles resize events, the scene would take our size as
                // its own.
                event.stop_propagation();
                if let Ok(size) = event.get_border_box_size() {
                    rendered_height.set(size.height);
                }
            },
            // The press goes on to its target, the panel's elements don't move in the DOM.
            onpointerdown: move |_| {
                if !in_front {
                    act(InspectorAction::BringToFront);
                }
            },
            onfocusin: move |_| {
                if !in_front {
                    act(InspectorAction::BringToFront);
                }
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    leave(state, return_to, id);
                }
            },

            header { class: "floating-head",
                // The title is the move handle. The buttons beside it aren't inside, so pressing
                // them never starts a move.
                button {
                    id: grip_id.clone(),
                    class: if moving { "floating-grip moving" } else { "floating-grip" },
                    r#type: "button",
                    aria_label: "Move {noun}",
                    "aria-description": "Arrow keys move it, hold Shift to move further.",
                    onpointerdown: move |event| start(event, GestureKind::Move, &grip_id),
                    onpointermove: drag,
                    onpointerup: move |_| end(),
                    onpointercancel: move |_| end(),
                    onlostpointercapture: move |_| end(),
                    onkeydown: move |event| {
                        let Some((dx, dy)) = arrow_step(&event) else {
                            return;
                        };
                        event.prevent_default();
                        panel.set(shown.moved(dx, dy, *rendered_height.peek(), scene));
                    },
                    span { class: "floating-grip-mark", aria_hidden: "true", "⠿" }
                    // Not part of the name, it's styled in capitals and some screen readers
                    // would spell it out. The panel's name says it instead.
                    span { class: "floating-title", aria_hidden: "true", "{title}" }
                }
                div { class: "floating-actions",
                    {actions}
                    button {
                        class: "button",
                        r#type: "button",
                        aria_label: "Close {noun}",
                        onclick: move |_| close(state, return_to, id, InspectorAction::Close),
                        "×"
                    }
                }
            }

            {children}

            button {
                id: handle_id.clone(),
                class: "floating-resize",
                r#type: "button",
                aria_label: "Resize {noun}",
                "aria-description": "Arrow keys resize it, hold Shift to resize further.",
                onpointerdown: move |event| start(event, GestureKind::Resize, &handle_id),
                onpointermove: drag,
                onpointerup: move |_| end(),
                onpointercancel: move |_| end(),
                onlostpointercapture: move |_| end(),
                onkeydown: move |event| {
                    let Some((dx, dy)) = arrow_step(&event) else {
                        return;
                    };
                    event.prevent_default();
                    let height = shown.height.unwrap_or(*rendered_height.peek());
                    panel.set(shown.resized(shown.width + dx, height + dy, scene));
                },
                span { aria_hidden: "true", "◢" }
            }
        }
    }
}

/// How far an arrow key moves or resizes, `None` for other keys.
fn arrow_step(event: &KeyboardEvent) -> Option<(f64, f64)> {
    let step = if event.modifiers().contains(Modifiers::SHIFT) {
        SHIFT_KEY_STEP
    } else {
        KEY_STEP
    };

    match event.key() {
        Key::ArrowLeft => Some((-step, 0.0)),
        Key::ArrowRight => Some((step, 0.0)),
        Key::ArrowUp => Some((0.0, -step)),
        Key::ArrowDown => Some((0.0, step)),
        _ => None,
    }
}

/// Moves focus out of panel `id`: to what opened it, or into the scene when that's gone.
fn leave(state: Signal<AppState>, return_to: Signal<ReturnTo>, id: InspectorId) {
    let opener = return_to.peek().get(&id).cloned();
    let entry = state.peek().loaded.as_ref().and_then(scene::entry_id);

    let targets: Vec<&str> = opener
        .iter()
        .chain(entry.iter())
        .map(String::as_str)
        .chain([OPEN_FILE_ID])
        .collect();
    dom::focus_first(&targets);
}

/// Closes panel `id` with `action`, returning focus like [`leave`] does.
pub fn close(
    mut state: Signal<AppState>,
    mut return_to: Signal<ReturnTo>,
    id: InspectorId,
    action: InspectorAction,
) {
    // Focus moves first, the focused button is about to disappear.
    leave(state, return_to, id);
    return_to.write().remove(&id);
    state.write().update(Action::Inspector(id, action));
}
