//! Tool window framework — docked/floating panels on the left/right/bottom
//! docks.
//!
//! The manager owns entries; the built-in windows live in their own files
//! (`gui_find_tool`, `gui_output_tool`, …).  Windows return `ToolAction`s that
//! `GuiApp` applies — the same actions-as-data contract as dialogs.  The render
//! paths live in `tool_window_view`.

use eframe::egui;
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;

// ── ToolWindow trait ──────────────────────────────────────────────────────────

/// A request from a tool window (navigation hit, terminal lifecycle action).
/// Actions-as-data: windows read editor state, the shell applies mutations.
pub enum ToolAction {
    Goto { path: String, line: usize },
    TerminalFocus(usize),
    TerminalNew,
    TerminalClose(usize),
    TerminalRename { id: usize, name: String },
    TerminalResize { id: usize, cols: u16, rows: u16 },
    /// Invoke a debug command (`dbg-continue`, `dbg-step-over`, …).
    DebugCmd(&'static str),
    /// Submit a prompt to the built-in agent.
    AgentSubmit(String),
    /// Ask a running agent session to stop.
    AgentStop(u64),
    /// Evaluate an expression in the debug console.
    DebugEval(String),
    /// Fetch the children of a variables reference.
    DebugExpand(i64),
    WatchAdd(String),
    WatchRemove(String),
    BreakpointToggle { file: String, line: usize },
    BreakpointRemove { file: String, line: usize },
}

pub trait ToolWindow: Send {
    fn title(&self) -> &str;
    fn icon(&self)  -> char;
    /// Render the tool window's content; returns navigation requests.
    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction>;
}

// ── Dock position ─────────────────────────────────────────────────────────────

/// The terminal dock's index in the manager's entry list.
pub const TERMINAL_TOOL: usize = 0;
/// The output window's index.
pub const OUTPUT_TOOL: usize = 2;
/// The debug window's index.
pub const DEBUG_TOOL: usize = 3;
/// The agent window's index.
pub const AGENT_TOOL: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum DockPosition {
    Bottom,
    Right,
    /// The left dock, shared with the Project/Commit tool windows and driven
    /// by `GuiLayout::left_tool` (JetBrains docks the Debugger on the left).
    Left,
    Floating,
}

// ── ToolWindowEntry ───────────────────────────────────────────────────────────

pub struct ToolWindowEntry {
    pub window:  Box<dyn ToolWindow>,
    pub visible: bool,
    pub dock:    DockPosition,
}

// ── ToolWindowManager ─────────────────────────────────────────────────────────

pub struct ToolWindowManager {
    pub entries: Vec<ToolWindowEntry>,
    /// Index of the active tab when multiple windows are docked bottom.
    pub active_bottom: usize,
    /// Index of the active window docked on the right.
    pub active_right: usize,
}

impl ToolWindowManager {
    pub fn new() -> Self {
        Self {
            entries: vec![
                ToolWindowEntry {
                    window:  Box::new(super::gui_terminal_tool::TerminalToolWindow::new()),
                    visible: false,
                    dock:    DockPosition::Bottom,
                },
                ToolWindowEntry {
                    window:  Box::new(super::gui_find_tool::FindInFilesToolWindow::new()),
                    visible: false,
                    dock:    DockPosition::Bottom,
                },
                ToolWindowEntry {
                    window:  Box::new(super::gui_output_tool::OutputToolWindow::new()),
                    visible: false,
                    dock:    DockPosition::Bottom,
                },
                ToolWindowEntry {
                    window:  Box::new(super::gui_debug_tool::DebugToolWindow::new()),
                    visible: false,
                    dock:    DockPosition::Left,
                },
                ToolWindowEntry {
                    window:  Box::new(super::gui_agent_tool::AgentToolWindow::new()),
                    visible: false,
                    dock:    DockPosition::Right,
                },
            ],
            active_bottom: 0,
            active_right:  AGENT_TOOL,
        }
    }

    /// Toggle the visibility of the tool window at `idx`.
    pub fn toggle(&mut self, idx: usize) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.visible = !e.visible;
            if e.visible {
                match e.dock {
                    DockPosition::Bottom => self.active_bottom = idx,
                    DockPosition::Right  => self.active_right = idx,
                    // Left-docked windows are shown by `GuiLayout::left_tool`,
                    // not by this flag.
                    DockPosition::Left | DockPosition::Floating => {}
                }
            }
        }
    }

    /// The index of the currently active window docked on the right, if any.
    pub fn active_right_index(&self) -> Option<usize> {
        let e = self.entries.get(self.active_right)?;
        (e.dock == DockPosition::Right && e.visible).then_some(self.active_right)
    }
}

impl Default for ToolWindowManager {
    fn default() -> Self { Self::new() }
}
