//! The conflict links of the shown workload: one arrow per revealed pair, cues at the edges of the
//! screen for links whose other end is off screen, and the explanation of the hovered or focused one.
//!
//! The links are drawn over the scene in screen pixels, so arrows keep their size at any zoom.
//! They're outside the panning surface: pressing one never pans or clears the selection, and
//! scrolling an explanation never pans the scene.
//! Nothing here moves the camera, the user pans to what a cue points to.

use crate::dom;
use crate::floating::{click_point, Openings};
use crate::scene::{tile_id, SceneRect};
use dioxus::prelude::*;
use shipyard::borrow::Mutability;
use visualizer::camera::Camera;
use visualizer::conflicts::VisibleLinks;
use visualizer::explain::{explain, End, Explanation, Mark, Storage};
use visualizer::inspectors::Record;
use visualizer::layout::WorkloadLayout;
use visualizer::links::{self, Cue, Route, Side, View};
use visualizer::model::OperationId;
use visualizer::panels::Point;
use visualizer::state::{Action, AppState};

/// A revealed `(blocker, blocked)` pair.
pub type Pair = (OperationId, OperationId);

const TOOLTIP_ID: &str = "link-tooltip";
/// The id of the explanation's sentences, links and cues describe themselves with them.
const TEXT_ID: &str = "link-tooltip-text";
const TOOLTIP_WIDTH: f64 = 340.0;
/// Space kept between the explanation and the scene's edges, and below or above its link.
const TOOLTIP_MARGIN: f64 = 12.0;
/// How long an explanation stays after the pointer leaves its link, so the pointer can reach it.
const LEAVE_DELAY_MS: u32 = 250;
/// How far the arrow keys scroll a long explanation, in pixels.
const SCROLL_STEP: f64 = 40.0;

/// Which link is emphasized and explained, and how it got there.
///
/// The pointer over a link or its explanation wins over keyboard focus on either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkFocus {
    pub hovered: Signal<Option<Pair>>,
    pub focused: Signal<Option<Pair>>,
    /// The link whose explanation is under the pointer.
    pub tooltip_hovered: Signal<Option<Pair>>,
    /// The link whose explanation has keyboard focus.
    pub tooltip_focused: Signal<Option<Pair>>,
    /// The link Escape dismissed. It stays hidden until another link is hovered or focused.
    pub dismissed: Signal<Option<Pair>>,
    /// Counts pointer entries, so a delayed leave knows whether the pointer came back meanwhile.
    pub entries: Signal<u64>,
}

impl LinkFocus {
    /// The link to emphasize.
    pub fn active(self) -> Option<Pair> {
        (self.hovered)()
            .or((self.tooltip_hovered)())
            .or((self.focused)())
            .or((self.tooltip_focused)())
    }

    /// The link to explain: the active one, unless Escape dismissed it.
    pub fn explained(self) -> Option<Pair> {
        self.active()
            .filter(|&pair| (self.dismissed)() != Some(pair))
    }

    /// Forgets every link no longer in `links`, they could be gone or mean something else now.
    fn retain(self, links: &VisibleLinks) {
        for mut signal in [
            self.hovered,
            self.focused,
            self.tooltip_hovered,
            self.tooltip_focused,
            self.dismissed,
        ] {
            let stale = signal
                .peek()
                .is_some_and(|pair| !links.pairs.contains(&pair));
            if stale {
                signal.set(None);
            }
        }
    }
}

