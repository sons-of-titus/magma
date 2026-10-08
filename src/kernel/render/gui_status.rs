//! Native status bar widget for the GUI — Phase 3.
//!
//! Defines `StatusWidget` trait and concrete per-slot widgets, then assembles
//! them into `StatusBar::show` with left-aligned (mode, filename) and
//! right-aligned (encoding, file type, position, git branch) sections.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
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

        let (row, col) = {
            let buf = arc.lock().unwrap();
            let offset = editor.view_of_buffer(buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
            let pos = buf.position_of(offset);
            (pos.line + 1, pos.column + 1)
        };

        ui.label(
            RichText::new(format!("Ln {}, Col {}", row, col))
                .color(theme.text)
                .font(label_font()),
        );
    }
}

/// What the user clicked in the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusAction {
    OpenVc,
}

/// Git branch from the last VC status fetch.  Hidden when no branch is
/// known; clicking opens the VC status buffer.
pub struct GitBranchWidget;

impl GitBranchWidget {
    fn render_clickable(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> bool {
        let Some(status) = &editor.vc.last_status else { return false };
        if status.branch.is_empty() { return false; }
        let btn = egui::Button::new(
            RichText::new(format!("{} {}", icons::GIT_BRANCH, status.branch))
                .color(theme.blue)
                .font(label_font()),
        )
        .frame(false);
        ui.add(btn).clicked()
    }
}

impl StatusWidget for GitBranchWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        self.render_clickable(ui, editor, theme);
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

/// Transient feedback line (errors, confirmations).  Rendered for a few
/// seconds after `editor.show_message`; `*Messages*` keeps the durable log.
pub struct MessageWidget;

impl MessageWidget {
    /// Messages stop rendering after this long; the next frame drops them.
    const LINGER: std::time::Duration = std::time::Duration::from_secs(6);

    fn fresh(editor: &Editor) -> bool {
        editor.status_message.as_ref().is_some_and(|m| m.at.elapsed() <= Self::LINGER)
    }

    pub fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        let Some(msg) = &editor.status_message else { return };
        if msg.at.elapsed() > Self::LINGER { return; }
        let color = match msg.kind {
            crate::kernel::state::MessageKind::Info => theme.green,
            crate::kernel::state::MessageKind::Error => theme.red,
        };
        ui.label(
            RichText::new(msg.text.clone())
                .color(color)
                .font(label_font())
                .strong(),
        );
    }
}

// ── StatusBar ─────────────────────────────────────────────────────────────────

/// Assembles all `StatusWidget`s into the bottom panel.
///
/// Left side:  mode pill → filename
/// Right side: encoding → file type → cursor position → git branch
pub struct StatusBar;

impl StatusBar {
    /// Assemble the bar; returns an action when a clickable widget was used.
    pub fn show(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Option<StatusAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // ── Left-aligned ──────────────────────────────────────────────
            FilenameWidget.render(ui, editor, theme);
            if MessageWidget::fresh(editor) {
                sep(ui, theme);
                MessageWidget.render(ui, editor, theme);
            }

            // ── Right-aligned (order is reversed due to right_to_left) ───
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if GitBranchWidget.render_clickable(ui, editor, theme) {
                    action = Some(StatusAction::OpenVc);
                }
                sep(ui, theme);
                PositionWidget.render(ui, editor, theme);
                sep(ui, theme);
                FileTypeWidget.render(ui, editor, theme);
                sep(ui, theme);
                EncodingWidget.render(ui, editor, theme);
            });
        });
        action
    }
}

/// Dim vertical separator between adjacent widgets.
fn sep(ui: &mut egui::Ui, theme: &GuiTheme) {
    ui.label(RichText::new("|").color(theme.surface0).font(label_font()));
}
