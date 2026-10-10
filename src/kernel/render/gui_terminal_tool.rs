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
    /// Flattened lines cached per (session, screen revision, row count,
    /// first visible line) so idle frames don't re-shape the grid.
    cache: Option<(usize, u64, usize, usize, Vec<StyledLine>)>,
    /// Session being renamed inline, its edit buffer, and a flag to grab
    /// focus on the first frame of the edit.
    renaming: Option<usize>,
    rename_buf: String,
    rename_focus: bool,
    /// Absolute first visible line while scrolled back through history;
    /// `None` follows the live screen.  New output never moves a scrolled
    /// view — scrollback line indices are stable, so the anchor does too.
    scroll_anchor: Option<usize>,
    /// Last `input_seq` seen from the manager; a change means the user sent
    /// stdin, which snaps a scrolled view back to the live screen.
    last_input_seq: u64,
}

impl TerminalToolWindow {
    pub fn new() -> Self {
        Self {
            last_resize: None,
            cache: None,
            renaming: None,
            rename_buf: String::new(),
            rename_focus: false,
            scroll_anchor: None,
            last_input_seq: 0,
        }
    }
}

impl super::tool_window::ToolWindow for TerminalToolWindow {
    fn title(&self) -> &str  { "Terminal" }
    fn icon(&self)  -> char  { gui_theme::icons::TERMINAL.chars().next().unwrap_or('⊟') }

    fn ui(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        let manager = &editor.io.terminals;
        let input_seq = manager.input_seq.load(std::sync::atomic::Ordering::Relaxed);
        if input_seq != self.last_input_seq {
            self.last_input_seq = input_seq;
            self.scroll_anchor = None;
        }

        // ── Session tab strip: [/bin/zsh ×] [+] ─────────────────────────
        // Horizontally scrollable so a pile of sessions never squeezes the
        // row; drag or trackpad-swipe to reach the overflow.  The egui bar
        // idles at 2px and fattens to 10px on hover — pin both to 2px.
        let scroll = &mut ui.spacing_mut().scroll;
        scroll.bar_width = 2.0;
        scroll.floating_width = 2.0;
        egui::ScrollArea::horizontal()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    for session in &manager.sessions {
                        let is_active = manager.active_id() == Some(session.id);
                        if self.renaming == Some(session.id) {
                            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.rename_buf)
                                    .desired_width(110.0)
                                    .font(prop(12.0)),
                            );
                            if self.rename_focus {
                                edit.request_focus();
                                self.rename_focus = false;
                            }
                            if edit.lost_focus() || esc {
                                let name = self.rename_buf.trim();
                                if !esc && !name.is_empty() {
                                    actions.push(ToolAction::TerminalRename {
                                        id: session.id,
                                        name: name.to_string(),
                                    });
                                }
                                self.renaming = None;
                            }
                        } else {
                            let label = RichText::new(&session.title)
                                .font(prop(12.0))
                                .color(if is_active { theme.text } else { theme.overlay0 });
                            let tab = egui::Button::new(label)
                                .fill(if is_active { theme.surface0 } else { theme.mantle })
                                .corner_radius(egui::CornerRadius::ZERO);
                            let resp = ui.add(tab);
                            if resp.clicked() {
                                actions.push(ToolAction::TerminalFocus(session.id));
                            }
                            if resp.double_clicked() {
                                self.rename_buf = session.title.clone();
                                self.renaming = Some(session.id);
                                self.rename_focus = true;
                            }
                            resp.on_hover_text("Click to focus · double-click to rename");
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
            self.scroll_anchor = None;
        }

        let rows_that_fit = (((body_rect.height() - 8.0) / row_h).floor() as usize).max(1);
        let total = screen.line_count();
        let max_first = total.saturating_sub(rows_that_fit);
        if body_resp.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                let base = self.scroll_anchor.unwrap_or(max_first);
                let lines = (dy / row_h).trunc() as isize;
                let next = (base as isize - lines).clamp(0, max_first as isize) as usize;
                self.scroll_anchor = (next < max_first).then_some(next);
            }
        }
        if self.scroll_anchor.is_some_and(|f| f >= max_first) {
            self.scroll_anchor = None;
        }
        let first_visible = self.scroll_anchor.unwrap_or(max_first);

        let revision = screen.revision;
        let rows = match &self.cache {
            Some((id, rev, n, first, lines))
                if *id == active_id
                    && *rev == revision
                    && *n == rows_that_fit
                    && *first == first_visible =>
                lines.clone(),
            _ => {
                let end = (first_visible + rows_that_fit).min(total);
                let lines: Vec<StyledLine> =
                    (first_visible..end).map(|i| screen.styled_line(i)).collect();
                self.cache =
                    Some((active_id, revision, rows_that_fit, first_visible, lines.clone()));
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
        // last visible row — so typing echoes where the block is.  While
        // scrolled back, the cursor only shows if its line is on screen.
        if manager.focused == Some(active_id) && screen.cursor_visible() {
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
