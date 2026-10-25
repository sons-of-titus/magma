//! Dock widgets and the editor shell's panel construction, kept out of
//! `editor_shell` so that function stays a thin orchestrator.
//!
//! `show_panels` builds every dock/top/bottom chrome panel (menu + main
//! toolbar row, status bar, bottom tool bar, tabs + breadcrumb, left rail +
//! dock, right rail + Agent dock) and returns the actions the shell applies at
//! its write boundary.

use std::collections::HashMap;

use eframe::egui;

use crate::kernel::command::args::ArgValue;
use crate::kernel::state::{Editor, MessageKind};

use super::gui::GuiApp;
use super::gui_breadcrumb::{BreadcrumbAction, BreadcrumbBar};
use super::gui_commit_tool::{CommitAction, CommitPanel};
use super::gui_dialog::DialogManager;
use super::gui_editor_pane::EditorPaneAction;
use super::gui_layout::{GuiLayout, LeftTool};
use super::gui_menu::{MenuAction, MenuBar};
use super::gui_run_tool::{RunAction, RunBar};
use super::gui_sidebar::ProjectTree;
use super::gui_sidebar_state::SidebarAction;
use super::gui_status::{StatusAction, StatusBar};
use super::gui_tabs::{TabAction, TabBar};
use super::gui_theme::{GuiTheme, icons};
use super::tool_window::{AGENT_TOOL, OUTPUT_TOOL, TERMINAL_TOOL, ToolAction, ToolWindowManager};
use super::tool_window_view::dock_button;

/// Actions collected from the chrome panels, applied by `editor_shell`.
pub(crate) struct PanelOutput {
    pub tab:        Option<TabAction>,
    pub sidebar:    Vec<SidebarAction>,
    pub breadcrumb: Option<BreadcrumbAction>,
    pub menu:       Vec<MenuAction>,
    pub commit:     Vec<CommitAction>,
    pub run:        Vec<RunAction>,
    pub tool:       Vec<ToolAction>,
    pub status:     Option<StatusAction>,
    pub left_clicked: Option<LeftTool>,
}

/// The left tool rail: Project, Commit, and Debugger buttons — the JetBrains
/// left strip.  Returns the newly selected tool when one is clicked.
pub(crate) fn left_rail(ui: &mut egui::Ui, active: LeftTool, theme: &GuiTheme) -> Option<LeftTool> {
    let mut clicked = None;
    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.add_space(8.0);
        if dock_button(ui, icons::PROJECTS, active == LeftTool::Project, "Project", theme, true) {
            clicked = Some(LeftTool::Project);
        }
        ui.add_space(4.0);
        if dock_button(ui, icons::GIT_BRANCH, active == LeftTool::Commit, "Commit", theme, true) {
            clicked = Some(LeftTool::Commit);
        }
        ui.add_space(4.0);
        if dock_button(ui, icons::DEBUG, active == LeftTool::Debug, "Debugger", theme, true) {
            clicked = Some(LeftTool::Debug);
        }
    });
    clicked
}

