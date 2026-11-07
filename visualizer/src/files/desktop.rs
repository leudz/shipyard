//! Desktop file acquisition: the native file picker, dropped files, file reads, and
//! auto-reload.
//!
//! Both paths know the file's actual location, which we keep in [`Source::path`] to watch it.

use crate::dom;
use crate::import_ui::OPEN_FILE_ID;
use dioxus::core::Task;
use dioxus::html::FileData;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;
use visualizer::import::ImportError;
use visualizer::state::{Action, AppState, Source, WatchTarget};

/// Opens a workload export chosen with the native file picker.
#[component]
pub fn OpenFile(state: Signal<AppState>) -> Element {
    rsx! {
        button {
            id: OPEN_FILE_ID,
            class: "button",
            r#type: "button",
            onclick: move |_| async move {
                // Closing the picker without choosing a file leaves everything as it was.
                if let Some(path) = pick_file().await {
                    import_file(state, path).await;
                }
            },
            "Open file…"
        }
    }
}

/// Imports the file given on the command line, like `visualizer workloads.json`, once at launch.
pub fn use_startup_file(state: Signal<AppState>) {
    use_hook(|| {
        if let Some(path) = std::env::args_os().nth(1) {
            spawn(import_file(state, PathBuf::from(path)));
        }
    });
}

/// Imports a file dropped on the window.
pub fn import_dropped(state: Signal<AppState>, file: FileData) {
    // The desktop renderer builds dropped files from the native drop event, so this is the
    // actual path rather than a display name.
    spawn(import_file(state, file.path()));
}

/// Asks for a file with the native file picker, `None` when the user closes it.
async fn pick_file() -> Option<PathBuf> {
    // We attach the picker to our window so it stays on top of it and blocks it while open.
    let window = dioxus::desktop::window();

    let file = rfd::AsyncFileDialog::new()
        .set_title("Open workload export")
        .add_filter("JSON", &["json"])
        .add_filter("All files", &["*"])
        .set_parent(&window.window)
        .pick_file()
        .await?;

    Some(file.path().to_path_buf())
}

/// Reads `path` and sends its contents through the import pipeline.
async fn import_file(mut state: Signal<AppState>, path: PathBuf) {
    let request = state.write().start_import();

    let read = tokio::fs::read(&path)
        .await
        .map_err(|error| ImportError::Read(error.to_string()));

    let source = Source {
        name: display_name(&path),
        path: Some(path),
    };

    state.write().update(Action::ImportRead {
        request,
        source,
        read,
    });
}

/// The file name of `path`, or the whole path when it doesn't end with one, like a drive root.
fn display_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

/// How long the file must stay quiet before we reload it, so a save written in several steps
/// reloads once.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// The **Auto-reload file** checkbox, and the watcher it turns on.
///
/// It's disabled when the dataset isn't a file, like the example, but keeps the session's choice.
#[component]
pub fn AutoReload(mut state: Signal<AppState>) -> Element {
    let target = use_memo(move || state.read().watch_target());
    let request = use_memo(move || state.read().reload_request().cloned());
    let shown = use_memo(move || {
        let app = state.read();
        let loaded = app.loaded.as_ref();
        let watchable = loaded.is_some_and(|loaded| loaded.source.path.is_some());
        let reloads = loaded.map_or(0, |loaded| loaded.reloads);
        let name = loaded
            .map(|loaded| loaded.source.name.clone())
            .unwrap_or_default();

        (app.session.auto_reload, watchable, reloads, name)
    });

    // One watcher task per target. A new target cancels the previous task, which drops its
    // watcher, so the old file's events stop with it.
    let mut watcher = use_signal(|| None::<Task>);
    use_effect(move || {
        let target = target();
        if let Some(previous) = watcher.write().take() {
            previous.cancel();
        }
        let Some(target) = target else {
            return;
        };

        watcher.set(Some(spawn(watch(state, target))));
    });

    // The state asks for a read, we report its result.
    use_effect(move || {
        let Some(request) = request() else {
            return;
        };

        spawn(async move {
            let read = tokio::fs::read(&request.path)
                .await
                .map_err(|error| ImportError::Read(error.to_string()));
            state.write().update(Action::ReloadRead {
                reload: request.id,
                read,
            });
        });
    });

    // A reload resets the workspace. When that removed the focused element, focus moves to a
    // control that's still there.
    let reloads = use_memo(move || shown().2);
    use_effect(move || {
        if reloads() > 0 {
            dom::focus_if_lost(&["workload", OPEN_FILE_ID]);
        }
    });

    let (enabled, watchable, reloads, name) = shown();
    let status = match reloads {
        0 => String::new(),
        1 => format!("Reloaded {name}"),
        count => format!("Reloaded {name} ({count} times)"),
    };

    rsx! {
        label {
            class: "auto-reload",
            title: if !watchable { "Only files opened from disk can be reloaded" },
            input {
                r#type: "checkbox",
                checked: enabled,
                disabled: !watchable,
                onchange: move |event| state.write().update(Action::SetAutoReload(event.checked())),
            }
            "Auto-reload file"
        }
        // Announced once per reload, the count changes the text.
        span { class: "reload-status", role: "status", "{status}" }
    }
}

/// Watches `target` until the task is cancelled, reporting quiet-after-change moments.
///
/// We watch the file's folder rather than the file: saving by replacing the file, as many
/// editors do, gives it a new identity that a watch on the file itself would lose.
async fn watch(mut state: Signal<AppState>, target: WatchTarget) {
    let folder = match target.path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let Some(file_name) = target.path.file_name().map(ToOwned::to_owned) else {
        return;
    };

    // The watcher calls back on its own thread, we only send to the channel there and handle
    // events in this task, through the state.
    let (sender, mut changes) = tokio::sync::mpsc::unbounded_channel();
    let callback = move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        // Reads, ours included, aren't changes.
        if matches!(event.kind, notify::EventKind::Access(_)) {
            return;
        }
        if event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(&file_name))
        {
            let _ = sender.send(());
        }
    };
    // Without a watcher we can't reload. The checkbox stays on for when the file is reachable.
    let Ok(mut watcher) = notify::recommended_watcher(callback) else {
        return;
    };
    if notify::Watcher::watch(&mut watcher, &folder, notify::RecursiveMode::NonRecursive).is_err() {
        return;
    }

    while changes.recv().await.is_some() {
        // Wait until the file stays quiet, every change restarts the wait.
        loop {
            match tokio::time::timeout(DEBOUNCE, changes.recv()).await {
                Ok(Some(())) => continue,
                Ok(None) => return,
                Err(_) => break,
            }
        }

        state.write().update(Action::FileChanged(target.watch));
    }
}
