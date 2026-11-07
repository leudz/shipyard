//! Import feedback: failure notices and the replacement confirmation dialog.

use crate::dom;
use dioxus::prelude::*;
use visualizer::state::{Action, AppState, ImportStatus};

/// The id of the open-file control, both platforms use it.
///
/// It's where focus goes when the control that had it disappears.
pub const OPEN_FILE_ID: &str = "open-file";

const REPLACE_DIALOG_ID: &str = "replace-dialog";

/// Shows why the last import failed, until dismissed or a later import succeeds.
#[component]
pub fn ErrorNotice(state: Signal<AppState>) -> Element {
    let app = state.read();
    let Some(failure) = &app.error else {
        return rsx! {};
    };

    rsx! {
        // An alert is announced as soon as it appears, wherever focus is.
        div { class: "panel notice notice-error", role: "alert",
            // The summary is what most people need, the parser details come after for those
            // who want to fix their file.
            p {
                strong { "{failure.error.summary()}" }
            }
            p { class: "notice-details",
                "{failure.source_name}: "
                code { "{failure.error.details()}" }
            }
            div { class: "actions",
                button {
                    class: "button",
                    r#type: "button",
                    onclick: move |_| {
                        state.write().update(Action::DismissError);
                        // The button disappears, so we keep focus somewhere useful.
                        dom::focus(OPEN_FILE_ID);
                    },
                    "Dismiss"
                }
            }
        }
    }
}

/// Asks before a valid import replaces the loaded dataset.
#[component]
pub fn ReplaceDialog(state: Signal<AppState>) -> Element {
    let candidate_name = use_memo(move || match &state.read().import {
        ImportStatus::Confirming(candidate) => Some(candidate.source.name.clone()),
        _ => None,
    });

    // The dialog element always exists, we open it as a modal while a candidate waits and close
    // it once the user decided, or a newer import superseded the candidate.
    use_effect(move || {
        if candidate_name.read().is_some() {
            dom::show_modal(REPLACE_DIALOG_ID);
        } else {
            dom::close_modal(REPLACE_DIALOG_ID, OPEN_FILE_ID);
        }
    });

    rsx! {
        dialog {
            id: REPLACE_DIALOG_ID,
            class: "panel dialog",
            aria_labelledby: "replace-title",
            aria_describedby: "replace-description",
            // Escape fires `cancel`, the browser then closes the dialog on its own.
            oncancel: move |_| state.write().update(Action::CancelReplacement),

            if let Some(name) = candidate_name() {
                h2 { id: "replace-title", "Replace the loaded workloads?" }
                p { id: "replace-description",
                    "Opening "
                    strong { "{name}" }
                    " closes inspectors and resets the workspace."
                }
                div { class: "actions",
                    // The browser focuses the first control when the dialog opens, so the
                    // harmless choice comes first.
                    button {
                        class: "button",
                        r#type: "button",
                        onclick: move |_| state.write().update(Action::CancelReplacement),
                        "Cancel"
                    }
                    button {
                        class: "button button-primary",
                        r#type: "button",
                        onclick: move |_| state.write().update(Action::ConfirmReplacement),
                        "Replace"
                    }
                }
            }
        }
    }
}