/// Build every chrome panel.  Must be called before the central editor panel.
pub(crate) fn show_panels(app: &mut GuiApp, ui: &mut egui::Ui, line_h: f32) -> PanelOutput {
    let breadcrumb_on = app.editor.options.get("breadcrumb-enabled").map(|v| v.as_str()) != Some("false");
    let status_h = line_h + 8.0;
    let mut out = PanelOutput {
        tab: None, sidebar: Vec::new(), breadcrumb: None,
        menu: Vec::new(), commit: Vec::new(), run: Vec::new(), tool: Vec::new(), status: None,
        left_clicked: None,
    };

    // Top row — menu on the left, the main (run) toolbar on the right, like
    // JetBrains New UI's single menu+toolbar band.  Registered first so it
    // sits above the tab strip.
    let top = egui::containers::Panel::top("magma_top")
        .exact_size(30.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(app.theme.crust)
            .inner_margin(egui::Margin::symmetric(6, 0)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                out.menu = MenuBar::show(ui, &app.editor, &app.theme);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    out.run = RunBar::show(ui, &app.editor, &app.theme);
                });
            });
        });
    ui.painter().hline(
        top.response.rect.x_range(),
        top.response.rect.bottom() - 0.5,
        egui::Stroke::new(1.0, app.theme.border),
    );

    // Status bar — outermost bottom panel (panel-colored, like the tool bar,
    // so the bottom chrome never reads as a black band).
    egui::containers::Panel::bottom("magma_status")
        .exact_size(status_h)
        .frame(egui::Frame::NONE.fill(app.theme.mantle)
            .inner_margin(egui::Margin::symmetric(6, 0)))
        .show(ui, |ui| {
            out.status = StatusBar::show(ui, &app.editor, &app.theme);
        });

    // Bottom tool-window bar (JetBrains bottom dock buttons).  Panel-colored
    // so it never reads as a black band against the editor.
    let mut bottom_clicked: Option<usize> = None;
    egui::containers::Panel::bottom("magma_dock_bar")
        .exact_size(26.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(app.theme.mantle))
        .show(ui, |ui| {
            bottom_clicked = app.tool_windows.show_bottom_bar(ui, &app.theme);
        });

    // The terminal dock tracks the session set.
    if let Some(entry) = app.tool_windows.entries.first_mut() {
        if app.editor.io.terminals.sessions.is_empty() {
            entry.visible = false;
        } else if app.editor.io.terminals.focused.is_some() && !entry.visible {
            entry.visible = true;
            app.tool_windows.active_bottom = TERMINAL_TOOL;
        }
    }
    if let Some(idx) = bottom_clicked {
        if idx == TERMINAL_TOOL {
            if let Some(entry) = app.tool_windows.entries.first_mut() {
                entry.visible = true;
            }
            app.tool_windows.active_bottom = TERMINAL_TOOL;
            out.tool.push(ToolAction::TerminalNew);
        } else {
            app.tool_windows.toggle(idx);
        }
    }

    // Docked-bottom tool windows.
    out.tool.extend(app.tool_windows.show_docked_bottom(ui, &mut app.editor, &app.theme));

    // Navigation band — tabs and breadcrumb.  Registered after the top row so
    // it is the last band above the editor: the tab strip sits directly on the
    // editor, with the breadcrumb tucked under it (JetBrains layout).
    egui::containers::Panel::top("magma_nav")
        .frame(egui::Frame::NONE.fill(app.theme.crust))
        .show(ui, |ui| {
            if let Some(a) = TabBar::show(ui, &app.editor, &app.theme) {
                out.tab = Some(a);
            }
            if breadcrumb_on {
                out.breadcrumb = BreadcrumbBar::show(ui, &app.editor, &app.theme);
            }
        });

    // Left tool rail — Project and Commit only.
    egui::containers::Panel::left("magma_tool_rail")
        .exact_size(28.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(app.theme.crust).inner_margin(egui::Margin::same(0)))
        .show(ui, |ui| {
            out.left_clicked = left_rail(ui, app.layout.left_tool, &app.theme);
        });

    // Left dock — Project tree, the Commit panel, or the Debugger.  Each tool
    // gets its own panel id so egui remembers a separate width per tool.
    if app.layout.sidebar_open {
        let left_tool = app.layout.left_tool;
        let (panel_id, width) = match left_tool {
            LeftTool::Project => ("magma_dock_project", app.layout.sidebar_width),
            LeftTool::Commit  => ("magma_dock_commit",  app.layout.sidebar_width),
            LeftTool::Debug   => ("magma_dock_debug",   app.layout.debug_width),
        };
        egui::containers::Panel::left(panel_id)
            .default_size(width)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(app.theme.mantle))
            .show(ui, |ui| {
                match left_tool {
                    LeftTool::Project => {
                        out.sidebar = ProjectTree::show(ui, &app.editor, &mut app.sidebar_state, &app.theme);
                    }
                    LeftTool::Commit => {
                        out.commit = CommitPanel::show(ui, &app.editor, &app.theme, &mut app.commit_state);
                    }
                    LeftTool::Debug => {
                        out.tool.extend(app.tool_windows.show_debug_left(ui, &mut app.editor, &app.theme));
                    }
                }
            });
    }

    // Right icon rail (Agent).
    let mut right_clicked: Option<usize> = None;
    egui::containers::Panel::right("magma_agent_rail")
        .exact_size(28.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(app.theme.crust).inner_margin(egui::Margin::same(0)))
        .show(ui, |ui| {
            right_clicked = app.tool_windows.show_right_rail(ui, &app.theme);
        });

    // Right dock — the Agent panel.
    let right_width = app.layout.right_width;
    out.tool.extend(app.tool_windows.show_docked_right(ui, &mut app.editor, &app.theme, right_width));

    // Apply the right-rail click (structural, not returned).
    if let Some(idx) = right_clicked {
        app.tool_windows.toggle(idx);
        if idx == AGENT_TOOL {
            app.layout.right_open = app.tool_windows.entries[idx].visible;
        }
    }

    out
}

