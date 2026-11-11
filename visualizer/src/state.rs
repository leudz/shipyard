//! Application state: the committed dataset with its workspace, and the import in progress.
//!
//! Views and platform adapters change it only through [`AppState::start_import`] and [`AppState::update`].

use crate::camera::Camera;
use crate::conflicts::VisibleLinks;
use crate::highlight::{link_roots, Relations};
use crate::import::{load_dataset, ImportError};
#[cfg(test)]
use crate::inspectors::OpenPanel;
use crate::inspectors::{InspectorAction, InspectorId, Inspectors, Record};
use crate::model::{ComponentId, Dataset, OperationId, SystemId, WorkloadId};
use crate::search::Search;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// The square-eater export bundled with the application.
const EXAMPLE: &[u8] = include_bytes!("square_eater_workloads.json");
const EXAMPLE_NAME: &str = "Square eater example";

#[derive(Debug, Default)]
pub struct AppState {
    /// Settings that outlive datasets.
    pub session: Session,
    /// The committed dataset, `None` until the first successful import.
    pub loaded: Option<Loaded>,
    pub import: ImportStatus,
    /// The last failed import, until it's dismissed or a later import completes.
    pub error: Option<ImportFailure>,
    /// The latest id [`AppState::start_import`] gave out.
    last_request: u64,
    /// How many datasets were committed, see [`Loaded::generation`].
    commits: u64,
    /// Native only: following the committed file for auto-reload.
    watch: Watch,
}

/// Where auto-reload stands, see [`AppState::watch_target`] and [`AppState::reload_request`].
#[derive(Debug, Default)]
struct Watch {
    /// Changes whenever what we watch does: auto-reload toggled, or another file committed.
    /// Events and reads from an earlier generation are stale.
    generation: u64,
    /// The reload to read, until its read completes.
    reload: Option<ReloadRequest>,
    /// A change arrived while we couldn't reload, during a manual import or another reload.
    /// We reload once that's over.
    waiting: bool,
    /// The latest id a reload got.
    last_reload: u64,
}

/// The file auto-reload follows, for the platform's watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchTarget {
    /// Pass it back with [`Action::FileChanged`].
    pub watch: WatchId,
    pub path: PathBuf,
}

/// Identifies one watch of one file, see [`AppState::watch_target`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchId(u64);

/// A file the platform should read for auto-reload, and report with [`Action::ReloadRead`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadRequest {
    pub id: ReloadId,
    pub path: PathBuf,
}

/// Identifies a reload, so a read completing late can't overwrite newer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReloadId(u64);

/// Settings for the whole application session, imports don't reset them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Session {
    /// Native only: reload the committed file when it changes on disk.
    pub auto_reload: bool,
}

/// A committed dataset and the interaction state that refers into it.
#[derive(Debug)]
pub struct Loaded {
    /// Different for every commit, so views can tell datasets apart even when their ids match.
    pub generation: u64,
    pub source: Source,
    pub dataset: Dataset,
    /// Searches `dataset`, the workspace keeps the queries and results.
    pub search: Search,
    pub workspace: Workspace,
    /// How many times auto-reload committed the file since it was last imported by hand.
    pub reloads: u32,
    /// A hash of the committed bytes, so a reload of the same content changes nothing. We compare
    /// the file's bytes because deserialized exports can't be compared, see [`crate::model`].
    content: u64,
}

/// Interaction state specific to one dataset.
///
/// Every commit replaces it with [`Workspace::new`], so nothing in it can refer to a previous dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    /// `None` only when the dataset has no workloads.
    pub selected_workload: Option<WorkloadId>,
    /// The tile last picked in the scene, a system appearance or a regroup.
    /// It can belong to another workload than the selected one.
    pub selected: Option<OperationId>,
    /// How the selected workload is panned and zoomed.
    pub camera: Camera,
    /// An operation of the selected workload the scene should move the camera to.
    ///
    /// The scene knows where batches are once it measured them, it does the framing and clears
    /// this with [`Action::SetCamera`].
    pub framing: Option<OperationId>,
    pub searches: Searches,
    pub inspectors: Inspectors,
    /// The system or component whose relationships every view marks.
    pub highlight: Option<Record>,
}

impl Workspace {
    pub fn new(dataset: &Dataset) -> Workspace {
        let selected_workload = (!dataset.is_empty()).then_some(WorkloadId(0));

        Workspace {
            selected_workload,
            selected: None,
            camera: Camera::default(),
            framing: None,
            searches: Searches::new(dataset),
            inspectors: Inspectors::default(),
            highlight: None,
        }
    }

    /// Shows `workload` from its start at 100%.
    fn select_workload(&mut self, workload: WorkloadId) {
        self.selected_workload = Some(workload);
        self.camera = Camera::default();
        self.framing = None;
    }
}

/// The query and results of each search field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Searches {
    pub components: SearchField<ComponentId>,
    pub systems: SearchField<SystemId>,
    pub workloads: SearchField<WorkloadId>,
}

impl Searches {
    /// Empty queries, which list every record alphabetically.
    fn new(dataset: &Dataset) -> Searches {
        Searches {
            components: SearchField::all((0..dataset.components.len()).map(ComponentId)),
            systems: SearchField::all((0..dataset.systems.len()).map(SystemId)),
            workloads: SearchField::all((0..dataset.workloads.len()).map(WorkloadId)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchField<Id> {
    pub query: String,
    /// The records matching `query`, best first.
    pub results: Vec<Id>,
}

impl<Id> SearchField<Id> {
    /// An empty query, with all `ids` as results.
    fn all(ids: impl Iterator<Item = Id>) -> SearchField<Id> {
        SearchField {
            query: String::new(),
            results: ids.collect(),
        }
    }
}

/// Which of the search fields an action is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Components,
    Systems,
    Workloads,
}

/// Where an import's bytes come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// What we show the user, a file name for files.
    pub name: String,
    /// Native files only, used to watch the file.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub enum ImportStatus {
    #[default]
    Idle,
    /// Waiting for the read of this request to complete.
    Reading(RequestId),
    /// A valid candidate waits for the user to confirm it replaces the loaded dataset.
    Confirming(Box<Candidate>),
}

/// A parsed and validated import, not committed yet.
#[derive(Debug)]
pub struct Candidate {
    pub source: Source,
    pub dataset: Dataset,
    /// See [`Loaded::content`].
    content: u64,
}

/// Identifies a manual import, so a read completing late can't overwrite a newer import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFailure {
    pub source_name: String,
    pub error: ImportError,
}

#[derive(Debug)]
pub enum Action {
    /// The read of a manual import's file completed.
    ImportRead {
        request: RequestId,
        source: Source,
        read: Result<Vec<u8>, ImportError>,
    },
    /// Imports the bundled example.
    LoadExample,
    ConfirmReplacement,
    CancelReplacement,
    DismissError,
    /// Changes a search field's query, which updates its results.
    Search {
        kind: SearchKind,
        query: String,
    },
    /// Shows another workload, from its start at 100%.
    SelectWorkload(WorkloadId),
    /// Selects a tile of the scene and inspects its system, or the regroup itself.
    SelectOperation(OperationId),
    /// Selects an operation and its workload, and frames its batch.
    ShowOperation(OperationId),
    /// Shows a record in the unpinned inspector, see [`Inspectors::inspect`]. This deselects the
    /// tile, the selection is about the tile last picked.
    Inspect(Record),
    /// Changes an open inspector. Going Back or Forward deselects the tile, like inspecting.
    Inspector(InspectorId, InspectorAction),
    /// Opens the global map, or brings it to the front.
    OpenMap,
    /// Highlights a system or component, or stops highlighting it when it already is.
    ToggleHighlight(Record),
    /// Pans or zooms the workload scene, replacing any framing not done yet.
    SetCamera(Camera),
    /// Native only: turns auto-reload of the committed file on or off.
    SetAutoReload(bool),
    /// Native only: the watcher of [`AppState::watch_target`] saw the file change.
    FileChanged(WatchId),
    /// Native only: the read of [`AppState::reload_request`] completed.
    ReloadRead {
        reload: ReloadId,
        read: Result<Vec<u8>, ImportError>,
    },
}

impl AppState {
    /// Starts a manual import, its read reports back with [`Action::ImportRead`] and the returned id.
    ///
    /// This supersedes any read or confirmation in progress, the latest manual import wins.
    /// Call it once there is a file to read, so closing a file picker changes nothing.
    pub fn start_import(&mut self) -> RequestId {
        self.last_request += 1;
        let request = RequestId(self.last_request);

        self.import = ImportStatus::Reading(request);

        request
    }

