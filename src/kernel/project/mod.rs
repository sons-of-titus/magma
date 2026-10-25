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

use std::collections::HashMap;
use std::path::PathBuf;

use crate::kernel::run_config::RunConfigManager;

pub mod detect;
pub mod gitignore;
pub mod toolchain;

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
    pub files: Vec<String>,
    /// Bumped whenever the file list is rebuilt — render caches key on it.
    pub files_epoch: u64,
    pub file_index_dirty: bool,
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
        let Some(language) = detect::detect_language(path) else { return };

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
            && self.projects.contains_key(&name) {
                self.projects.insert(name, proj);
            }
    }

    /// Launch one of the project's run configurations through the Task
    /// System.
    pub fn launch_config(
        &mut self,
        name: &str,
        scheduler: &mut crate::kernel::task::TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        let root = self.project.root.clone()
            .ok_or_else(|| "no project open".to_string())?;
        self.run_configs.launch(name, scheduler, bg, &root)
    }

    /// Dispatch the project's `build` config.
    pub fn build(
        &mut self,
        scheduler: &mut crate::kernel::task::TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        self.launch_config("build", scheduler, bg)
    }

    /// Dispatch the project's `test` config.
    pub fn test(
        &mut self,
        scheduler: &mut crate::kernel::task::TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        self.launch_config("test", scheduler, bg)
    }

    /// Dispatch the project's `run` config.
    pub fn run(
        &mut self,
        scheduler: &mut crate::kernel::task::TaskScheduler,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<u64, String> {
        self.launch_config("run", scheduler, bg)
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
/// run Janet handlers (workspace detection, run-config seeding).  The run
/// configuration store resets here and is re-seeded by `detect`.
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
    editor.project_manager.run_configs = crate::kernel::run_config::RunConfigManager::default();
    // Breakpoints and watches are per-project session state: the store
    // clears with the project and the new project's session file refills it.
    editor.debug.breakpoints.clear();
    editor.debug.watches.clear();
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
    let mut stack = gitignore::IgnoreStack::default();
    walk_inner(dir, root, &mut stack, &mut files, 0)?;
    Ok(files)
}

/// Recursion depth cap: a pathological tree (or a bug in the symlink
/// guard) must never overflow the stack mid-walk.
const MAX_WALK_DEPTH: usize = 128;

fn walk_inner(
    dir: &std::path::Path,
    root: &std::path::Path,
    stack: &mut gitignore::IgnoreStack,
    files: &mut Vec<String>,
    depth: usize,
) -> std::io::Result<()> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }
    let pushed_ignore = gitignore::parse_file(&dir.join(".gitignore"))
        .map(|gi| { stack.push(gi); true })
        .unwrap_or(false);
    let result = (|| -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            // file_type() does not follow symlinks: a repo-planted link
            // can neither loop the walk nor pull outside trees in.
            let ft = entry.file_type()?;
            if ft.is_symlink() {
                continue;
            }
            let is_dir = ft.is_dir();
            if stack.ignored(&path, is_dir) {
                continue;
            }
            if is_dir {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if index_ignored_dir(&name) {
                    continue;
                }
                walk_inner(&path, root, stack, files, depth + 1)?;
            } else if let Some(rel) = path.strip_prefix(root).ok().and_then(|p| p.to_str()) {
                files.push(rel.to_string());
            }
        }
        Ok(())
    })();
    if pushed_ignore {
        stack.pop();
    }
    result
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

/// Test entry point: the index walk over `root`.
#[cfg(test)]
pub fn walk_files_for_test(root: &std::path::Path) -> Vec<String> {
    walk_dir(root, root).unwrap_or_default()
}
