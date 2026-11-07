//! Shipyard visualizer: explores the workloads exported by `World::workloads_info`.
//!
//! The same components render in the browser (`web` feature) and in a system WebView
//! (`desktop` feature). Pick exactly one of those features per build.

// Platform file acquisition. Both modules provide an `OpenFile` button, `import_dropped`, and the
// `AutoReload` checkbox, which is empty in the browser, and `use_startup_file` for a file given on
// the command line.
#[cfg(feature = "desktop")]
#[path = "files/desktop.rs"]
mod files;
#[cfg(not(feature = "desktop"))]
#[path = "files/web.rs"]
mod files;

mod dom;
mod floating;
mod import_ui;
mod inspector_ui;
mod map_ui;
mod scene;
mod search_ui;

use dioxus::html::HasFileData;
use dioxus::prelude::*;
use import_ui::{ErrorNotice, ReplaceDialog};
use scene::Scene;
use visualizer::state::{Action, AppState};

const MAIN_CSS: Asset = asset!("/assets/main.css");

/// How users produce the file this application reads.
const EXPORT_SNIPPET: &str = r#"std::fs::write(
    "workloads.json",
    serde_json::to_string(&world.workloads_info()).unwrap(),
)
.unwrap();"#;

const GUIDE_URL: &str =
    "https://leudz.github.io/shipyard/guide/master/going-further/visualizer.html";

#[cfg(feature = "desktop")]
fn main() {
    use dioxus::desktop::{Config, WindowBuilder};

    // Dioxus keeps debug builds, the ones `dx serve` runs, above every other window. We give
    // our own window instead, it's an ordinary one in every build.
    let window = WindowBuilder::new()
        .with_title("Shipyard Visualizer")
        .with_always_on_top(false);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}

#[cfg(not(feature = "desktop"))]
fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut state = use_signal(AppState::default);
    files::use_startup_file(state);
    let app = state.read();

    rsx! {
        document::Title { "Shipyard Visualizer" }
        document::Stylesheet { href: MAIN_CSS }

        div {
            class: "app",
            // Files can be dropped anywhere in the window. We prevent the default so dropping
            // doesn't navigate away from the application.
            ondragover: move |event| event.prevent_default(),
            ondrop: move |event| {
                event.prevent_default();
                // We load one dataset at a time, extra files are ignored.
                if let Some(file) = event.files().into_iter().next() {
                    files::import_dropped(state, file);
                }
            },

            header { class: "app-header",
                h1 { class: "app-title",
                    "Shipyard "
                    span { "Visualizer" }
                }
                // The file name only changes once a replacement is committed.
                if let Some(loaded) = &app.loaded {
                    p {
                        class: "file-name",
                        title: loaded.source.path.as_ref().map(|path| path.display().to_string()),
                        "{loaded.source.name}"
                    }
                    files::AutoReload { state }
                }
                div { class: "header-actions",
                    files::OpenFile { state }
                    button {
                        class: "button",
                        r#type: "button",
                        onclick: move |_| state.write().update(Action::LoadExample),
                        "Load example"
                    }
                }
            }

            main { class: if app.loaded.is_some() { "app-main workspace" } else { "app-main" },
                ErrorNotice { state }

                if app.loaded.is_some() {
                    Scene { state }
                } else {
                    EmptyState {}
                }
            }

            ReplaceDialog { state }
        }
    }
}

/// Shown while no dataset is loaded.
#[component]
fn EmptyState() -> Element {
    rsx! {
        section { class: "panel empty-state", aria_labelledby: "empty-state-title",
            h2 { id: "empty-state-title", "No workloads loaded" }
            p {
                "The visualizer reads a JSON export of "
                code { "shipyard::scheduler::info::WorkloadsInfo" }
                "."
                br {}
                "You can create one from your application:"
            }
            pre { class: "code-block",
                code { "{EXPORT_SNIPPET}" }
            }
            p {
                "See the "
                a { href: GUIDE_URL, target: "_blank", rel: "noopener noreferrer", "visualizer guide" }
                " for details."
            }
        }
    }
}

/// Formats `amount` with `noun`, pluralized with an `s` or `es`.
fn count(amount: usize, noun: &str) -> String {
    if amount == 1 {
        return format!("1 {noun}");
    }

    let suffix = if noun.ends_with('h') { "es" } else { "s" };
    format!("{amount} {noun}{suffix}")
}
