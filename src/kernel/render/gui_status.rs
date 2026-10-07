//! Native status bar widget for the GUI — Phase 3.
//!
//! Defines `StatusWidget` trait and concrete per-slot widgets, then assembles
//! them into `StatusBar::show` with left-aligned (mode, filename) and
//! right-aligned (encoding, file type, position, git branch) sections.

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_fonts::mode_color;
use super::gui_render::safe_boundary;
use super::gui_theme::{GuiTheme, icons};

const FONT_SIZE: f32 = 12.0;

fn label_font() -> FontId { FontId::new(FONT_SIZE, FontFamily::Proportional) }

// ── StatusWidget trait ────────────────────────────────────────────────────────

/// A pluggable status bar slot.  Any subsystem can contribute one by
/// implementing this trait and adding it to `StatusBar::show`.
pub trait StatusWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme);
}

// ── Concrete widgets ──────────────────────────────────────────────────────────

/// Colored pill showing the current vim mode (NORMAL / INSERT / VISUAL …).
pub struct ModeWidget;

impl StatusWidget for ModeWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, _theme: &GuiTheme) {
        let mode = editor.vim_mode_name();
        let pill_color = mode_color(&mode);
        ui.label(
            RichText::new(format!(" {} ", mode))
                .background_color(pill_color)
                .color(Color32::WHITE)
                .font(label_font())
                .strong(),
        );
    }
}

/// Buffer name with "[+]" when the buffer has unsaved changes.
pub struct FilenameWidget;

impl StatusWidget for FilenameWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        let Some(wid)    = editor.view_tree.focused_window() else { return };
        let Some(buf_id) = editor.view_tree.buffer(wid)      else { return };
        let Some(arc)    = editor.buffers.get(buf_id)         else { return };

        let (name, modified) = {
            let buf = arc.lock().unwrap();
            (buf.name.clone(), buf.modified())
        };
        let text = if modified {
            format!("{} [+]", name)
        } else {
            name
        };
        ui.label(RichText::new(text).color(theme.text).font(label_font()));
    }
}

/// Cursor position in "Ln N, Col M" format.
pub struct PositionWidget;

impl StatusWidget for PositionWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        let Some(wid)    = editor.view_tree.focused_window() else { return };
        let Some(buf_id) = editor.view_tree.buffer(wid)      else { return };
        let Some(arc)    = editor.buffers.get(buf_id)         else { return };

        let (content, cursor_offset) = {
            let buf = arc.lock().unwrap();
            let content = buf.slice(0, buf.len());
            let offset  = editor.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
            (content, offset)
        };
        let safe   = safe_boundary(&content, cursor_offset);
        let before = &content[..safe];
        let row    = before.chars().filter(|&c| c == '\n').count() + 1;
        let last   = before.rfind('\n').map(|p| &before[p + 1..]).unwrap_or(before);
        let col    = last.chars().count() + 1;

        ui.label(
            RichText::new(format!("Ln {}, Col {}", row, col))
                .color(theme.text)
                .font(label_font()),
        );
    }
}

/// Git branch from the last VC status fetch.  Hidden when no branch is known.
pub struct GitBranchWidget;

impl StatusWidget for GitBranchWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        let Some(status) = &editor.vc.last_status else { return };
        if status.branch.is_empty() { return; }
        ui.label(
            RichText::new(format!("{} {}", icons::GIT_BRANCH, status.branch))
                .color(theme.blue)
                .font(label_font()),
        );
    }
}

/// Buffer's major mode name (e.g. "fundamental", "janet", "rust").
pub struct FileTypeWidget;

impl StatusWidget for FileTypeWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        let Some(wid)    = editor.view_tree.focused_window() else { return };
        let Some(buf_id) = editor.view_tree.buffer(wid)      else { return };
        let Some(arc)    = editor.buffers.get(buf_id)         else { return };

        let mode = arc.lock().unwrap().major_mode.name().to_string();
        ui.label(RichText::new(mode).color(theme.overlay0).font(label_font()));
    }
}

/// File encoding — "UTF-8" (constant; extensible later).
pub struct EncodingWidget;

impl StatusWidget for EncodingWidget {
    fn render(&self, ui: &mut egui::Ui, _editor: &Editor, theme: &GuiTheme) {
        ui.label(RichText::new("UTF-8").color(theme.overlay0).font(label_font()));
    }
}

// ── StatusBar ─────────────────────────────────────────────────────────────────

/// Assembles all `StatusWidget`s into the bottom panel.
///
/// Left side:  mode pill → filename
/// Right side: encoding → file type → cursor position → git branch
pub struct StatusBar;

impl StatusBar {
    pub fn show(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // ── Left-aligned ──────────────────────────────────────────────
            ModeWidget.render(ui, editor, theme);
            sep(ui, theme);
            FilenameWidget.render(ui, editor, theme);

            // ── Right-aligned (order is reversed due to right_to_left) ───
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                GitBranchWidget.render(ui, editor, theme);
                sep(ui, theme);
                PositionWidget.render(ui, editor, theme);
                sep(ui, theme);
                FileTypeWidget.render(ui, editor, theme);
                sep(ui, theme);
                EncodingWidget.render(ui, editor, theme);
            });
        });
    }
}

/// Dim vertical separator between adjacent widgets.
fn sep(ui: &mut egui::Ui, theme: &GuiTheme) {
    ui.label(RichText::new("|").color(theme.surface0).font(label_font()));
}