    pub fn update(&mut self, action: Action) {
        match action {
            Action::ImportRead {
                request,
                source,
                read,
            } => self.finish_import(request, source, read),
            Action::LoadExample => {
                let request = self.start_import();
                let source = Source {
                    name: EXAMPLE_NAME.to_string(),
                    path: None,
                };

                self.finish_import(request, source, Ok(EXAMPLE.to_vec()));
            }
            Action::ConfirmReplacement => match std::mem::take(&mut self.import) {
                ImportStatus::Confirming(candidate) => {
                    self.commit(*candidate);
                    self.resume_watch();
                }
                other => self.import = other,
            },
            Action::CancelReplacement => {
                if matches!(self.import, ImportStatus::Confirming(_)) {
                    self.import = ImportStatus::Idle;
                    self.resume_watch();
                }
            }
            Action::DismissError => self.error = None,
            Action::Search { kind, query } => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.search(kind, query);
                }
            }
            Action::SelectWorkload(workload) => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.workspace.select_workload(workload);
                }
            }
            Action::SelectOperation(operation) => {
                if let Some(loaded) = &mut self.loaded {
                    // A system's inspector is shared by its appearances, a regroup has its own.
                    let record = match operation {
                        OperationId::System(occurrence) => {
                            Record::System(loaded.dataset.occurrence(occurrence).system)
                        }
                        OperationId::Regroup(regroup) => Record::Regroup(regroup),
                    };
                    loaded.workspace.selected = Some(operation);
                    loaded.workspace.inspectors.inspect(record);
                }
            }
            Action::ShowOperation(operation) => {
                if let Some(loaded) = &mut self.loaded {
                    let (workload, _) = loaded.dataset.location(operation);
                    let workspace = &mut loaded.workspace;

                    // Staying in the workload keeps its zoom, framing only pans.
                    if workspace.selected_workload != Some(workload) {
                        workspace.select_workload(workload);
                    }
                    workspace.selected = Some(operation);
                    workspace.framing = Some(operation);
                }
            }
            Action::Inspect(record) => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.workspace.inspectors.inspect(record);
                    loaded.workspace.selected = None;
                }
            }
            Action::Inspector(inspector, action) => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.workspace.inspectors.update(inspector, action);
                    if matches!(action, InspectorAction::Back | InspectorAction::Forward) {
                        loaded.workspace.selected = None;
                    }
                }
            }
            Action::OpenMap => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.workspace.inspectors.open_map();
                }
            }
            Action::ToggleHighlight(record) => {
                if let Some(loaded) = &mut self.loaded {
                    let highlight = &mut loaded.workspace.highlight;
                    *highlight = (*highlight != Some(record)).then_some(record);
                }
            }
            Action::SetCamera(camera) => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.workspace.camera = camera;
                    loaded.workspace.framing = None;
                }
            }
            Action::SetAutoReload(enabled) => {
                if self.session.auto_reload != enabled {
                    self.session.auto_reload = enabled;
                    self.restart_watch();
                }
            }
            Action::FileChanged(watch) => self.file_changed(watch),
            Action::ReloadRead { reload, read } => self.finish_reload(reload, read),
        }
    }

    /// The file the platform should watch: the committed file while auto-reload is on.
    ///
    /// `None` without a file to watch, like for the bundled example or in the browser. When this
    /// changes, the platform should drop its previous watcher.
    pub fn watch_target(&self) -> Option<WatchTarget> {
        if !self.session.auto_reload {
            return None;
        }

        let path = self.loaded.as_ref()?.source.path.clone()?;

        Some(WatchTarget {
            watch: WatchId(self.watch.generation),
            path,
        })
    }

    /// The file the platform should read for auto-reload, `None` when nothing needs reading.
    pub fn reload_request(&self) -> Option<&ReloadRequest> {
        self.watch.reload.as_ref()
    }

    /// Starts watching anew: earlier events and reads become stale.
    fn restart_watch(&mut self) {
        self.watch.generation += 1;
        self.watch.reload = None;
        self.watch.waiting = false;
    }

    /// Reloads the watched file now, or once a manual import or the current reload is over.
    fn file_changed(&mut self, watch: WatchId) {
        // A watcher of something we don't watch anymore.
        if self.watch_target().map(|target| target.watch) != Some(watch) {
            return;
        }

        // The user's decision comes first, and one read at a time: the next one catches up with
        // every change since.
        if !matches!(self.import, ImportStatus::Idle) || self.watch.reload.is_some() {
            self.watch.waiting = true;
            return;
        }

        self.request_reload();
    }

    fn request_reload(&mut self) {
        let Some(target) = self.watch_target() else {
            return;
        };

        self.watch.last_reload += 1;
        self.watch.waiting = false;
        self.watch.reload = Some(ReloadRequest {
            id: ReloadId(self.watch.last_reload),
            path: target.path,
        });
    }

    /// Reloads for a change that waited, once nothing is in the way anymore.
    fn resume_watch(&mut self) {
        let free = matches!(self.import, ImportStatus::Idle) && self.watch.reload.is_none();
        if self.watch.waiting && free {
            self.request_reload();
        }
    }

    /// Commits the reloaded file without confirmation, unless its content didn't change.
    ///
    /// A failed read or parse shows an error and keeps the dataset, the next change retries.
    fn finish_reload(&mut self, reload: ReloadId, read: Result<Vec<u8>, ImportError>) {
        // Stale: watching restarted, or another reload replaced this one.
        let Some(request) = self.watch.reload.take_if(|request| request.id == reload) else {
            return;
        };

        // A manual import started meanwhile and comes first, we read again after it.
        if !matches!(self.import, ImportStatus::Idle) {
            self.watch.waiting = true;
            return;
        }

        self.apply_reload(request, read);
        self.resume_watch();
    }

    fn apply_reload(&mut self, request: ReloadRequest, read: Result<Vec<u8>, ImportError>) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        let source = loaded.source.clone();
        let failure = |error| ImportFailure {
            source_name: source.name.clone(),
            error,
        };

        let bytes = match read {
            Ok(bytes) => bytes,
            Err(error) => {
                self.error = Some(failure(error));
                return;
            }
        };
        // Saving without changes, or an editor touching the file, keeps the workspace.
        let content = content_hash(&bytes);
        if content == loaded.content {
            return;
        }
        let dataset = match load_dataset(&bytes) {
            Ok(dataset) => dataset,
            Err(error) => {
                self.error = Some(failure(error));
                return;
            }
        };

        debug_assert_eq!(source.path.as_ref(), Some(&request.path));
        let reloads = loaded.reloads + 1;
        self.error = None;
        self.commit(Candidate {
            source,
            dataset,
            content,
        });
        if let Some(loaded) = &mut self.loaded {
            loaded.reloads = reloads;
        }
    }

    fn is_reading(&self, request: RequestId) -> bool {
        matches!(self.import, ImportStatus::Reading(current) if current == request)
    }

    fn finish_import(
        &mut self,
        request: RequestId,
        source: Source,
        read: Result<Vec<u8>, ImportError>,
    ) {
        // A newer import started since this one.
        if !self.is_reading(request) {
            return;
        }

        self.import = ImportStatus::Idle;

        let parsed = read.and_then(|bytes| Ok((content_hash(&bytes), load_dataset(&bytes)?)));
        let (content, dataset) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                self.error = Some(ImportFailure {
                    source_name: source.name,
                    error,
                });
                self.resume_watch();

                return;
            }
        };

        self.error = None;
        let candidate = Candidate {
            source,
            dataset,
            content,
        };

        // Replacing loaded data needs the user's confirmation, a first import doesn't.
        if self.loaded.is_some() {
            self.import = ImportStatus::Confirming(Box::new(candidate));
            return;
        }

        self.commit(candidate);
    }

    /// Swaps in the candidate with a fresh workspace, in one step.
    ///
    /// Committing another file than the watched one restarts watching, for the new file.
    fn commit(&mut self, candidate: Candidate) {
        let search = Search::new(&candidate.dataset);
        let workspace = Workspace::new(&candidate.dataset);
        let previous_path = self
            .loaded
            .as_ref()
            .map(|loaded| loaded.source.path.clone());

        if previous_path != Some(candidate.source.path.clone()) {
            self.restart_watch();
        }

        self.commits += 1;
        self.loaded = Some(Loaded {
            generation: self.commits,
            source: candidate.source,
            dataset: candidate.dataset,
            search,
            workspace,
            reloads: 0,
            content: candidate.content,
        });
    }
}

