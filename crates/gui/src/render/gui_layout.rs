//! GUI layout state — panel visibility and sizing.

/// Which tool occupies the left dock (JetBrains: Project / Commit / Debug).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTool {
    Project,
    Commit,
    Debug,
}

pub struct GuiLayout {
    /// Active left tool window.
    pub left_tool: LeftTool,
    /// Left dock visibility (Project tree / Commit / Debugger).
    pub sidebar_open: bool,
    pub sidebar_width: f32,
    /// Left dock width while the Debugger holds it — wider than the tree,
    /// since the Frames tab pairs a call stack with a variables tree.
    pub debug_width: f32,
    /// Right dock (Agent) visibility and width.
    pub right_open: bool,
    pub right_width: f32,
}

impl Default for GuiLayout {
    fn default() -> Self {
        Self {
            left_tool: LeftTool::Project,
            sidebar_open: true,
            sidebar_width: 240.0,
            debug_width: 440.0,
            right_open: false,
            right_width: 340.0,
        }
    }
}
