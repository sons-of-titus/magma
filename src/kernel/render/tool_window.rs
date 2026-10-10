//! Tool window framework — docked/floating panels on the left tool strip.
//!
//! The manager owns entries; the built-in windows live in their own files
//! (`gui_find_tool`, `gui_output_tool`).  Windows return `ToolAction`s that
//! `GuiApp` applies — the same actions-as-data contract as dialogs.

use eframe::egui::{self, FontFamily, FontId, Pos2, Sense};
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;

fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

// ── ToolWindow trait ──────────────────────────────────────────────────────────

/// A request from a tool window (navigation hit, terminal lifecycle action).
/// Actions-as-data: windows read editor state, the shell applies mutations.
pub enum ToolAction {
    Goto { path: String, line: usize },
    TerminalFocus(usize),
    TerminalNew,
    TerminalClose(usize),
    TerminalResize { id: usize, cols: u16, rows: u16 },
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

#[derive(Debug, Clone, PartialEq)]
pub enum DockPosition {
    Bottom,
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
    /// Index of the active tab when multiple windows are docked to the same panel.
    pub active_bottom: usize,
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
                    dock:    DockPosition::Bottom,
                },
            ],
            active_bottom: 0,
        }
    }

    /// Toggle the visibility of tool window at `idx`.
    pub fn toggle(&mut self, idx: usize) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.visible = !e.visible;
            if e.visible && e.dock == DockPosition::Bottom {
                self.active_bottom = idx;
            }
        }
    }

    /// Render the narrow vertical icon strip on the left edge.
    ///
    /// Returns `Some(idx)` when a button was clicked.
    pub fn show_sidebar_edge(&self, ui: &mut egui::Ui, theme: &GuiTheme) -> Option<usize> {
        let mut toggled = None;
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            for (i, entry) in self.entries.iter().enumerate() {
                let icon = entry.window.icon().to_string();
                let active = entry.visible;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::click());
                if active {
                    // JetBrains active-tool indicator: a blue bar on the
                    // window edge beside the icon.
                    let left = ui.max_rect().left();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_max(
                            Pos2::new(left, rect.top() + 3.0),
                            Pos2::new(left + 2.5, rect.bottom() - 3.0),
                        ),
                        1.0,
                        theme.blue,
                    );
                }
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 6, theme.surface0.gamma_multiply(0.5));
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &icon,
                    prop(15.0),
                    if active { theme.blue } else { theme.overlay0 },
                );
                let resp = resp.on_hover_text(entry.window.title());
                if resp.clicked() {
                    toggled = Some(i);
                }
                ui.add_space(4.0);
            }
        });
        toggled
    }

    /// Render docked-bottom tool windows as a resizable bottom panel.
    ///
    /// Returns navigation actions from the active window.
    pub fn show_docked_bottom(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let visible: Vec<usize> = self.entries.iter().enumerate()
            .filter(|(_, e)| e.visible && e.dock == DockPosition::Bottom)
            .map(|(i, _)| i)
            .collect();
        if visible.is_empty() { return Vec::new() }

        let mut actions = Vec::new();
        egui::containers::Panel::bottom("magma_tools_bottom")
            .resizable(true)
            .default_size(200.0)
            .frame(egui::Frame::NONE.fill(theme.mantle).inner_margin(egui::Margin::same(0)))
            .show(ui, |ui| {
                // Tab strip — quiet text tabs, active one filled like the
                // welcome nav.
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.set_min_height(28.0);
                    for &idx in &visible {
                        let title = self.entries[idx].window.title();
                        let is_active = idx == self.active_bottom;
                        let w = ui.painter()
                            .layout_no_wrap(title.to_owned(), prop(12.0), theme.text)
                            .size().x + 16.0;
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::click());
                        if is_active {
                            ui.painter().rect_filled(rect, 4, theme.surface0);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 4, theme.surface0.gamma_multiply(0.5));
                        }
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            title,
                            prop(12.0),
                            if is_active { theme.text } else { theme.overlay0 },
                        );
                        if resp.clicked() {
                            self.active_bottom = idx;
                        }
                    }
                });
                ui.separator();
                // Active window content
                if let Some(&active_idx) = visible.iter().find(|&&i| i == self.active_bottom)
                    .or_else(|| visible.first())
                {
                    self.active_bottom = active_idx;
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            actions = self.entries[active_idx].window.ui(ui, editor, theme);
                        });
                }
            });
        actions
    }

    /// Render floating tool windows as independent egui Windows.
    pub fn show_floating(&mut self, ctx: &egui::Context, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        for entry in &mut self.entries {
            if !entry.visible || entry.dock != DockPosition::Floating { continue }
            let title = entry.window.title().to_string();
            let mut out = Vec::new();
            egui::Window::new(&title)
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    out = entry.window.ui(ui, editor, theme);
                });
            actions.append(&mut out);
        }
        actions
    }
}
