//! Tool windows + floating panels — Phase 5.
//!
//! Defines the `ToolWindow` trait, `ToolWindowManager`, and built-in windows:
//!  • `TerminalToolWindow`  — live PTY output viewer
//!  • `FindInFilesToolWindow` — in-buffer text search with results list
//!
//! The manager is stored on `GuiApp` and called from `update()` to render the
//! vertical icon strip (sidebar edge), docked-bottom panels, and floating egui
//! windows.

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;

// ── Catppuccin Mocha colours (shared with other gui_* modules) ───────────────
const TOOL_BG:     Color32 = Color32::from_rgb(24,  24,  37);  // mantle
const TOOL_FG:     Color32 = Color32::from_rgb(205, 214, 244); // text
const TOOL_SEL:    Color32 = Color32::from_rgb(49,  50,  68);  // surface0
const TERM_GREEN:  Color32 = Color32::from_rgb(166, 227, 161); // green
const STRIP_ICON:  Color32 = Color32::from_rgb(108, 112, 134); // overlay0
const STRIP_ACT:   Color32 = Color32::from_rgb(137, 180, 250); // blue

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

// ── ToolWindow trait ──────────────────────────────────────────────────────────

pub trait ToolWindow: Send {
    fn title(&self) -> &str;
    fn icon(&self)  -> char;
    /// Render the tool window's content into `ui`.
    fn ui(&mut self, ui: &mut egui::Ui, editor: &Editor);
}

// ── Dock position ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Left/Right reserved for future docking directions
pub enum DockPosition {
    Bottom,
    Left,
    Right,
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
                    window:  Box::new(FindInFilesToolWindow::new()),
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
    pub fn show_sidebar_edge(&self, ui: &mut egui::Ui) -> Option<usize> {
        let mut toggled = None;
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            for (i, entry) in self.entries.iter().enumerate() {
                let icon = entry.window.icon().to_string();
                let active = entry.visible;
                let color = if active { STRIP_ACT } else { STRIP_ICON };
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
    /// Returns without rendering if no bottom-docked window is visible.
    pub fn show_docked_bottom(&mut self, ctx: &egui::Context, editor: &Editor) {
        let visible: Vec<usize> = self.entries.iter().enumerate()
            .filter(|(_, e)| e.visible && e.dock == DockPosition::Bottom)
            .map(|(i, _)| i)
            .collect();
        if visible.is_empty() { return }

        egui::TopBottomPanel::bottom("magma_tools_bottom")
            .resizable(true)
            .default_height(200.0)
            .frame(egui::Frame::none().fill(TOOL_BG).inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| {
                // Tab strip
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    for &idx in &visible {
                        let title = self.entries[idx].window.title();
                        let is_active = idx == self.active_bottom;
                        let bg = if is_active { TOOL_SEL } else { TOOL_BG };
                        let fg = if is_active { TOOL_FG } else { STRIP_ICON };
                        let btn = egui::Button::new(
                            RichText::new(title).font(prop(12.0)).color(fg)
                        )
                        .fill(bg)
                        .rounding(egui::Rounding::ZERO);
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
                            self.entries[active_idx].window.ui(ui, editor);
                        });
                }
            });
    }

    /// Render floating tool windows as independent egui Windows.
    pub fn show_floating(&mut self, ctx: &egui::Context, editor: &Editor) {
        for entry in &mut self.entries {
            if !entry.visible || entry.dock != DockPosition::Floating { continue }
            let title = entry.window.title().to_string();
            egui::Window::new(&title)
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    entry.window.ui(ui, editor);
                });
        }
    }
}

// ── TerminalToolWindow ────────────────────────────────────────────────────────

pub struct TerminalToolWindow;

impl TerminalToolWindow {
    pub fn new() -> Self { Self }
}

impl ToolWindow for TerminalToolWindow {
    fn title(&self) -> &str  { "Terminal" }
    fn icon(&self)  -> char  { '⊟' }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &Editor) {
        // Show the first active terminal buffer, if any.
        let term_buf = editor.io.terminals.keys().copied().next();
        if let Some(buf_id) = term_buf {
            if let Some(arc) = editor.buffers.get(buf_id) {
                let text = {
                    let buf = arc.lock().unwrap();
                    buf.slice(0, buf.len())
                };
                ui.add(
                    egui::Label::new(
                        RichText::new(text).font(mono(13.0)).color(TERM_GREEN)
                    )
                    .wrap_mode(egui::TextWrapMode::Extend)
                );
                return;
            }
        }
        ui.label(
            RichText::new("No terminal running.  Use :terminal to start one.")
                .font(prop(13.0))
                .color(STRIP_ICON),
        );
    }
}

// ── FindInFilesToolWindow ─────────────────────────────────────────────────────

pub struct FindInFilesToolWindow {
    query:      String,
    results:    Vec<(String, usize, String)>, // (buf_name, line, text)
    last_query: String,
}

impl FindInFilesToolWindow {
    pub fn new() -> Self {
        Self { query: String::new(), results: Vec::new(), last_query: String::new() }
    }

    fn search(&mut self, editor: &Editor) {
        self.results.clear();
        if self.query.is_empty() { return }
        'outer: for (_, arc) in editor.buffers.iter() {
            let buf  = arc.lock().unwrap();
            let name = buf.name.clone();
            let text = buf.slice(0, buf.len());
            for (line_idx, line) in text.lines().enumerate() {
                if line.contains(self.query.as_str()) {
                    self.results.push((name.clone(), line_idx + 1, line.trim_end().to_string()));
                    if self.results.len() >= 200 { break 'outer }
                }
            }
        }
    }
}

impl ToolWindow for FindInFilesToolWindow {
    fn title(&self) -> &str { "Find in Files" }
    fn icon(&self)  -> char { '⌕' }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &Editor) {
        // Search bar
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Search…")
                .font(mono(13.0))
                .desired_width(f32::INFINITY),
        );
        if resp.changed() && self.query != self.last_query {
            self.last_query = self.query.clone();
            self.search(editor);
        }
        ui.separator();

        // Results
        let count = self.results.len();
        if count == 0 && !self.query.is_empty() {
            ui.label(RichText::new("No results.").font(prop(12.0)).color(STRIP_ICON));
            return;
        }
        if count >= 200 {
            ui.label(RichText::new("Showing first 200 matches.").font(prop(11.0)).color(STRIP_ICON));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (buf_name, line_num, line_text) in &self.results {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(buf_name).font(prop(11.0)).color(STRIP_ACT));
                    ui.label(RichText::new(format!(":{}", line_num)).font(prop(11.0)).color(STRIP_ICON));
                    ui.label(RichText::new(line_text).font(mono(12.0)).color(TOOL_FG));
                });
            }
        });
    }
}