/// Apply run-toolbar requests.  Launching opens the corresponding tool window
/// so the run has a visible surface.
pub(crate) fn apply_run_actions(
    ed:           &mut Editor,
    tool_windows: &mut ToolWindowManager,
    layout:       &mut GuiLayout,
    dialogs:      &mut DialogManager,
    actions:      Vec<RunAction>,
) {
    for action in actions {
        match action {
            RunAction::Run | RunAction::RunNamed(_) => {
                let mut args = HashMap::new();
                if let RunAction::RunNamed(name) = action {
                    args.insert("name".to_string(), ArgValue::String(name));
                }
                if let Err(e) = crate::kernel::command::execute_command(ed, "run-config", &args) {
                    ed.show_message(MessageKind::Error, e);
                }
                // A launch always has output worth surfacing.
                if let Some(entry) = tool_windows.entries.get_mut(OUTPUT_TOOL) {
                    entry.visible = true;
                }
                tool_windows.active_bottom = OUTPUT_TOOL;
            }
            RunAction::Stop => {
                if let Err(e) = crate::kernel::command::execute_command(
                    ed, "stop-config", &HashMap::new()) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            RunAction::Debug => {
                if let Err(e) = crate::kernel::command::execute_command(
                    ed, "debug-config", &HashMap::new()) {
                    ed.show_message(MessageKind::Error, e);
                } else {
                    // JetBrains opens the Debugger tool window the moment a
                    // session starts.
                    layout.left_tool = LeftTool::Debug;
                    layout.sidebar_open = true;
                }
            }
            RunAction::Select(name) => {
                let mut args = HashMap::new();
                args.insert("name".to_string(), ArgValue::String(name));
                if let Err(e) = crate::kernel::command::execute_command(ed, "select-config", &args) {
                    ed.show_message(MessageKind::Error, e);
                }
            }
            RunAction::Edit => {
                dialogs.open_run_config_editor(ed);
            }
        }
    }
}

/// Apply a scrollbar/minimap drag: move the viewport, never the cursor.  The
/// pin holds until the cursor moves again.
pub(crate) fn apply_pane_action(ed: &mut Editor, action: EditorPaneAction) {
    let (line, col) = match action {
        EditorPaneAction::ScrollViewport(line) => (Some(line), None),
        EditorPaneAction::ScrollLeft(col)      => (None, Some(col)),
    };
    if let Some(wid) = ed.view_tree.focused_window() {
        let buf = ed.view_tree.buffer(wid);
        let cursor = buf
            .and_then(|b| ed.view_of_buffer(b))
            .map(|v| v.cursor.offset);
        if let Some(pane) = ed.view_tree.window_mut(wid) {
            if let Some(line) = line {
                pane.scroll_offset = line.saturating_sub(1);
                pane.scroll_pinned = true;
            }
            if let Some(col) = col {
                pane.scroll_left = col;
                pane.scroll_left_pinned = true;
            }
            pane.scroll_buf = buf;
            pane.scroll_cursor = cursor;
        }
    }
    ed.frame_dirty = true;
}
