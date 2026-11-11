//! The conflict links of the shown workload: one arrow per revealed pair, and cues at the edges of
//! the screen for links whose other end is off screen.
//!
//! The links are drawn over the scene in screen pixels, so arrows keep their size at any zoom.
//! They're outside the panning surface: pressing one never pans or clears the selection.
//! Nothing here moves the camera, the user pans to what a cue points to.

use crate::scene::SceneRect;
use dioxus::prelude::*;
use visualizer::camera::Camera;
use visualizer::conflicts::VisibleLinks;
use visualizer::layout::WorkloadLayout;
use visualizer::links::{self, Side, View};
use visualizer::model::OperationId;
use visualizer::state::AppState;

/// A revealed `(blocker, blocked)` pair.
pub type Pair = (OperationId, OperationId);

/// The link under the pointer and the focused one, the hovered one wins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkFocus {
    pub hovered: Signal<Option<Pair>>,
    pub focused: Signal<Option<Pair>>,
}

impl LinkFocus {
    /// The link to emphasize and explain.
    pub fn active(self) -> Option<Pair> {
        (self.hovered)().or((self.focused)())
    }
}

/// Draws `links` over the workload laid out as `layout`.
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
    // Routes only change with the layout or the revealed links, not while panning.
    let routes = use_memo(move || {
        let layout = layout.read();
        let Some(layout) = layout.as_ref() else {
            return Vec::new();
        };

        links::routes(layout, &links.read().pairs)
    });

    let view = View {
        camera: camera(),
        width: scene_rect().map_or(0.0, |rect| rect.width),
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
    let app = state.peek();
    let Some(loaded) = &app.loaded else {
        return rsx! {};
    };
    let dataset = &loaded.dataset;

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
                                onmouseenter: move |_| enter(focus.hovered, pair),
                                onmouseleave: move |_| clear(focus.hovered, pair),
                                onfocus: move |_| enter(focus.focused, pair),
                                onblur: move |_| clear(focus.focused, pair),
                            }
                        }
                    }
                }
            }
        }
        for cue in cues {
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
                        onmouseenter: move |_| enter(focus.hovered, pair),
                        onmouseleave: move |_| clear(focus.hovered, pair),
                        onfocus: move |_| enter(focus.focused, pair),
                        onblur: move |_| clear(focus.focused, pair),
                        "{arrow_before}{hidden}{arrow_after}"
                    }
                }
            }
        }
    }
}

fn enter(mut link: Signal<Option<Pair>>, pair: Pair) {
    link.set(Some(pair));
}

/// Clears `link`, unless another link replaced `pair` since.
fn clear(mut link: Signal<Option<Pair>>, pair: Pair) {
    if *link.peek() == Some(pair) {
        link.set(None);
    }
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
fn path_data(route: &links::Route, view: View) -> String {
    let mut d = String::new();

    for (index, point) in route.points.iter().enumerate() {
        let point = view.to_screen(*point);
        let command = if index == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{command}{:.1},{:.1} ", point.x, point.y));
    }

    d
}
