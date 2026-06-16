use std::path::PathBuf;

/// A member subproject within a workspace.
#[derive(Debug, Clone)]
pub struct ProjectMember {
    pub name: String,
    pub root: PathBuf,
}

/// A workspace grouping multiple subprojects (monorepo support).
#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub members: Vec<ProjectMember>,
}

/// Per-project state stored on the editor and in the multi-project registry.
#[derive(Debug, Clone, Default)]
pub struct ProjectState {
    pub root: Option<PathBuf>,
    pub name: Option<String>,
    pub workspace: Option<Workspace>,
    pub files: Vec<String>,
    pub options: std::collections::HashMap<String, String>,
    pub file_index_dirty: bool,
}

/// All project-management state for the editor.
#[derive(Debug, Clone, Default)]
pub struct ProjectManager {
    pub project: ProjectState,
    pub projects: std::collections::HashMap<String, ProjectState>,
    pub current_project: Option<String>,
    pub buffer_projects: std::collections::HashMap<usize, String>,
    pub recent_projects: Vec<String>,
}
