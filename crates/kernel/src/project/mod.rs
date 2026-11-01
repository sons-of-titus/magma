//! Project Model — Phase 5.
//!
//! `ProjectManager` owns the active project, the multi-project registry, and
//! the project's run configurations.  A `Project` is a first-class
//! programmable object with a detected language and file index; launchable
//! build/test/run targets are `run_config::RunConfig`s seeded by `detect`.
//!
//! Key types:
//!   `Project`         — first-class project object
//!   `ProjectManager`  — active project + registry + run configurations
//!   `ProjectMember` / `Workspace` — monorepo grouping

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::run_config::RunConfigManager;

pub use project::{Event, detect, gitignore, toolchain};
pub mod tree;
mod walk;

pub use walk::{index_change_marks_dirty, spawn_file_index, walk_files_for_test};

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

// ── Project ───────────────────────────────────────────────────────────────────

/// First-class project object.
///
/// Carries identity/index state (root, name, workspace, files, options,
/// file_index_dirty) and the detected language; launchable targets live in
/// `ProjectManager::run_configs`.
#[derive(Debug, Clone, Default)]
pub struct Project {
    // ── Identity & index ──────────────────────────────────────────────────
    pub root: Option<PathBuf>,
    pub name: Option<String>,
    pub workspace: Option<Workspace>,
    /// Open-generation counter, bumped on every `open_root`.  Background
    /// jobs stamp results with the generation that spawned them and the
    /// foreground drops anything stale, so a slow walk, extraction, or
    /// session read for project A can never mutate project B after a switch.
    pub generation: u64,
    pub files: Vec<String>,
    pub ignored_files: HashSet<String>,
    /// Bumped whenever the file list is rebuilt — render caches key on it.
    pub files_epoch: u64,
    pub file_index_dirty: bool,
    /// True while a walk for this project generation is still in flight —
    /// the tree can show a loading hint before the first batch lands.
    pub indexing: bool,
    /// True while background run-config detection for this generation is in
    /// flight; cleared when its `TasksDetected` result lands.
    pub detecting_tasks: bool,
    /// Per-project editor options (e.g. `"tab-width"`, `"lsp-server"`).
    pub options: HashMap<String, String>,

    // ── Detection ─────────────────────────────────────────────────────────
    /// Detected language identifier: `"rust"`, `"node"`, `"python"`, `"make"`.
    pub language: String,
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
}

// ── ProjectManager ────────────────────────────────────────────────────────────

/// Manages the active project, the multi-project registry, and the active
/// project's run configurations.
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
    /// Run configurations for the active project.
    pub run_configs: RunConfigManager,
}

impl ProjectManager {
    /// Detect the project language from marker files at `root` (table in
    /// `detect.rs`, first match wins) and seed the project's run
    /// configurations (saved configs when present, detection defaults
    /// otherwise).
    pub fn detect(&mut self, root: &str) {
        let path = std::path::Path::new(root);
        let Some(language) = detect::detect_language(path) else {
            return;
        };

        self.project.language = language.to_string();
        let pb = PathBuf::from(root);
        self.run_configs.load_or_defaults(&pb, language);
        self.sync_active_to_registry();
    }

    /// Copy the active project's current state into the name-keyed registry
    /// — the registry snapshot would otherwise go stale the moment
    /// detection or indexing enriches the active project.
    pub fn sync_active_to_registry(&mut self) {
        if let (Some(name), proj) = (self.current_project.clone(), self.project.clone())
            && self.projects.contains_key(&name)
        {
            self.projects.insert(name, proj);
        }
    }

    /// Launch one of the project's run configurations through the Task
    /// System.
    pub fn launch_config(
        &mut self,
        name: &str,
        scheduler: &mut crate::task::TaskScheduler,
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        let root = self
            .project
            .root
            .clone()
            .ok_or_else(|| "no project open".to_string())?;
        self.run_configs.launch(name, scheduler, bg, &root)
    }

    /// Dispatch the project's `build` config.
    pub fn build(
        &mut self,
        scheduler: &mut crate::task::TaskScheduler,
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        self.launch_config("build", scheduler, bg)
    }

    /// Dispatch the project's `test` config.
    pub fn test(
        &mut self,
        scheduler: &mut crate::task::TaskScheduler,
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        self.launch_config("test", scheduler, bg)
    }

    /// Dispatch the project's `run` config.
    pub fn run(
        &mut self,
        scheduler: &mut crate::task::TaskScheduler,
        bg: &crate::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        self.launch_config("run", scheduler, bg)
    }