/// Draws `links` over the workload laid out as `layout`, and explains the active one.
#[component]
pub fn LinkLayer(
    state: Signal<AppState>,
    links: Memo<VisibleLinks>,
    layout: Memo<Option<WorkloadLayout>>,
    camera: Memo<Camera>,
    scene_rect: Signal<Option<SceneRect>>,
    /// Where scene y 0 is on screen.
    top: f64,
    focus: LinkFocus,
) -> Element {
    let openings = use_context::<Openings>();
    // Routes only change with the layout or the revealed links, not while panning.
    let routes = use_memo(move || {
        let layout = layout.read();
        let Some(layout) = layout.as_ref() else {
            return Vec::new();
        };

        links::routes(layout, &links.read().pairs)
    });
    // A link can disappear with the selection or the highlight, its explanation goes with it.
    use_effect(move || focus.retain(&links.read()));
    // Escape hides an explanation until its link is left: coming back to it shows it again.
    use_effect(move || {
        let mut dismissed = focus.dismissed;
        if focus.active().is_none() && dismissed.peek().is_some() {
            dismissed.set(None);
        }
    });

    let size = scene_rect();
    let view = View {
        camera: camera(),
        width: size.map_or(0.0, |rect| rect.width),
        top,
    };
    let routes = routes.read();
    let layout = layout.read();
    let Some(layout) = layout.as_ref() else {
        return rsx! {};
    };
    if routes.is_empty() {
        return rsx! {};
    }
    let cues = links::cues(&routes, layout, view);
    let active = focus.active();
    let explained = focus.explained();
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;
    let describedby = |pair: Pair| (explained == Some(pair)).then_some(TEXT_ID);

    rsx! {
        svg { class: "link-layer",
            defs {
                marker {
                    id: "link-arrow",
                    view_box: "0 0 8 8",
                    ref_x: "8",
                    ref_y: "4",
                    marker_width: "8",
                    marker_height: "8",
                    marker_units: "userSpaceOnUse",
                    orient: "auto",
                    path { class: "link-arrow", d: "M0,0 L8,4 L0,8 Z" }
                }
            }
            for route in routes.iter() {
                {
                    let pair = (route.blocker, route.blocked);
                    let d = path_data(route, view);
                    let class = if active == Some(pair) { "link-line active" } else { "link-line" };
                    let name = format!(
                        "{} blocks {}",
                        dataset.operation_label(route.blocker),
                        dataset.operation_label(route.blocked)
                    );

                    rsx! {
                        g { key: "{pair_key(pair)}",
                            path {
                                class,
                                d: d.clone(),
                                marker_end: "url(#link-arrow)",
                                "aria-hidden": "true",
                            }
                            // A wider transparent stroke is easier to point at, and it's what
                            // keyboard focus lands on.
                            path {
                                class: "link-hit",
                                d,
                                tabindex: "0",
                                role: "img",
                                "aria-label": name,
                                "aria-describedby": describedby(pair),
                                onmouseenter: move |_| enter(focus, pair),
                                onmouseleave: move |_| leave(focus, pair),
                                onfocus: move |_| focus_link(focus, focus.focused, pair),
                                onblur: move |_| clear(focus.focused, pair),
                                onkeydown: move |event| on_key(event, state, focus),
                            }
                        }
                    }
                }
            }
        }
        for cue in cues.iter() {
            {
                let pair = (cue.blocker, cue.blocked);
                let hidden = dataset.operation_label(cue.hidden);
                let (class, arrow_before, arrow_after, edge) = match cue.side {
                    Side::Left => ("link-cue left", "◀ ", "", "left"),
                    Side::Right => ("link-cue right", "", " ▶", "right"),
                };
                let role = if cue.hidden == cue.blocker { "blocker" } else { "blocked" };
                let class = if active == Some(pair) { format!("{class} active") } else { class.to_string() };
                let name = format!(
                    "Off screen to the {edge}: {role} {hidden}, {} blocks {}",
                    dataset.operation_label(cue.blocker),
                    dataset.operation_label(cue.blocked)
                );

                rsx! {
                    span {
                        key: "cue-{pair_key(pair)}",
                        class,
                        style: "top: {cue.y}px;",
                        tabindex: "0",
                        role: "img",
                        aria_label: name,
                        aria_describedby: describedby(pair),
                        onmouseenter: move |_| enter(focus, pair),
                        onmouseleave: move |_| leave(focus, pair),
                        onfocus: move |_| focus_link(focus, focus.focused, pair),
                        onblur: move |_| clear(focus.focused, pair),
                        onkeydown: move |event| on_key(event, state, focus),
                        "{arrow_before}{hidden}{arrow_after}"
                    }
                }
            }
        }
        if let (Some(pair), Some(size)) = (explained, size) {
            if let Some(route) = routes.iter().find(|route| (route.blocker, route.blocked) == pair) {
                {
                    let reasons = dataset.conflicts.reasons(pair.0, pair.1).unwrap_or_default();
                    let explanation = explain(dataset, pair.0, pair.1, reasons);
                    let style = tooltip_style(anchor(route, &cues, view), size);

                    rsx! {
                        div {
                            id: TOOLTIP_ID,
                            class: "link-tooltip",
                            role: "tooltip",
                            style,
                            // The pointer can move onto the explanation to read or scroll it.
                            onmouseenter: move |_| enter_tooltip(focus, pair),
                            onmouseleave: move |_| clear(focus.tooltip_hovered, pair),
                            onfocusin: move |_| focus_link(focus, focus.tooltip_focused, pair),
                            onfocusout: move |_| clear(focus.tooltip_focused, pair),
                            onkeydown: move |event| on_key(event, state, focus),
                            // Wheel and drag gestures over it are for it, not the scene.
                            onpointerdown: move |event| event.stop_propagation(),
                            onwheel: move |event| event.stop_propagation(),
                            {explanation_view(state, openings, pair, explanation)}
                        }
                    }
                }
            }
        }
    }
}

