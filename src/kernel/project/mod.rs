//! Project Model — Phase 5.
//!
//! `ProjectManager` is promoted from `kernel/state/project.rs` into its own
//! kernel subsystem.  A `Project` is a first-class programmable object with
//! build targets, modules, dependencies, and runtime configuration.
//!
//! Key types:
//!   `Project`        — first-class project object (replaces old `ProjectState`)
//!   `ProjectManager` — manages active project + multi-project registry
//!   `BuildTarget`    — a named compilation target backed by a Task System task
//!   `Module`         — a logical source unit within a project
//!   `Dependency`     — an external or internal project dependency

use std::collections::HashMap;
use std::path::PathBuf;

use crate::kernel::task::{TaskId, TaskScheduler};

// ── Supporting types ──────────────────────────────────────────────────────────

/// A member subproject within a workspace (monorepo support).
#[derive(Debug, Clone)]
pub struct ProjectMember {
    pub name: String,
    pub root: PathBuf,
}

/// A workspace grouping multiple subprojects.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub members: Vec<ProjectMember>,
}

/// A logical module within a project (crate, library, source directory, …).
#[derive(Debug, Clone, Default)]
pub struct Module {
    pub name: String,
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    pub language: String,
}

/// A project dependency (crate, npm package, pip package, …).
#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyKind {
    External,
    Internal,
}

/// Whether a build target produces a debug or optimised artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildKind {
    Debug,
    Release,
}

/// A named compilation target (build, test, run, …) backed by a Task System task.
#[derive(Debug, Clone)]
pub struct BuildTarget {
    pub name: String,
    pub kind: BuildKind,
    pub task_id: Option<TaskId>,
    pub artifacts: Vec<PathBuf>,
}

/// Runtime environment configuration attached to a project.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub env: HashMap<String, String>,
}

// ── Project ───────────────────────────────────────────────────────────────────

/// First-class project object.  Replaces the old `ProjectState` struct.
///
/// Carries the legacy identity/index state (root, name, workspace, files,
/// options, file_index_dirty) and the Phase 5 semantic model (language,
/// modules, dependencies, build_targets, tasks, runtime_config).
#[derive(Debug, Clone, Default)]
pub struct Project {
    // ── Identity & index ──────────────────────────────────────────────────
    pub root: Option<PathBuf>,
    pub name: Option<String>,
    pub workspace: Option<Workspace>,
    pub files: Vec<String>,
    /// Bumped whenever the file list is rebuilt — render caches key on it.
    pub files_epoch: u64,
    pub file_index_dirty: bool,
    /// Per-project editor options (e.g. `"tab-width"`, `"lsp-server"`).
    pub options: HashMap<String, String>,

    // ── Phase 5: semantic model ────────────────────────────────────────────
    /// Detected language identifier: `"rust"`, `"node"`, `"python"`, `"make"`.
    pub language: String,
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
    /// Task IDs registered for this project by `ProjectManager::detect`.
    pub tasks: Vec<TaskId>,
    pub build_targets: Vec<BuildTarget>,
    pub runtime_config: Config,
}

// ── ProjectManager ────────────────────────────────────────────────────────────

/// Manages the active project and the multi-project registry.
#[derive(Debug, Clone, Default)]
pub struct ProjectManager {
    /// The currently active project.
    pub project: Project,
    /// Registry of all known projects indexed by project name.
    pub projects: HashMap<String, Project>,
    pub current_project: Option<String>,
    /// Maps buffer slab keys to project names.
    pub buffer_projects: HashMap<usize, String>,
    pub recent_projects: Vec<String>,
}

impl ProjectManager {
    /// Auto-detect project language from marker files at `root`, define
    /// `build`/`test`/`run` tasks in `scheduler`, and populate
    /// `project.language` and `project.build_targets`.
    ///
    /// Replaces `TaskScheduler::auto_detect_project_tasks` — detection now
    /// lives in the Project Model layer and enriches the `Project` struct.
    pub fn detect(&mut self, root: &str, scheduler: &mut TaskScheduler) {
        let path = std::path::Path::new(root);

        let (language, build_cmd, test_cmd, run_cmd) =
            if path.join("Cargo.toml").exists() {
                ("rust", "cargo build", "cargo test", "cargo run")
            } else if path.join("package.json").exists() {
                ("node", "npm run build", "npm test", "npm start")
            } else if path.join("Makefile").exists() {
                ("make", "make", "make test", "make run")
            } else if path.join("pyproject.toml").exists()
                || path.join("setup.py").exists()
            {
                ("python", "python -m build", "python -m pytest", "python -m main")
            } else {
                return;
            };

        self.project.language = language.to_string();

        let build_id = scheduler.define("build".into(), build_cmd.into(), HashMap::new(), vec![]);
        let test_id  = scheduler.define("test".into(),  test_cmd.into(),  HashMap::new(), vec![]);
        let run_id   = scheduler.define("run".into(),   run_cmd.into(),   HashMap::new(), vec![]);

        self.project.build_targets = vec![
            BuildTarget { name: "build".into(), kind: BuildKind::Debug, task_id: Some(build_id), artifacts: vec![] },
            BuildTarget { name: "test".into(),  kind: BuildKind::Debug, task_id: Some(test_id),  artifacts: vec![] },
            BuildTarget { name: "run".into(),   kind: BuildKind::Debug, task_id: Some(run_id),   artifacts: vec![] },
        ];
        self.project.tasks = vec![build_id, test_id, run_id];
    }