    /// Sync the project's current file list into the Semantic Engine's
    /// `ProjectGraph` using the detected language as the file language.
    ///
    /// Called after file indexing completes (`index::Event::ProjectIndexed`)
    /// so the Semantic Engine can route providers to newly indexed files.
    pub fn sync_to_project_graph(&self, graph: &mut crate::semantic::project_graph::ProjectGraph) {
        let lang = self.project.language.as_str();
        for file_path in &self.project.files {
            if !graph.files.contains_key(file_path.as_str()) {
                graph.add_file(file_path, lang, vec![]);
            }
        }
    }
}

// ── Opening projects ──────────────────────────────────────────────────────────

/// Anchor `path` to the active project root.  The sidebar tree, finder
/// results, and session files speak project-relative paths, while the fs
/// layer resolves against the process cwd — which is not the project root
/// in general.  Absolute paths pass through unchanged.
pub fn resolve_path(root: Option<&std::path::Path>, path: &str) -> String {
    if std::path::Path::new(path).is_absolute() {
        return path.to_string();
    }
    match root {
        Some(root) => root.join(path).to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

/// Close every buffer and clear pane buffer references — the teardown of
/// one editing context.  A project switch or editor close must not carry
/// this project's files into the next one's tabs.  Call only when no
/// unsaved user work remains (`dirty_buffer_names` is empty): dropped
/// buffers are gone, there is no undo across the boundary.
pub fn close_all_buffers(ed: &mut crate::state::Editor) {
    let doomed: Vec<usize> = ed.buffers.iter().map(|(k, _)| k).collect();
    for key in doomed {
        ed.events.emit_typed(
            crate::event::keys::events::BUFFER_CLOSED,
            crate::event::payload::BufferClosedPayload {
                buffer_id: key.to_string(),
            },
        );
        ed.drop_buffer(key);
    }
    ed.alternate_buffer = None;
    ed.project_manager.buffer_projects.clear();
    ed.io.saves_in_flight.clear();
}

/// Set `path` as the active project root, register it, emit `project-opened`,
/// and record it in the recent-projects list.  Emits `project-closed` first
/// when a project was already active — refusing the switch while buffers
/// hold unsaved work, snapshotting the outgoing session, and closing its
/// buffers otherwise.  Call `dispatch_events_scoped` after to run Janet
/// handlers (workspace detection, run-config seeding).  The run
/// configuration store resets here and is re-seeded by `detect`.
pub fn open_root(editor: &mut crate::state::Editor, path: &str) -> Result<(), String> {
    use crate::event::keys;

    let was_active = editor.project_manager.project.root.is_some();
    if was_active {
        let dirty = editor.dirty_buffer_names();
        if !dirty.is_empty() {
            return Err(format!(
                "Save or close {} modified buffer(s) before switching projects",
                dirty.len()
            ));
        }
        // Switching projects snapshots the outgoing session first, so
        // each project reopens exactly as it was left.
        crate::session::save_if_project(editor);
        editor.events.emit_typed(
            keys::events::PROJECT_CLOSED,
            crate::event::payload::EmptyPayload,
        );
        close_all_buffers(editor);
    }

    let pb = PathBuf::from(path);
    let name = pb
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    editor.project_manager.run_configs = crate::run_config::RunConfigManager::default();
    // Breakpoints and watches are per-project session state: the store
    // clears with the project and the new project's session file refills it.
    editor.debug.breakpoints.clear();
    editor.debug.watches.clear();
    // A restore still draining for the outgoing project must not show on
    // the new one's progress surface, and the outgoing generation's
    // outstanding extraction slots are invalidated with it — their records
    // would be dropped as stale.
    editor.session_restore_pending = 0;
    editor.index.clear_pending();
    let generation = editor.project_manager.project.generation.wrapping_add(1);
    editor.project_manager.project = Project {
        root: Some(pb),
        name: Some(name.clone()),
        generation,
        file_index_dirty: true,
        ..Project::default()
    };
    editor.project_manager.current_project = Some(name.clone());
    editor
        .project_manager
        .projects
        .insert(name.clone(), editor.project_manager.project.clone());

    push_recent(editor, path);
    editor.events.emit_typed(
        keys::events::PROJECT_OPENED,
        crate::event::payload::ProjectOpenedPayload {
            root: path.to_string(),
            name,
        },
    );
    Ok(())
}

/// Move `path` to the front of the recent list (deduped, capped at 50)
/// and persist it.
pub fn push_recent(editor: &mut crate::state::Editor, path: &str) {
    editor.project_manager.recent_projects.retain(|p| p != path);
    editor
        .project_manager
        .recent_projects
        .insert(0, path.to_string());
    editor.project_manager.recent_projects.truncate(50);
    crate::state::persist::save_recent_projects(&editor.project_manager.recent_projects);
}

/// Remove `path` from the recent list and persist.
pub fn remove_recent(editor: &mut crate::state::Editor, path: &str) {
    editor.project_manager.recent_projects.retain(|p| p != path);
    crate::state::persist::save_recent_projects(&editor.project_manager.recent_projects);
}