/// The ends of the link, then one row per kind of reason, each in a few words. Hovering a part
/// shows its full sentence, and clicking a storage inspects it. Links and cues are described by
/// the sentences instead, only the storage buttons are left for screen readers to reach.
fn explanation_view(
    state: Signal<AppState>,
    openings: Openings,
    pair: Pair,
    explanation: Explanation,
) -> Element {
    let ends = [&explanation.blocker, &explanation.blocked];
    let purposes = ends.into_iter().filter_map(|end| {
        let purpose = end.purpose?;

        Some(format!("{}: {purpose}", end.name))
    });
    let details = std::iter::once(format!("{}.", explanation.summary))
        .chain(purposes)
        .chain(explanation.borrows.iter().map(|row| row.detail.clone()))
        .chain(explanation.ordering.iter().map(|tag| tag.detail.clone()))
        .chain(explanation.thread.iter().map(|tag| tag.detail.clone()));
    let text = details.collect::<Vec<_>>().join(" ");

    rsx! {
        p { id: TEXT_ID, class: "visually-hidden", "{text}" }
        div { class: "link-ends", aria_hidden: "true",
            {end_view(&explanation.blocker)}
            span { class: "link-ends-arrow", "──▶" }
            {end_view(&explanation.blocked)}
        }
        div { class: "link-reasons",
            for row in explanation.borrows {
                span { class: "link-storages",
                    for (index, storage) in row.storages.into_iter().enumerate() {
                        if index > 0 {
                            span { aria_hidden: "true", ", " }
                        }
                        {storage_view(state, openings, pair, storage, &row.detail)}
                    }
                }
                span { class: "link-marks", aria_hidden: "true", title: "{row.detail}",
                    {marks_view(&row.blocker)}
                    span { class: "link-marks-arrow", "▶" }
                    {marks_view(&row.blocked)}
                }
            }
            if !explanation.ordering.is_empty() {
                span { class: "link-label", aria_hidden: "true", "order" }
                span { class: "link-tags", aria_hidden: "true",
                    for tag in explanation.ordering {
                        span { class: "link-tag", title: "{tag.detail}", "{tag.short}" }
                    }
                }
            }
            if let Some(tag) = explanation.thread {
                span { class: "link-label", aria_hidden: "true", "thread" }
                span { class: "link-tags", aria_hidden: "true",
                    span { class: "link-tag", title: "{tag.detail}", "{tag.short}" }
                }
            }
        }
    }
}

/// An end's name with its batch. A system's is in the systems' color, a regroup's says it's one
/// in its own color then lists its storages in the components' color, and what it does on hover.
fn end_view(end: &End) -> Element {
    let kind = if end.is_regroup { "regroup" } else { "system" };

    rsx! {
        span { class: "link-end", "data-kind": kind,
            span { class: "link-end-name", title: end.purpose,
                if end.is_regroup {
                    span { class: "link-end-kind", "Regroup" }
                    " "
                    span { class: "link-end-storages", {end.storages.join(" + ")} }
                } else {
                    "{end.name}"
                }
            }
            span { class: "link-end-batch", "batch {end.batch}" }
        }
    }
}

/// A storage's label, a button inspecting it when it's a component.
fn storage_view(
    state: Signal<AppState>,
    openings: Openings,
    pair: Pair,
    storage: Storage,
    detail: &str,
) -> Element {
    let Some(component) = storage.component else {
        return rsx! {
            span { class: "link-storage", aria_hidden: "true", title: "{detail}", "{storage.label}" }
        };
    };

    rsx! {
        button {
            class: "link-storage",
            r#type: "button",
            title: "{detail}",
            // Inspecting deselects the tile, the links and this explanation go with it. Leaving the
            // inspector returns to the blocked tile.
            onclick: move |event| {
                let pointer = click_point(&event);
                let action = Action::Inspect(Record::Component(component));
                openings.open(state, action, pointer, Some(tile_id(pair.1)));
            },
            "{storage.label}"
        }
    }
}

/// `EX` and `SH` chips, colored like the highlight's.
fn marks_view(marks: &[Mark]) -> Element {
    rsx! {
        for mark in marks.iter().copied() {
            {
                let class = match mark.mutability {
                    Mutability::Exclusive => "link-mark exclusive",
                    Mutability::Shared => "link-mark shared",
                };

                rsx! {
                    span { class,
                        "{mark.label()}"
                        if mark.condition {
                            small { "cond" }
                        }
                    }
                }
            }
        }
    }
}

/// The pointer entered `pair`'s link or cue.
fn enter(mut focus: LinkFocus, pair: Pair) {
    *focus.entries.write() += 1;
    focus.hovered.set(Some(pair));
    forget_dismissal(focus, pair);
}