/// Identifies file content. A collision would only skip one reload.
fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);

    hasher.finish()
}

impl Loaded {
    /// The conflict links of the selected workload to show: the selected operation's blockers,
    /// and those of every operation the highlight marks.
    pub fn visible_links(&self) -> VisibleLinks {
        let Some(workload) = self.workspace.selected_workload else {
            return VisibleLinks::default();
        };
        let relations = self
            .workspace
            .highlight
            .map(|target| Relations::new(&self.dataset, target));

        let roots = link_roots(
            &self.dataset,
            workload,
            self.workspace.selected,
            relations.as_ref(),
        );

        self.dataset.conflicts.visible(roots)
    }

    fn search(&mut self, kind: SearchKind, query: String) {
        let searches = &mut self.workspace.searches;

        match kind {
            SearchKind::Components => {
                searches.components.results = self.search.components(&query);
                searches.components.query = query;
            }
            SearchKind::Systems => {
                searches.systems.results = self.search.systems(&query);
                searches.systems.query = query;
            }
            SearchKind::Workloads => {
                searches.workloads.results = self.search.workloads(&query);
                searches.workloads.query = query;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OccurrenceId;

    const TWO_WORKLOADS: &[u8] = br#"{
        "a": {"name": "a", "batches_info": [{"systems": [null, [{
            "name": "System(run)", "borrow": [], "conflict": null,
            "after": [], "after_all": [], "before_all": [], "unique_id": 0,
            "occurrence": {"id": 0, "conditions": [], "footprint": [], "thread_bound": false, "after": []}
        }]], "regroups": []}]},
        "b": {"name": "b", "batches_info": []}
    }"#;
    /// One system, in two batches.
    const REPEATED: &[u8] = br#"{"a": {"name": "a", "batches_info": [
        {"systems": [null, [{
            "name": "System(run)", "borrow": [], "conflict": null,
            "after": [], "after_all": [], "before_all": [], "unique_id": 0,
            "occurrence": {"id": 0, "conditions": [], "footprint": [], "thread_bound": false, "after": []}
        }]], "regroups": []},
        {"systems": [null, [{
            "name": "System(run)", "borrow": [], "conflict": null,
            "after": [], "after_all": [], "before_all": [], "unique_id": 1,
            "occurrence": {"id": 1, "conditions": [], "footprint": [], "thread_bound": false, "after": []}
        }]], "regroups": []}
    ]}}"#;
    const ONE_WORKLOAD: &[u8] = br#"{"c": {"name": "c", "batches_info": []}}"#;
    /// Workload `a`: an entry regroup of A, a system reading A, then a pending regroup of A.
    /// Workload `b`: a system reading A.
    const REGROUPS: &[u8] = br#"{
        "a": {"name": "a", "batches_info": [
            {"systems": [null, []], "regroups": [{"borrow": [
                {"name": "A", "mutability": "Exclusive", "thread_safe": true}
            ], "trigger": "Entry", "occurrence": {"id": 0, "thread_bound": false, "after": []}}]},
            {"systems": [null, [{
                "name": "System(read)", "borrow": [
                    {"name": "A", "mutability": "Shared", "thread_safe": true}
                ], "conflict": null,
                "after": [], "after_all": [], "before_all": [], "unique_id": 0,
                "occurrence": {"id": 0, "conditions": [], "footprint": [
                    {"name": "A", "mutability": "Shared", "thread_safe": true}
                ], "thread_bound": false, "after": []}
            }]], "regroups": []},
            {"systems": [null, []], "regroups": [{"borrow": [
                {"name": "A", "mutability": "Exclusive", "thread_safe": true}
            ], "trigger": "Pending", "occurrence": {"id": 1, "thread_bound": false, "after": []}}]}
        ]},
        "b": {"name": "b", "batches_info": [{"systems": [null, [{
            "name": "System(read)", "borrow": [
                {"name": "A", "mutability": "Shared", "thread_safe": true}
            ], "conflict": null,
            "after": [], "after_all": [], "before_all": [], "unique_id": 0,
            "occurrence": {"id": 0, "conditions": [], "footprint": [
                {"name": "A", "mutability": "Shared", "thread_safe": true}
            ], "thread_bound": false, "after": []}
        }]], "regroups": []}]}
    }"#;
    /// Valid JSON whose occurrence information points to a system that doesn't exist.
    const BROKEN_OCCURRENCE: &[u8] = br#"{"c": {"name": "c", "batches_info": [{"systems": [null, [{
        "name": "System(run)", "borrow": [], "conflict": null,
        "after": [], "after_all": [], "before_all": [], "unique_id": 0,
        "occurrence": {"id": 0, "conditions": [], "footprint": [], "thread_bound": false,
            "after": [{"occurrence": {"System": 9}, "origins": ["Implicit"]}]}
    }]], "regroups": []}]}}"#;

    fn source(name: &str) -> Source {
        Source {
            name: name.to_string(),
            path: Some(PathBuf::from(name)),
        }
    }

    fn read(request: RequestId, name: &str, bytes: &[u8]) -> Action {
        Action::ImportRead {
            request,
            source: source(name),
            read: Ok(bytes.to_vec()),
        }
    }

    fn import(state: &mut AppState, name: &str, bytes: &[u8]) {
        let request = state.start_import();
        state.update(read(request, name, bytes));
    }

    fn loaded(state: &AppState) -> &Loaded {
        state.loaded.as_ref().unwrap()
    }

    /// A state with `a.json` loaded, a non-default workspace and auto-reload on.
    fn seeded() -> AppState {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::SelectWorkload(WorkloadId(1)));
        state.update(search(SearchKind::Workloads, "b"));
        state.update(Action::SelectOperation(OperationId::System(OccurrenceId(
            0,
        ))));
        // Pin the inspector selecting the tile opened.
        let inspector = loaded(&state).workspace.inspectors.open()[0].id();
        state.update(Action::Inspector(inspector, InspectorAction::TogglePin));
        state.update(Action::ToggleHighlight(Record::System(SystemId(0))));
        state.update(Action::SetCamera(Camera {
            x: -300.0,
            zoom: 0.5,
        }));
        state.update(Action::SetAutoReload(true));

        state
    }

    /// The workspace of [`seeded`].
    fn seeded_workspace() -> Workspace {
        seeded().loaded.unwrap().workspace
    }

    fn search(kind: SearchKind, query: &str) -> Action {
        Action::Search {
            kind,
            query: query.to_string(),
        }
    }

    /// Asserts that the first import commits without confirmation and selects the first workload.
    #[test]
    fn first_import() {
        let mut state = AppState::default();

        import(&mut state, "a.json", TWO_WORKLOADS);

        assert!(matches!(state.import, ImportStatus::Idle));
        assert_eq!(loaded(&state).source, source("a.json"));
        assert_eq!(
            loaded(&state).workspace.selected_workload,
            Some(WorkloadId(0))
        );
    }

    /// Asserts that the bundled example imports and has no path to watch.
    #[test]
    fn example() {
        let mut state = AppState::default();

        state.update(Action::LoadExample);

        assert_eq!(loaded(&state).source.name, EXAMPLE_NAME);
        assert_eq!(loaded(&state).source.path, None);
        assert_eq!(loaded(&state).dataset.workloads.len(), 1);
    }

    /// Asserts that an export without workloads commits with no selected workload.
    #[test]
    fn empty_dataset() {
        let mut state = AppState::default();

        import(&mut state, "empty.json", b"{}");

        assert!(loaded(&state).dataset.is_empty());
        assert_eq!(loaded(&state).workspace.selected_workload, None);
    }

    /// Asserts that a failed first import shows an error and loads nothing.
    #[test]
    fn failed_first_import() {
        let mut state = AppState::default();

        import(&mut state, "bad.json", b"{");

        assert!(state.loaded.is_none());
        assert_eq!(state.error.as_ref().unwrap().source_name, "bad.json");
    }

    /// Asserts that a valid replacement waits for confirmation and leaves the loaded dataset untouched.
    #[test]
    fn replacement_waits() {
        let mut state = seeded();

        import(&mut state, "b.json", ONE_WORKLOAD);

        assert!(matches!(state.import, ImportStatus::Confirming(_)));
        assert_eq!(loaded(&state).source, source("a.json"));
        assert_eq!(loaded(&state).workspace, seeded_workspace());
    }

    /// Asserts that cancelling a replacement keeps the dataset and its workspace.
    #[test]
    fn cancel_replacement() {
        let mut state = seeded();
        import(&mut state, "b.json", ONE_WORKLOAD);

        state.update(Action::CancelReplacement);

        assert!(matches!(state.import, ImportStatus::Idle));
        assert_eq!(loaded(&state).source, source("a.json"));
        assert_eq!(loaded(&state).workspace, seeded_workspace());
    }

    /// Asserts that confirming a replacement commits it with a fresh workspace, keeping session settings.
    #[test]
    fn confirm_replacement() {
        let mut state = seeded();
        import(&mut state, "b.json", ONE_WORKLOAD);

        state.update(Action::ConfirmReplacement);

        let loaded = loaded(&state);
        assert!(matches!(state.import, ImportStatus::Idle));
        assert_eq!(loaded.source, source("b.json"));
        assert_eq!(loaded.dataset.workloads[0].name, "c");
        assert_eq!(loaded.workspace, Workspace::new(&loaded.dataset));
        assert!(state.session.auto_reload);
    }

    /// Asserts that a failed replacement shows an error without asking for confirmation or touching the workspace.
    #[test]
    fn failed_replacement() {
        let mut state = seeded();

        import(&mut state, "bad.json", br#"{"version": 1}"#);

        assert!(matches!(state.import, ImportStatus::Idle));
        assert!(matches!(
            state.error.as_ref().unwrap().error,
            ImportError::UnsupportedFormat(_)
        ));
        assert_eq!(loaded(&state).source, source("a.json"));
        assert_eq!(loaded(&state).workspace, seeded_workspace());
    }

    /// Asserts that a replacement with broken occurrence information is rejected before confirmation,
    /// keeping the dataset and workspace.
    #[test]
    fn broken_occurrence_replacement() {
        let mut state = seeded();

        import(&mut state, "broken.json", BROKEN_OCCURRENCE);

        assert!(matches!(state.import, ImportStatus::Idle));
        assert!(matches!(
            state.error.as_ref().unwrap().error,
            ImportError::UnsupportedFormat(_)
        ));
        assert_eq!(loaded(&state).source, source("a.json"));
        assert_eq!(loaded(&state).workspace, seeded_workspace());
    }

    /// A state with [`REGROUPS`] loaded, a camera away from the default, workload `a` shown.
    fn regrouped() -> AppState {
        let mut state = AppState::default();
        import(&mut state, "r.json", REGROUPS);
        state.update(Action::SetCamera(Camera {
            x: -120.0,
            zoom: 0.75,
        }));

        state
    }

    fn regroup_op(index: usize) -> OperationId {
        OperationId::Regroup(crate::model::RegroupId(index))
    }

    fn system_op(index: usize) -> OperationId {
        OperationId::System(OccurrenceId(index))
    }

    /// The `(blocker, blocked)` pairs the loaded workspace shows.
    fn links(state: &AppState) -> Vec<(OperationId, OperationId)> {
        loaded(state).visible_links().pairs.into_iter().collect()
    }

    /// Asserts that selecting a regroup inspects that regroup, and that two regroups of the same
    /// storages are two entries of the inspector's history.
    #[test]
    fn select_regroups() {
        let mut state = regrouped();

        state.update(Action::SelectOperation(regroup_op(0)));
        state.update(Action::SelectOperation(regroup_op(1)));

        let workspace = &loaded(&state).workspace;
        assert_eq!(workspace.selected, Some(regroup_op(1)));
        let inspector = workspace.inspectors.open()[0].inspector().unwrap();
        assert_eq!(
            inspector.record(),
            Record::Regroup(crate::model::RegroupId(1))
        );
        assert_eq!(inspector.position(), (2, 2));
    }

    /// Asserts that selecting, highlighting and clearing the highlight never move the camera.
    #[test]
    fn links_keep_camera() {
        let mut state = regrouped();
        let camera = loaded(&state).workspace.camera;

        state.update(Action::SelectOperation(system_op(0)));
        state.update(Action::ToggleHighlight(Record::Regroup(
            crate::model::RegroupId(1),
        )));
        state.update(Action::ToggleHighlight(Record::Regroup(
            crate::model::RegroupId(1),
        )));

        assert_eq!(loaded(&state).workspace.camera, camera);
        assert_eq!(loaded(&state).workspace.framing, None);
    }

    /// Asserts that a selected system shows its blockers.
    #[test]
    fn selection_links() {
        let mut state = regrouped();

        state.update(Action::SelectOperation(system_op(0)));

        assert_eq!(links(&state), [(regroup_op(0), system_op(0))]);
    }

    /// Asserts that the selection and the highlight add up, and that toggling the highlight off
    /// keeps the selection's links.
    #[test]
    fn selection_and_highlight() {
        let mut state = regrouped();
        state.update(Action::SelectOperation(system_op(0)));

        state.update(Action::ToggleHighlight(Record::Regroup(
            crate::model::RegroupId(1),
        )));
        assert_eq!(
            links(&state),
            [(system_op(0), regroup_op(1)), (regroup_op(0), system_op(0))]
        );

        state.update(Action::ToggleHighlight(Record::Regroup(
            crate::model::RegroupId(1),
        )));
        assert_eq!(links(&state), [(regroup_op(0), system_op(0))]);
    }

    /// Asserts that a highlighted component brings the blockers of its system and regroup borrowers.
    #[test]
    fn component_links() {
        let mut state = regrouped();
        let a = ComponentId(0);

        state.update(Action::ToggleHighlight(Record::Component(a)));

        assert_eq!(
            links(&state),
            [(system_op(0), regroup_op(1)), (regroup_op(0), system_op(0))]
        );
    }

    /// Asserts that a selection kept from another workload has no link in the shown one.
    #[test]
    fn other_workload_selection() {
        let mut state = regrouped();
        state.update(Action::SelectOperation(system_op(0)));

        state.update(Action::SelectWorkload(WorkloadId(1)));

        assert_eq!(loaded(&state).workspace.selected, Some(system_op(0)));
        assert!(links(&state).is_empty());
    }

    /// Asserts that a highlighted system only brings the links of its appearances in the shown workload.
    #[test]
    fn system_links_per_workload() {
        let mut state = regrouped();
        let system = loaded(&state).dataset.occurrence(OccurrenceId(0)).system;

        state.update(Action::ToggleHighlight(Record::System(system)));
        assert_eq!(links(&state), [(regroup_op(0), system_op(0))]);

        state.update(Action::SelectWorkload(WorkloadId(1)));
        assert!(links(&state).is_empty());
    }

    /// Asserts that a read error is reported like any other failure.
    #[test]
    fn read_error() {
        let mut state = seeded();
        let request = state.start_import();

        state.update(Action::ImportRead {
            request,
            source: source("gone.json"),
            read: Err(ImportError::Read("not found".to_string())),
        });

        assert_eq!(
            state.error.as_ref().unwrap().error,
            ImportError::Read("not found".to_string())
        );
        assert_eq!(loaded(&state).source, source("a.json"));
    }

    /// Asserts that a successful import clears the previous error.
    #[test]
    fn error_cleared() {
        let mut state = AppState::default();
        import(&mut state, "bad.json", b"{");

        import(&mut state, "a.json", TWO_WORKLOADS);

        assert_eq!(state.error, None);
    }

    /// Asserts that dismissing an error keeps the loaded dataset.
    #[test]
    fn dismiss_error() {
        let mut state = seeded();
        import(&mut state, "bad.json", b"{");

        state.update(Action::DismissError);

        assert_eq!(state.error, None);
        assert_eq!(loaded(&state).source, source("a.json"));
    }

    /// Asserts that an older read completing before a newer one is ignored.
    #[test]
    fn older_read_first() {
        let mut state = AppState::default();
        let older = state.start_import();
        let newer = state.start_import();

        state.update(read(older, "old.json", TWO_WORKLOADS));
        assert!(state.loaded.is_none());

        state.update(read(newer, "new.json", ONE_WORKLOAD));
        assert_eq!(loaded(&state).source, source("new.json"));
    }

    /// Asserts that an older read completing after a newer one committed changes nothing.
    #[test]
    fn older_read_last() {
        let mut state = AppState::default();
        let older = state.start_import();
        let newer = state.start_import();
        state.update(read(newer, "new.json", ONE_WORKLOAD));

        state.update(read(older, "old.json", TWO_WORKLOADS));

        assert!(matches!(state.import, ImportStatus::Idle));
        assert_eq!(loaded(&state).source, source("new.json"));
    }

    /// Asserts that an older failed read doesn't show its error.
    #[test]
    fn older_failure() {
        let mut state = AppState::default();
        let older = state.start_import();
        let _newer = state.start_import();

        state.update(read(older, "bad.json", b"{"));

        assert_eq!(state.error, None);
    }

    /// Asserts that starting another import while confirming discards the pending candidate.
    #[test]
    fn import_while_confirming() {
        let mut state = seeded();
        import(&mut state, "b.json", ONE_WORKLOAD);

        let _request = state.start_import();
        state.update(Action::ConfirmReplacement);

        assert!(matches!(state.import, ImportStatus::Reading(_)));
        assert_eq!(loaded(&state).source, source("a.json"));
    }

    /// Asserts that confirming or cancelling without a pending candidate leaves a read in progress alone.
    #[test]
    fn confirm_without_candidate() {
        let mut state = seeded();
        let request = state.start_import();

        state.update(Action::ConfirmReplacement);
        state.update(Action::CancelReplacement);

        assert!(state.is_reading(request));
    }

    /// Asserts that every seeded workspace field differs from a fresh workspace, so the reset
    /// tests above check each of them.
    #[test]
    fn seeded_differs() {
        let state = seeded();
        let loaded = loaded(&state);
        let fresh = Workspace::new(&loaded.dataset);

        assert_ne!(loaded.workspace.selected_workload, fresh.selected_workload);
        assert_ne!(loaded.workspace.selected, fresh.selected);
        assert_ne!(loaded.workspace.camera, fresh.camera);
        assert_ne!(loaded.workspace.searches, fresh.searches);
        assert_ne!(loaded.workspace.inspectors, fresh.inspectors);
        assert_ne!(loaded.workspace.highlight, fresh.highlight);
    }

    /// Asserts that every commit gets a new generation, and a cancelled replacement doesn't.
    #[test]
    fn generations() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        let first = loaded(&state).generation;

        import(&mut state, "b.json", ONE_WORKLOAD);
        state.update(Action::CancelReplacement);
        let cancelled = loaded(&state).generation;
        import(&mut state, "b.json", ONE_WORKLOAD);
        state.update(Action::ConfirmReplacement);

        assert_eq!(cancelled, first);
        assert_ne!(loaded(&state).generation, first);
    }

    /// Asserts that a query replaces its field's results, leaving the other fields alone.
    #[test]
    fn search_query() {
        let mut state = AppState::default();
        state.update(Action::LoadExample);

        state.update(search(SearchKind::Systems, "mvpl"));

        let loaded = loaded(&state);
        let systems = &loaded.workspace.searches.systems;
        assert_eq!(systems.query, "mvpl");
        assert_eq!(systems.results.len(), 1);
        assert_eq!(
            loaded.dataset.system(systems.results[0]).label,
            "move_player"
        );
        assert_eq!(
            loaded.workspace.searches.components,
            Workspace::new(&loaded.dataset).searches.components
        );
    }

    /// Asserts that a fresh workspace lists every record in each search field.
    #[test]
    fn search_all() {
        let mut state = AppState::default();

        state.update(Action::LoadExample);

        let loaded = loaded(&state);
        let searches = &loaded.workspace.searches;
        assert_eq!(searches.systems.results.len(), loaded.dataset.systems.len());
        assert_eq!(
            searches.components.results.len(),
            loaded.dataset.components.len()
        );
        assert_eq!(searches.workloads.results, [WorkloadId(0)]);
    }

    /// Asserts that searching before any dataset is loaded does nothing.
    #[test]
    fn search_unloaded() {
        let mut state = AppState::default();

        state.update(search(SearchKind::Systems, "run"));

        assert!(state.loaded.is_none());
    }

    /// Asserts that selecting a workload changes the selected workload and resets the camera.
    #[test]
    fn select_workload() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::SetCamera(Camera {
            x: -300.0,
            zoom: 0.5,
        }));

        state.update(Action::SelectWorkload(WorkloadId(1)));

        let workspace = &loaded(&state).workspace;
        assert_eq!(workspace.selected_workload, Some(WorkloadId(1)));
        assert_eq!(workspace.camera, Camera::default());
    }

    /// The record each open inspector shows, back to front.
    fn records(workspace: &Workspace) -> Vec<Record> {
        let inspectors = workspace.inspectors.open();
        inspectors
            .iter()
            .filter_map(OpenPanel::inspector)
            .map(|inspector| inspector.record())
            .collect()
    }

    /// Asserts that inspecting reaches the workspace's inspectors.
    #[test]
    fn inspect() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);

        state.update(Action::Inspect(Record::System(SystemId(0))));

        assert_eq!(
            records(&loaded(&state).workspace),
            [Record::System(SystemId(0))]
        );
    }

    /// Asserts that inspector actions reach the workspace's inspectors.
    #[test]
    fn inspector_action() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::Inspect(Record::System(SystemId(0))));
        let inspector = loaded(&state).workspace.inspectors.open()[0].id();

        state.update(Action::Inspector(inspector, InspectorAction::TogglePin));

        let inspector = loaded(&state).workspace.inspectors.open()[0].inspector();
        assert!(inspector.unwrap().pinned);
    }
    /// Asserts that selecting a tile selects its occurrence and inspects its system.
    #[test]
    fn select_occurrence() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);

        state.update(Action::SelectOperation(OperationId::System(OccurrenceId(
            0,
        ))));

        let workspace = &loaded(&state).workspace;
        assert_eq!(
            workspace.selected,
            Some(OperationId::System(OccurrenceId(0)))
        );
        assert_eq!(records(workspace), [Record::System(SystemId(0))]);
    }

    /// Asserts that selecting two occurrences of one system selects the latest and opens one
    /// inspector.
    #[test]
    fn select_repeated_occurrence() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        let dataset = &loaded(&state).dataset;
        let [first, second] = dataset.systems[0].occurrences[..] else {
            panic!("expected two occurrences");
        };

        state.update(Action::SelectOperation(OperationId::System(first)));
        state.update(Action::SelectOperation(OperationId::System(second)));

        let workspace = &loaded(&state).workspace;
        assert_eq!(workspace.selected, Some(OperationId::System(second)));
        assert_eq!(records(workspace), [Record::System(SystemId(0))]);
    }
    /// Asserts that showing an occurrence of another workload selects that workload from its start
    /// and asks to frame the occurrence, without touching the inspectors.
    #[test]
    fn show_occurrence() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::SelectWorkload(WorkloadId(1)));
        state.update(Action::SetCamera(Camera {
            x: -300.0,
            zoom: 0.5,
        }));

        state.update(Action::ShowOperation(OperationId::System(OccurrenceId(0))));

        let workspace = &loaded(&state).workspace;
        assert_eq!(workspace.selected_workload, Some(WorkloadId(0)));
        assert_eq!(
            workspace.selected,
            Some(OperationId::System(OccurrenceId(0)))
        );
        assert_eq!(workspace.camera, Camera::default());
        assert_eq!(
            workspace.framing,
            Some(OperationId::System(OccurrenceId(0)))
        );
        assert!(records(workspace).is_empty());
    }

    /// Asserts that showing an occurrence of the selected workload keeps its zoom.
    #[test]
    fn show_occurrence_same_workload() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        let camera = Camera {
            x: -300.0,
            zoom: 0.5,
        };
        state.update(Action::SetCamera(camera));

        state.update(Action::ShowOperation(OperationId::System(OccurrenceId(1))));

        let workspace = &loaded(&state).workspace;
        assert_eq!(workspace.camera, camera);
        assert_eq!(
            workspace.framing,
            Some(OperationId::System(OccurrenceId(1)))
        );
    }

    /// Asserts that setting the camera, which the scene does once it framed, ends the framing.
    #[test]
    fn framed() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        state.update(Action::ShowOperation(OperationId::System(OccurrenceId(1))));

        state.update(Action::SetCamera(Camera::default()));

        assert_eq!(loaded(&state).workspace.framing, None);
    }

    /// Asserts that selecting another workload drops a framing not done yet.
    #[test]
    fn select_workload_drops_framing() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::ShowOperation(OperationId::System(OccurrenceId(0))));

        state.update(Action::SelectWorkload(WorkloadId(1)));

        assert_eq!(loaded(&state).workspace.framing, None);
    }

    /// Asserts that replacing the dataset drops a framing not done yet.
    #[test]
    fn replace_drops_framing() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::ShowOperation(OperationId::System(OccurrenceId(0))));

        import(&mut state, "b.json", ONE_WORKLOAD);
        state.update(Action::ConfirmReplacement);

        assert_eq!(loaded(&state).workspace.framing, None);
    }
    /// Asserts that inspecting a record, from a chip or a search result, deselects the tile.
    #[test]
    fn inspect_deselects() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        state.update(Action::SelectOperation(OperationId::System(OccurrenceId(
            0,
        ))));

        state.update(Action::Inspect(Record::System(SystemId(0))));

        assert_eq!(loaded(&state).workspace.selected, None);
    }

    /// Asserts that going Back in an inspector deselects the tile, and bringing it to the front
    /// doesn't.
    #[test]
    fn history_deselects() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        state.update(Action::SelectOperation(OperationId::System(OccurrenceId(
            0,
        ))));
        let inspector = loaded(&state).workspace.inspectors.open()[0].id();

        state.update(Action::Inspector(inspector, InspectorAction::BringToFront));
        assert_eq!(
            loaded(&state).workspace.selected,
            Some(OperationId::System(OccurrenceId(0)))
        );

        state.update(Action::Inspector(inspector, InspectorAction::Back));
        assert_eq!(loaded(&state).workspace.selected, None);
    }
    /// Asserts that highlighting a record twice stops highlighting it.
    #[test]
    fn highlight_toggles() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        let run = Record::System(SystemId(0));

        state.update(Action::ToggleHighlight(run));
        assert_eq!(loaded(&state).workspace.highlight, Some(run));

        state.update(Action::ToggleHighlight(run));
        assert_eq!(loaded(&state).workspace.highlight, None);
    }

    /// Asserts that highlighting another record replaces the highlight, there's only one.
    #[test]
    fn highlight_replaces() {
        let mut state = AppState::default();
        state.update(Action::LoadExample);
        let run = Record::System(SystemId(0));
        let storage = Record::Component(ComponentId(0));
        state.update(Action::ToggleHighlight(run));

        state.update(Action::ToggleHighlight(storage));

        assert_eq!(loaded(&state).workspace.highlight, Some(storage));
    }

    /// Asserts that highlighting is separate from inspecting and selecting.
    #[test]
    fn highlight_separate() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        state.update(Action::SelectOperation(OperationId::System(OccurrenceId(
            0,
        ))));

        state.update(Action::ToggleHighlight(Record::System(SystemId(0))));

        let workspace = &loaded(&state).workspace;
        assert_eq!(
            workspace.selected,
            Some(OperationId::System(OccurrenceId(0)))
        );
        assert_eq!(records(workspace), [Record::System(SystemId(0))]);
    }

    /// Asserts that opening the map keeps the selected tile, unlike inspecting.
    #[test]
    fn open_map() {
        let mut state = AppState::default();
        import(&mut state, "r.json", REPEATED);
        state.update(Action::SelectOperation(OperationId::System(OccurrenceId(
            0,
        ))));

        state.update(Action::OpenMap);

        let workspace = &loaded(&state).workspace;
        assert_eq!(workspace.inspectors.open().len(), 2);
        assert_eq!(
            workspace.selected,
            Some(OperationId::System(OccurrenceId(0)))
        );
    }
    /// `a.json` loaded with auto-reload on, and a workspace that isn't fresh.
    fn watching() -> AppState {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        state.update(Action::SetAutoReload(true));
        state.update(Action::SelectWorkload(WorkloadId(1)));

        state
    }

    fn target(state: &AppState) -> WatchTarget {
        state.watch_target().unwrap()
    }

    /// The watcher reports a change of the watched file.
    fn change(state: &mut AppState) {
        let watch = target(state).watch;
        state.update(Action::FileChanged(watch));
    }

    /// The pending reload's read completes with `read`.
    fn reload_with(state: &mut AppState, read: Result<Vec<u8>, ImportError>) {
        let reload = state.reload_request().unwrap().id;
        state.update(Action::ReloadRead { reload, read });
    }

    fn reload(state: &mut AppState, bytes: &[u8]) {
        reload_with(state, Ok(bytes.to_vec()));
    }

    /// Asserts that only a committed file with auto-reload on is watched.
    #[test]
    fn watch_target() {
        let mut state = AppState::default();
        import(&mut state, "a.json", TWO_WORKLOADS);
        assert_eq!(state.watch_target(), None);

        state.update(Action::SetAutoReload(true));

        assert_eq!(target(&state).path, PathBuf::from("a.json"));
    }

    /// Asserts that the example has nothing to watch but keeps the session's preference.
    #[test]
    fn watch_example() {
        let mut state = watching();
        import(&mut state, "b.json", ONE_WORKLOAD);
        state.update(Action::CancelReplacement);
        state.update(Action::LoadExample);
        state.update(Action::ConfirmReplacement);

        assert_eq!(state.watch_target(), None);
        assert!(state.session.auto_reload);
    }

    /// Asserts that a changed file reloads without confirmation, resetting the workspace and
    /// keeping auto-reload on.
    #[test]
    fn reload_commits() {
        let mut state = watching();

        change(&mut state);
        assert_eq!(
            state.reload_request().unwrap().path,
            PathBuf::from("a.json")
        );
        reload(&mut state, ONE_WORKLOAD);

        let loaded = loaded(&state);
        assert!(matches!(state.import, ImportStatus::Idle));
        assert_eq!(loaded.source, source("a.json"));
        assert_eq!(loaded.dataset.workloads[0].name, "c");
        assert_eq!(loaded.workspace, Workspace::new(&loaded.dataset));
        assert!(state.session.auto_reload);
        assert_eq!(state.reload_request(), None);
        assert_eq!(loaded.reloads, 1);
    }

    /// Asserts that a reload of unchanged content keeps the workspace.
    #[test]
    fn reload_unchanged() {
        let mut state = watching();
        let generation = loaded(&state).generation;

        change(&mut state);
        reload(&mut state, TWO_WORKLOADS);

        assert_eq!(loaded(&state).generation, generation);
        assert_eq!(
            loaded(&state).workspace.selected_workload,
            Some(WorkloadId(1))
        );
    }

    /// Asserts that an invalid save shows an error and keeps the dataset and watch, and a later
    /// valid save recovers.
    #[test]
    fn reload_invalid_then_valid() {
        let mut state = watching();

        change(&mut state);
        reload(&mut state, b"{");
        assert!(state.error.is_some());
        assert_eq!(
            loaded(&state).workspace.selected_workload,
            Some(WorkloadId(1))
        );

        change(&mut state);
        reload(&mut state, ONE_WORKLOAD);
        assert_eq!(state.error, None);
        assert_eq!(loaded(&state).dataset.workloads[0].name, "c");
    }

    /// Asserts that a reload with broken occurrence information keeps the dataset and workspace,
    /// and a later valid save recovers.
    #[test]
    fn reload_broken_occurrence_then_valid() {
        let mut state = watching();
        let generation = loaded(&state).generation;

        change(&mut state);
        reload(&mut state, BROKEN_OCCURRENCE);
        assert!(matches!(
            state.error.as_ref().unwrap().error,
            ImportError::UnsupportedFormat(_)
        ));
        assert_eq!(loaded(&state).generation, generation);
        assert_eq!(
            loaded(&state).workspace.selected_workload,
            Some(WorkloadId(1))
        );

        change(&mut state);
        reload(&mut state, ONE_WORKLOAD);
        assert_eq!(state.error, None);
        assert_eq!(loaded(&state).dataset.workloads[0].name, "c");
    }

    /// Asserts that a failed read shows an error and keeps the dataset.
    #[test]
    fn reload_read_error() {
        let mut state = watching();

        change(&mut state);
        reload_with(&mut state, Err(ImportError::Read("locked".to_string())));

        assert_eq!(state.error.as_ref().unwrap().source_name, "a.json");
        assert_eq!(
            loaded(&state).workspace.selected_workload,
            Some(WorkloadId(1))
        );
        assert!(state.watch_target().is_some());
    }

    /// Asserts that turning auto-reload off drops the pending reload, even when its read
    /// completes afterwards.
    #[test]
    fn disable_during_read() {
        let mut state = watching();
        change(&mut state);
        let reload = state.reload_request().unwrap().id;

        state.update(Action::SetAutoReload(false));
        state.update(Action::ReloadRead {
            reload,
            read: Ok(ONE_WORKLOAD.to_vec()),
        });

        assert_eq!(state.reload_request(), None);
        assert_eq!(loaded(&state).dataset.workloads.len(), 2);
    }

    /// Asserts that a reload of A completing while B waits for confirmation doesn't commit, and
    /// confirming B follows B without reloading.
    #[test]
    fn replace_during_reload() {
        let mut state = watching();
        change(&mut state);
        let reload = state.reload_request().unwrap().id;
        import(&mut state, "b.json", ONE_WORKLOAD);

        state.update(Action::ReloadRead {
            reload,
            read: Ok(REPEATED.to_vec()),
        });
        assert!(matches!(state.import, ImportStatus::Confirming(_)));
        assert_eq!(loaded(&state).source, source("a.json"));
        assert_eq!(loaded(&state).dataset.workloads.len(), 2);

        state.update(Action::ConfirmReplacement);
        assert_eq!(loaded(&state).source, source("b.json"));
        assert_eq!(target(&state).path, PathBuf::from("b.json"));
        assert_eq!(state.reload_request(), None);
    }

    /// Asserts that the previous file's watcher is ignored once another file is committed.
    #[test]
    fn stale_watcher() {
        let mut state = watching();
        let old = target(&state).watch;
        import(&mut state, "b.json", ONE_WORKLOAD);
        state.update(Action::ConfirmReplacement);

        state.update(Action::FileChanged(old));

        assert_eq!(state.reload_request(), None);
    }

    /// Asserts that a change during a pending confirmation waits, and cancelling reloads the
    /// file still committed.
    #[test]
    fn cancel_resumes() {
        let mut state = watching();
        import(&mut state, "b.json", ONE_WORKLOAD);

        change(&mut state);
        assert_eq!(state.reload_request(), None);

        state.update(Action::CancelReplacement);
        assert_eq!(
            state.reload_request().unwrap().path,
            PathBuf::from("a.json")
        );
    }

    /// Asserts that a change during a manual read waits until that import fails.
    #[test]
    fn failed_import_resumes() {
        let mut state = watching();
        let request = state.start_import();
        change(&mut state);
        assert_eq!(state.reload_request(), None);

        state.update(read(request, "bad.json", b"{"));

        assert_eq!(
            state.reload_request().unwrap().path,
            PathBuf::from("a.json")
        );
    }

    /// Asserts that a manual import still asks for confirmation with auto-reload on.
    #[test]
    fn manual_confirmation() {
        let mut state = watching();

        import(&mut state, "a.json", ONE_WORKLOAD);

        assert!(matches!(state.import, ImportStatus::Confirming(_)));
    }

    /// Asserts that changes during a reload's read are coalesced into one later read.
    #[test]
    fn coalesce() {
        let mut state = watching();
        change(&mut state);
        let first = state.reload_request().unwrap().id;

        change(&mut state);
        change(&mut state);
        assert_eq!(state.reload_request().unwrap().id, first);

        reload(&mut state, TWO_WORKLOADS);
        let second = state.reload_request().unwrap().id;
        assert_ne!(second, first);

        reload(&mut state, TWO_WORKLOADS);
        assert_eq!(state.reload_request(), None);
    }

    /// Asserts that toggling auto-reload makes the earlier watch and reads stale.
    #[test]
    fn toggle_restarts() {
        let mut state = watching();
        let old = target(&state).watch;
        change(&mut state);
        let reload = state.reload_request().unwrap().id;

        state.update(Action::SetAutoReload(false));
        state.update(Action::SetAutoReload(true));
        state.update(Action::FileChanged(old));
        state.update(Action::ReloadRead {
            reload,
            read: Ok(ONE_WORKLOAD.to_vec()),
        });

        assert_ne!(target(&state).watch, old);
        assert_eq!(state.reload_request(), None);
        assert_eq!(loaded(&state).dataset.workloads.len(), 2);
    }
}
