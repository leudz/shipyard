//! Browser file acquisition: a file input, dropped files, and file reads.
//!
//! Browsers only give us a file's contents and name, never its path, so there's nothing to watch.

use crate::import_ui::OPEN_FILE_ID;
use dioxus::html::FileData;
use dioxus::prelude::*;
use visualizer::import::ImportError;
use visualizer::state::{Action, AppState, Source};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Opens a workload export chosen with the browser's file picker.
#[component]
pub fn OpenFile(state: Signal<AppState>) -> Element {
    rsx! {
        // The input stays focusable and labelled by the visible text, only its own look is hidden.
        label { class: "button file-button",
            input {
                id: OPEN_FILE_ID,
                class: "visually-hidden",
                r#type: "file",
                accept: ".json,application/json",
                // Closing the picker without choosing a file doesn't fire `change`.
                onchange: move |event: FormEvent| {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };

                    clear_input(&event);
                    spawn(import_file(state, file));
                },
            }
            "Open file…"
        }
    }
}

/// Imports a file dropped on the page.
pub fn import_dropped(state: Signal<AppState>, file: FileData) {
    spawn(import_file(state, file));
}

/// Empties the file input so choosing the same file again, after re-exporting it for example,
/// still fires `change`.
///
/// The `File` we already took stays readable.
fn clear_input(event: &FormEvent) {
    let input = event
        .data()
        .downcast::<web_sys::Event>()
        .and_then(|event| event.target())
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok());

    if let Some(input) = input {
        input.set_value("");
    }
}

/// Reads `file` and sends its contents through the import pipeline.
async fn import_file(mut state: Signal<AppState>, file: FileData) {
    let request = state.write().start_import();

    let read = read_file(&file).await;

    let source = Source {
        name: file.name(),
        path: None,
    };

    state.write().update(Action::ImportRead {
        request,
        source,
        read,
    });
}

async fn read_file(file: &FileData) -> Result<Vec<u8>, ImportError> {
    // We read with `Blob.arrayBuffer` rather than `FileData::read_bytes` because the latter never
    // completes when the read fails, a dropped folder for example, while this promise rejects.
    let Some(file) = file.inner().downcast_ref::<web_sys::File>() else {
        return Err(ImportError::Read("not a browser file".to_string()));
    };

    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|error| ImportError::Read(js_error_message(&error)))?;

    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn js_error_message(error: &wasm_bindgen::JsValue) -> String {
    match error.dyn_ref::<js_sys::Error>() {
        Some(error) => error.message().into(),
        None => format!("{error:?}"),
    }
}

/// The browser can't watch files, imports are always explicit there.
#[component]
pub fn AutoReload(state: Signal<AppState>) -> Element {
    let _ = state;

    rsx! {}
}

/// The browser has no command line, files are imported from the page.
pub fn use_startup_file(state: Signal<AppState>) {
    let _ = state;
}
