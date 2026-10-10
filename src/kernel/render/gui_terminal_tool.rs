//! The terminal dock — one tab per PTY session, the ANSI screen rendered as
//! styled runs, click-to-focus, and a PTY resize that follows the panel.

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, RichText, Sense};
use crate::kernel::state::Editor;
use crate::kernel::terminal::screen::{Color as TermColor, StyledLine};

use super::gui_theme::{self, GuiTheme};
use super::tool_window::ToolAction;

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

/// The dedicated terminal surface: one tab per session, screen rendering,
/// click-to-focus, and a PTY resize that follows the panel size.
pub struct TerminalToolWindow {
    last_resize: Option<(usize, u16, u16)>,
    /// Flattened lines cached per (session, screen revision, row count) so
    /// idle frames (cursor blink, mouse move) don't re-shape the grid.
    cache: Option<(usize, u64, usize, Vec<StyledLine>)>,
}

impl TerminalToolWindow {
    pub fn new() -> Self { Self { last_resize: None, cache: None } }
}

impl super::tool_window::ToolWindow for TerminalToolWindow {
    fn title(&self) -> &str  { "Terminal" }
    fn icon(&self)  -> char  { gui_theme::icons::TERMINAL.chars().next().unwrap_or('⊟') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        let manager = &editor.io.terminals;

        // ── Session tab strip: [/bin/zsh ×] [+] ─────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            for session in &manager.sessions {
                let is_active = manager.active_id() == Some(session.id);
                let label = RichText::new(&session.title)
                    .font(prop(12.0))
                    .color(if is_active { theme.text } else { theme.overlay0 });
                let tab = egui::Button::new(label)
                    .fill(if is_active { theme.surface0 } else { theme.mantle })
                    .corner_radius(egui::CornerRadius::ZERO);
                if ui.add(tab).clicked() {
                    actions.push(ToolAction::TerminalFocus(session.id));
                }
                if ui.add(
                    egui::Button::new(
                        RichText::new(gui_theme::icons::CLOSE)
                            .font(prop(11.0))
                            .color(theme.overlay0),
                    )
                    .frame(false),
                ).clicked() {
                    actions.push(ToolAction::TerminalClose(session.id));
                }
            }
            if ui.add(
                egui::Button::new(RichText::new("+").font(prop(13.0)).color(theme.green))
                    .frame(false)
            ).on_hover_text("New terminal").clicked() {
                actions.push(ToolAction::TerminalNew);
            }
        });
        ui.separator();

        // ── Screen of the active session ────────────────────────────────
        let Some(active_id) = manager.active_id() else {
            ui.add_space(8.0);
            ui.label(
                RichText::new("No terminal running.  Press + or run :terminal to start one.")
                    .font(prop(13.0))
                    .color(theme.overlay0),
            );
            return actions;
        };
        let Some(session) = manager.get(active_id) else { return actions };
        let screen = &session.screen;

        let font_id = mono(13.0);
        let row_h = ui.fonts_mut(|f| f.row_height(&font_id));
        let char_w = ui.fonts_mut(|f| f.glyph_width(&font_id, 'M')).max(1.0);
        let avail = ui.available_size();

        // Reserve one hint line under the screen so it stays above the fold.
        let (body_rect, body_resp) = ui.allocate_exact_size(
            egui::vec2(avail.x, (avail.y - row_h - 8.0).max(row_h * 4.0)),
            Sense::click(),
        );
        if body_resp.clicked() {
            actions.push(ToolAction::TerminalFocus(active_id));
        }

        let rows_that_fit = (((body_rect.height() - 8.0) / row_h).floor() as usize).max(1);
        let revision = screen.revision;
        let rows = match &self.cache {
            Some((id, rev, n, lines))
                if *id == active_id && *rev == revision && *n == rows_that_fit => lines.clone(),
            _ => {
                let lines = screen.tail_styled(rows_that_fit);
                self.cache = Some((active_id, revision, rows_that_fit, lines.clone()));
                lines
            }
        };

        // ANSI palette → chrome colors (Catppuccin-aligned).
        let palette: [Color32; 16] = [
            theme.crust, egui::Color32::from_rgb(243, 139, 168), egui::Color32::from_rgb(166, 227, 161),
            egui::Color32::from_rgb(249, 226, 175), egui::Color32::from_rgb(137, 180, 250),
            egui::Color32::from_rgb(203, 166, 247), egui::Color32::from_rgb(148, 226, 213),
            theme.overlay0, theme.surface1, egui::Color32::from_rgb(243, 139, 168),
            egui::Color32::from_rgb(166, 227, 161), egui::Color32::from_rgb(249, 226, 175),
            egui::Color32::from_rgb(137, 180, 250), egui::Color32::from_rgb(203, 166, 247),
            egui::Color32::from_rgb(148, 226, 213), theme.text,
        ];
        let color_of = |c: TermColor| -> Color32 {
            match c {
                TermColor::Default => theme.text,
                TermColor::Indexed(i) => palette[(i as usize).min(15)],
                TermColor::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
            }
        };
        for (i, line) in rows.iter().enumerate() {
            let y = body_rect.top() + 4.0 + i as f32 * row_h;
            for span in &line.spans {
                if span.start as usize >= line.text.len() { break; }
                let end = (span.start as usize + span.len as usize).min(line.text.len());
                let seg = &line.text[span.start as usize..end];
                // No cell backgrounds are painted, so a blank run draws
                // nothing — skip it instead of paying for a galley.
                if seg.chars().all(|c| c == ' ') { continue; }
                let mut fg = color_of(span.style.fg);
                if span.style.reverse {
                    fg = color_of(span.style.bg);
                }
                ui.painter().text(
                    Pos2::new(body_rect.left() + 6.0 + span.start as f32 * char_w, y),
                    Align2::LEFT_TOP,
                    seg,
                    font_id.clone(),
                    fg,
                );
            }
        }

        // The block cursor sits on the session's real cursor cell — not the
        // last visible row — so typing echoes where the block is.
        if manager.focused == Some(active_id) && screen.cursor_visible() {
            let first_visible = screen.line_count().saturating_sub(rows.len());
            let cursor_line = screen.scrollback_len() + screen.cursor_screen_row() as usize;
            if let Some(i) = cursor_line.checked_sub(first_visible)
                && i < rows.len() {
                    let y = body_rect.top() + 4.0 + i as f32 * row_h;
                    let cx = body_rect.left() + 6.0 + screen.cursor_col() as f32 * char_w;
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            Pos2::new(cx, y),
                            egui::vec2(char_w.max(2.0), row_h),
                        ),
                        0.0,
                        theme.green.gamma_multiply(0.55),
                    );
                }
        }

        // Keep the PTY size in sync with the panel so the shell wraps at the
        // right column; only reported when the size actually changed.
        let cols = (((body_rect.width() - 12.0) / char_w).floor() as u16).max(20);
        let new_rows = (rows_that_fit as u16).max(4);
        if self.last_resize != Some((active_id, cols, new_rows)) {
            self.last_resize = Some((active_id, cols, new_rows));
            actions.push(ToolAction::TerminalResize { id: active_id, cols, rows: new_rows });
        }

        if manager.focused != Some(active_id) {
            ui.label(
                RichText::new("Click the terminal to type into it.  Esc returns to the editor.")
                    .font(prop(11.0))
                    .color(theme.overlay0),
            );
        }
        actions
    }
}