/// The pointer left `pair`'s link or cue: its explanation stays a moment, so the pointer can reach it.
fn leave(focus: LinkFocus, pair: Pair) {
    let entries = *focus.entries.peek();

    spawn(async move {
        dom::sleep(LEAVE_DELAY_MS).await;

        // The pointer entered a link meanwhile, maybe this one again.
        if *focus.entries.peek() != entries {
            return;
        }
        clear(focus.hovered, pair);
    });
}

fn enter_tooltip(mut focus: LinkFocus, pair: Pair) {
    focus.tooltip_hovered.set(Some(pair));
}

/// Keyboard focus reached `pair`'s link, cue, or explanation.
fn focus_link(focus: LinkFocus, mut signal: Signal<Option<Pair>>, pair: Pair) {
    signal.set(Some(pair));
    forget_dismissal(focus, pair);
}

/// Another link than the dismissed one shows its explanation again.
fn forget_dismissal(mut focus: LinkFocus, pair: Pair) {
    let dismissed = *focus.dismissed.peek();
    if dismissed.is_some_and(|dismissed| dismissed != pair) {
        focus.dismissed.set(None);
    }
}

/// Clears `link`, unless another link replaced `pair` since.
fn clear(mut link: Signal<Option<Pair>>, pair: Pair) {
    if *link.peek() == Some(pair) {
        link.set(None);
    }
}

/// Escape dismisses the explanation and clears the selection, like in the rest of the scene.
/// The arrow and page keys scroll a long explanation.
fn on_key(event: KeyboardEvent, mut state: Signal<AppState>, mut focus: LinkFocus) {
    let step = match event.key() {
        Key::Escape => {
            event.prevent_default();
            focus.dismissed.set(focus.active());
            state.write().update(Action::ClearSelection);
            return;
        }
        Key::ArrowDown => SCROLL_STEP,
        Key::ArrowUp => -SCROLL_STEP,
        Key::PageDown => 4.0 * SCROLL_STEP,
        Key::PageUp => -4.0 * SCROLL_STEP,
        _ => return,
    };

    event.prevent_default();
    dom::scroll_by(TOOLTIP_ID, step);
}

/// Where the explanation of `route` points from: the end of the link that's on screen, or its cue.
fn anchor(route: &Route, cues: &[Cue], view: View) -> Point {
    let cue = cues
        .iter()
        .find(|cue| (cue.blocker, cue.blocked) == (route.blocker, route.blocked));
    if let Some(cue) = cue {
        let x = match cue.side {
            Side::Left => 0.0,
            Side::Right => view.width,
        };
        return Point { x, y: cue.y };
    }

    // Both ends are on screen, or neither: we point at the blocked end, clamped to the screen.
    let end = view.to_screen(*route.points.last().expect("routes have points"));

    Point {
        x: end.x.clamp(0.0, view.width),
        y: end.y,
    }
}

/// Places the explanation next to `anchor`, below it in the scene's top half and above it in the
/// bottom half, as high as the room left there. It scrolls beyond that.
fn tooltip_style(anchor: Point, size: SceneRect) -> String {
    let width = TOOLTIP_WIDTH
        .min(size.width - 2.0 * TOOLTIP_MARGIN)
        .max(0.0);
    let left = (anchor.x + TOOLTIP_MARGIN)
        .min(size.width - width - TOOLTIP_MARGIN)
        .max(TOOLTIP_MARGIN);

    let vertical = if anchor.y < size.height / 2.0 {
        let top = anchor.y + TOOLTIP_MARGIN;
        let room = size.height - top - TOOLTIP_MARGIN;
        format!("top: {top}px; bottom: auto; max-height: {room}px;")
    } else {
        let bottom = size.height - anchor.y + TOOLTIP_MARGIN;
        let room = anchor.y - 2.0 * TOOLTIP_MARGIN;
        format!("top: auto; bottom: {bottom}px; max-height: {room}px;")
    };

    // We write every side each time: Dioxus updates the style declaration by declaration, a side
    // we stop writing would keep its last value.
    format!("left: {left}px; width: {width}px; {vertical}")
}

/// A key and id fragment unique to `pair`.
pub fn pair_key((blocker, blocked): Pair) -> String {
    format!("{}-{}", operation_key(blocker), operation_key(blocked))
}

fn operation_key(operation: OperationId) -> String {
    match operation {
        OperationId::System(occurrence) => format!("s{}", occurrence.0),
        OperationId::Regroup(regroup) => format!("r{}", regroup.0),
    }
}

/// The SVG path of `route` on screen.
fn path_data(route: &Route, view: View) -> String {
    let mut d = String::new();

    for (index, point) in route.points.iter().enumerate() {
        let point = view.to_screen(*point);
        let command = if index == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{command}{:.1},{:.1} ", point.x, point.y));
    }

    d
}
