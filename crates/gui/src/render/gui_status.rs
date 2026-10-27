//! Native status bar widget for the GUI — Phase 3.
//!
//! Defines `StatusWidget` trait and concrete per-slot widgets, then assembles
//! them into `StatusBar::show` with left-aligned (filename) and right-aligned
//! (encoding, file type, position, git branch) sections.  Every slot renders
//! as a JetBrains-style pill: quiet text that highlights on hover.

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, RichText, Sense, Vec2};
use magma::kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};

const FONT_SIZE: f32 = 12.0;

fn label_font() -> FontId { FontId::new(FONT_SIZE, FontFamily::Proportional) }

/// A status-bar pill: quiet text (with optional leading icon) in a rounded
/// slot that highlights on hover.  Returns true when clicked.
fn pill(
    ui: &mut egui::Ui,
    theme: &GuiTheme,
    icon: Option<(&str, Color32)>,
    text: &str,
    color: Color32,
) -> bool {
    let font = label_font();
    let mut w = ui.painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color)
        .size().x + 12.0;
    if let Some((glyph, _)) = icon {
        w += ui.painter()
            .layout_no_wrap(glyph.to_owned(), font.clone(), color)
            .size().x + 4.0;
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4, theme.surface0.gamma_multiply(0.55));
    }
    let mut cx = rect.left() + 6.0;
    if let Some((glyph, icon_color)) = icon {
        ui.painter().text(
            Pos2::new(cx, rect.center().y),
            Align2::LEFT_CENTER,
            glyph,
            font.clone(),
            icon_color,
        );
        cx += ui.painter()
            .layout_no_wrap(glyph.to_owned(), font.clone(), color)
            .size().x + 4.0;
    }
    ui.painter().text(
        Pos2::new(cx, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        color,
    );
    resp.clicked()
}

// ── StatusWidget trait ────────────────────────────────────────────────────────

/// A pluggable status bar slot.  Any subsystem can contribute one by
/// implementing this trait and adding it to `StatusBar::show`.
pub trait StatusWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme);
}

// ── Concrete widgets ──────────────────────────────────────────────────────────

/// Focused buffer name; dimmed when unmodified, accented when dirty.
pub struct FilenameWidget;

impl StatusWidget for FilenameWidget {
    fn render(&self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) {
        let Some(wid)    = editor.view_tree.focused_window() else { return };
        let Some(buf_id) = editor.view_tree.buffer(wid)      else { return };
        let Some(arc)    = editor.buffers.get(buf_id)         else { return };

        let (name, modified) = {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            (buf.name.clone(), buf.modified())
        };
        pill(ui, theme, None, &name, if modified { theme.text } else { theme.subtext0 });
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
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let offset = editor.view_of_buffer(buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
            let pos = buf.position_of(offset);
            (pos.line + 1, pos.column + 1)
        };

        pill(ui, theme, None, &format!("Ln {}, Col {}", row, col), theme.subtext0);
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
        pill(ui, theme, Some((icons::GIT_BRANCH, theme.blue)), &status.branch, theme.subtext0)
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

        let mode = arc.lock().unwrap_or_else(|e| e.into_inner()).major_mode.name().to_string();
        pill(ui, theme, None, &mode, theme.overlay0);
    }
}

/// File encoding — "UTF-8" (constant; extensible later).
pub struct EncodingWidget;

impl StatusWidget for EncodingWidget {
    fn render(&self, ui: &mut egui::Ui, _editor: &Editor, theme: &GuiTheme) {
        pill(ui, theme, None, "UTF-8", theme.overlay0);
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
            magma::kernel::state::MessageKind::Info => theme.green,
            magma::kernel::state::MessageKind::Error => theme.red,
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
/// Left side:  filename (+ transient message)
/// Right side: encoding → file type → cursor position → git branch
pub struct StatusBar;

impl StatusBar {
    /// Assemble the bar; returns an action when a clickable widget was used.
    pub fn show(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Option<StatusAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.set_min_height(22.0);

            // ── Left-aligned ──────────────────────────────────────────────
            FilenameWidget.render(ui, editor, theme);
            if MessageWidget::fresh(editor) {
                MessageWidget.render(ui, editor, theme);
            }

            // ── Right-aligned (order is reversed due to right_to_left) ───
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if GitBranchWidget.render_clickable(ui, editor, theme) {
                    action = Some(StatusAction::OpenVc);
                }
                PositionWidget.render(ui, editor, theme);
                FileTypeWidget.render(ui, editor, theme);
                EncodingWidget.render(ui, editor, theme);
            });
        });
        action
    }
}