    /// Dispatch the project's `build` target through the Task System.
    pub fn build(
        &self,
        scheduler: &mut TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<TaskId, String> {
        scheduler.run_by_name("build", bg)
    }

    /// Dispatch the project's `test` target through the Task System.
    pub fn test(
        &self,
        scheduler: &mut TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<TaskId, String> {
        scheduler.run_by_name("test", bg)
    }

    /// Dispatch the project's `run` target through the Task System.
    pub fn run(
        &self,
        scheduler: &mut TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<TaskId, String> {
        scheduler.run_by_name("run", bg)
    }

    /// Sync the project's current file list into the Semantic Engine's
    /// `ProjectGraph` using the detected language as the file language.
    ///
    /// Called after file indexing completes (`FileIndexed` background event)
    /// so the Semantic Engine can route providers to newly indexed files.
    pub fn sync_to_project_graph(
        &self,
        graph: &mut crate::kernel::semantic::project_graph::ProjectGraph,
    ) {
        let lang = self.project.language.as_str();
        for file_path in &self.project.files {
            if !graph.files.contains_key(file_path.as_str()) {
                graph.add_file(file_path, lang, vec![]);
            }
        }
    }
}

// ── Opening projects ──────────────────────────────────────────────────────────

/// Set `path` as the active project root, register it, emit `project-opened`,
/// and record it in the recent-projects list.  Emits `project-closed` first
/// when a project was already active.  Call `dispatch_events_scoped` after to
/// run Janet handlers (workspace detection, …).
pub fn open_root(editor: &mut crate::kernel::state::Editor, path: &str) {
    use crate::kernel::event::keys;

    let was_active = editor.project_manager.project.root.is_some();
    if was_active {
        editor.events.emit_typed(keys::events::PROJECT_CLOSED, crate::kernel::event::payload::EmptyPayload);
    }

    let pb = PathBuf::from(path);
    let name = pb.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    editor.project_manager.project = Project {
        root: Some(pb),
        name: Some(name.clone()),
        file_index_dirty: true,
        ..Project::default()
    };
    editor.project_manager.current_project = Some(name.clone());
    editor.project_manager.projects.insert(name.clone(), editor.project_manager.project.clone());

    push_recent(editor, path);
    editor.events.emit_typed(
        keys::events::PROJECT_OPENED,
        crate::kernel::event::payload::ProjectOpenedPayload { root: path.to_string(), name },
    );
}

/// Move `path` to the front of the recent list (deduped, capped at 50)
/// and persist it.
pub fn push_recent(editor: &mut crate::kernel::state::Editor, path: &str) {
    editor.project_manager.recent_projects.retain(|p| p != path);
    editor.project_manager.recent_projects.insert(0, path.to_string());
    editor.project_manager.recent_projects.truncate(50);
    crate::kernel::state::persist::save_recent_projects(&editor.project_manager.recent_projects);
}

/// Remove `path` from the recent list and persist.
pub fn remove_recent(editor: &mut crate::kernel::state::Editor, path: &str) {
    editor.project_manager.recent_projects.retain(|p| p != path);
    crate::kernel::state::persist::save_recent_projects(&editor.project_manager.recent_projects);
}

/// Directories the index never descends into: dot-dirs (VCS, tool state),
/// build output, and dependency trees.
fn index_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "target"
}

/// Does this watcher event change the project's file list?
///
/// Churn inside an ignored directory never lands in the index, and a data
/// write to an already-indexed file doesn't change the list — only creates,
/// removals, and the first sighting of an unknown path do.
pub fn index_change_marks_dirty(root: &std::path::Path, path: &str, kind: &str, indexed: &[String]) -> bool {
    let Ok(rel_path) = std::path::Path::new(path).strip_prefix(root) else { return false };
    let mut components = rel_path.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_some()
            && index_ignored_dir(component.as_os_str().to_string_lossy().as_ref()) {
                return false;
            }
    }
    match kind {
        "removed" => true,
        "modified" => !indexed.contains(&rel_path.to_string_lossy().into_owned()),
        _ => true,
    }
}

/// Recursively list files under `dir`, skipping VCS/build/dependency dirs.
/// Paths are stored **relative to `root`** — the project's canonical file
/// identity everywhere (sidebar tree, quick-open, project graph).
fn walk_dir(dir: &std::path::Path, root: &std::path::Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if index_ignored_dir(&name) {
                    continue;
                }
                if let Ok(mut sub) = walk_dir(&path, root) {
                    files.append(&mut sub);
                }
            } else if path.is_file()
                && let Some(rel) = path.strip_prefix(root).ok().and_then(|p| p.to_str()) {
                    files.push(rel.to_string());
                }
        }
    }
    Ok(files)
}

/// Enqueue an async walk of the active project's files.  Results arrive as
/// `BackgroundEvent::FileIndexed`; the dirty flag clears at enqueue time so
/// a slow walk never queues duplicates.
pub fn spawn_file_index(ed: &mut crate::kernel::state::Editor) -> bool {
    let Some(root) = ed.project_manager.project.root.clone() else { return false };
    let Some(bg) = ed.background.clone() else { return false };
    let name = ed.project_manager.project.name.clone().unwrap_or_default();
    let sender = bg.sender.clone();
    ed.project_manager.project.file_index_dirty = false;
    bg.spawn_blocking(move || {
        let files = walk_dir(&root, &root).unwrap_or_default();
        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::FileIndexed {
            project_name: name,
            files,
        });
    });
    true
}
