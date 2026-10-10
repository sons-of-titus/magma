//! Sidebar interaction state and the actions it produces — split from the
//! tree renderer (`gui_sidebar.rs`) so each file stays at its abstraction.

use std::collections::HashSet;

/// What the user asked the shell to do from the sidebar.
pub enum SidebarAction {
    Open(String),
    CreateFile { dir: String, name: String },
    Rename { from: String, to: String },
    Delete(String),
    CopyPath(String),
}

/// An operation waiting for a name from the inline input row.
pub enum PendingOp {
    NewFile { dir: String },
    Rename { full_path: String },
}

impl PendingOp {
    pub fn hint(&self) -> String {
        match self {
            PendingOp::NewFile { dir } => format!("new file in {dir}/…  (Enter, Esc cancels)"),
            PendingOp::Rename { full_path } => {
                let old = full_path.rsplit('/').next().unwrap_or("");
                format!("rename {old} → …  (Enter, Esc cancels)")
            }
        }
    }
}

/// Interaction state for the project tree sidebar.
pub struct SidebarState {
    /// Project root the state was built for; a change resets everything.
    pub root:          Option<String>,
    pub expanded_dirs: HashSet<String>,
    pub selected:      Option<String>,
    /// Inline text row collecting a name for a new/rename operation.
    pub pending:       Option<PendingOp>,
    /// The in-progress name typed into the inline row.
    pub pending_name:  String,
    /// Path armed for deletion via the context menu; a second menu
    /// confirmation executes it.
    pub confirm_delete: Option<String>,
    /// Built FileNode tree cached by project files-epoch — the sidebar
    /// must not re-walk the flat list every frame (rework.md §7).  Shared
    /// via Rc so frames borrow it without pinning the state.
    pub tree_cache: Option<(u64, std::rc::Rc<Vec<crate::kernel::render::gui_sidebar::FileNode>>)>,

}

impl SidebarState {
    /// Drop all per-project interaction state.
    pub fn reset_for(&mut self, root: String) {
        self.root = Some(root);
        self.expanded_dirs.clear();
        self.selected = None;
        self.pending = None;
        self.pending_name.clear();
        self.confirm_delete = None;
    }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            tree_cache: None,
            root: None,
            expanded_dirs: HashSet::new(),
            selected: None,
            pending: None,
            pending_name: String::new(),
            confirm_delete: None,
        }
    }
}
