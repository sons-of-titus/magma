//! Tool window framework — docked/floating panels on the left tool strip.
//!
//! The manager owns entries; the built-in windows live in their own files
//! (`gui_find_tool`, `gui_output_tool`).  Windows return `ToolAction`s that
//! `GuiApp` applies — the same actions-as-data contract as dialogs.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_theme::GuiTheme;

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

// ── ToolWindow trait ──────────────────────────────────────────────────────────

/// A navigation request from a tool window row (quickfix hit, search match).
pub enum ToolAction {
    Goto { path: String, line: usize },
}

pub trait ToolWindow: Send {
    fn title(&self) -> &str;
    fn icon(&self)  -> char;
    /// Render the tool window's content; returns navigation requests.
    fn ui(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Vec<ToolAction>;
}

// ── Dock position ─────────────────────────────────────────────────────────────

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
                    window:  Box::new(TerminalToolWindow::new()),
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
            ui.add_space(6.0);
            for (i, entry) in self.entries.iter().enumerate() {
                let icon = entry.window.icon().to_string();
                let active = entry.visible;
                let color = if active { theme.blue } else { theme.overlay0 };
                let btn = egui::Button::new(
                    RichText::new(icon).font(mono(16.0)).color(color)
                )
                .frame(false)
                .min_size(egui::vec2(28.0, 28.0));
                let resp = ui.add(btn).on_hover_text(entry.window.title());
                if resp.clicked() {
                    toggled = Some(i);
                }
                ui.add_space(2.0);
            }
        });
        toggled
    }

    /// Render docked-bottom tool windows as a resizable bottom panel.
    ///
    /// Returns navigation actions from the active window.
    pub fn show_docked_bottom(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Vec<ToolAction> {
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
                // Tab strip
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    for &idx in &visible {
                        let title = self.entries[idx].window.title();
                        let is_active = idx == self.active_bottom;
                        let bg = if is_active { theme.surface0 } else { theme.mantle };
                        let fg = if is_active { theme.text } else { theme.overlay0 };
                        let btn = egui::Button::new(
                            RichText::new(title).font(prop(12.0)).color(fg)
                        )
                        .fill(bg)
                        .corner_radius(egui::CornerRadius::ZERO);
                        if ui.add(btn).clicked() {
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
    pub fn show_floating(&mut self, ctx: &egui::Context, editor: &Editor, theme: &GuiTheme) -> Vec<ToolAction> {
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

// ── TerminalToolWindow ────────────────────────────────────────────────────────

pub struct TerminalToolWindow {
    input: String,
}

impl TerminalToolWindow {
    pub fn new() -> Self { Self { input: String::new() } }
}

impl ToolWindow for TerminalToolWindow {
    fn title(&self) -> &str  { "Terminal" }
    fn icon(&self)  -> char  { super::gui_theme::icons::TERMINAL.chars().next().unwrap_or('⊟') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        // Show the first active terminal buffer, if any.
        let term_buf = editor.io.terminals.keys().copied().next();
        let Some(buf_id) = term_buf else {
            ui.label(
                RichText::new("No terminal running.  Use :terminal to start one.")
                    .font(prop(13.0))
                    .color(theme.overlay0),
            );
            return Vec::new();
        };
        if let Some(arc) = editor.buffers.get(buf_id) {
            let text = {
                let buf = arc.lock().unwrap();
                buf.slice(0, buf.len())
            };
            ui.add(
                egui::Label::new(
                    RichText::new(text).font(mono(13.0)).color(theme.green)
                )
                .wrap_mode(egui::TextWrapMode::Extend)
            );
        }

        // Input row — the docked terminal accepts lines, so the edit → run
        // → observe loop works without leaving the panel.
        let active = editor.io.terminals.contains_key(&buf_id);
        if active {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .hint_text("Send to terminal… (Enter)")
                    .font(mono(13.0))
                    .desired_width(f32::INFINITY),
            );
            let send = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if send && !self.input.is_empty() {
                let line = std::mem::take(&mut self.input);
                let _ = crate::kernel::terminal::send_input(editor, buf_id, &format!("{line}\n"));
            }
            if send {
                resp.request_focus();
            }
        }
        Vec::new()
    }
}
