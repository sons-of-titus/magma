//! Central editor pane — the Surface-based editing area plus its scrollbar
//! and minimap strip.
//!
//! The pane shrinks the character grid to leave room for a right-hand strip:
//! a classic scrollbar (default) or the code-overview minimap
//! (`minimap-enabled` option).  Mouse events that land inside the strip are
//! consumed by the strip; only editor-rect events reach `dispatch_mouse`.

use eframe::egui::{self, Align2, FontFamily, FontId, Pos2, Rect, Vec2};
use eframe::wgpu::TextureFormat;

use kernel::input::event::MouseEvent;
use kernel::input::mouse;
use crate::render::frame::{pane_scroll_state, render_frame_with, sync_viewport_scroll};
use crate::render::CursorPaint;
use crate::render::gpu_atlas::{build_glyph_instances, build_rect_instances, GpuGlyphAtlas};
use crate::render::gpu_paint_callback::MagmaPaintCallback;
use crate::render::surface::Surface;
use kernel::state::Editor;
use super::gui_input::RawMouseEvent;
use super::gui_minimap::{Minimap, STRIP_W};
use super::gui_render::present;
use super::gui_theme::GuiTheme;

const SCROLLBAR_W: f32 = 10.0;
const THUMB_MIN_H: f32 = 18.0;
/// Height of the bottom horizontal scrollbar strip.
const H_BAR_H: f32 = 8.0;

/// Everything the pane needs to paint one frame.
pub struct PanePaint<'a> {
    pub editor:        &'a mut Editor,
    /// Scratch character grid, reused across frames (re-allocated on resize).
    pub surface:       &'a mut Surface,
    pub atlas:         &'a GpuGlyphAtlas,
    pub use_gpu:       bool,
    pub target_format: Option<TextureFormat>,
    /// Atlas pixels to re-upload this frame, if dirty.
    pub atlas_pixels:  Option<Vec<u8>>,
    pub theme:         &'a GuiTheme,
    /// IME composition text drawn inline at the cursor (R-15); `None`
    /// while no composition is active.
    pub preedit:       Option<PreeditOverlay>,
    /// Caret blink phase owned by the shell's heartbeat.
    pub cursor_visible: bool,
}

/// Composed-but-uncommitted IME text.
#[derive(Debug, Clone)]
pub struct PreeditOverlay {
    pub text: String,
    /// Composition cursor as a char offset into `text`, when the IME
    /// reports one.  Reserved for inline composition-caret rendering; the
    /// overlay currently paints the full composition at the text cursor.
    #[allow(dead_code)]
    pub cursor_char: Option<usize>,
}

/// Action produced by pane chrome, applied by the caller after the frame.
pub enum EditorPaneAction {
    /// Scroll the focused pane's viewport so `line` (1-based) is the top row.
    /// Viewport-only: the cursor stays where it is.
    ScrollViewport(usize),
    /// Horizontal pan: make `col` the pane's first visible content column.
    ScrollLeft(usize),
}

/// Render the central editor pane.  Returns a chrome-initiated action.
pub fn render_editor_pane(
    ui:       &mut egui::Ui,
    paint:    &mut PanePaint<'_>,
    char_w:   f32,
    line_h:   f32,
    raw:      &mut Vec<RawMouseEvent>,
) -> Option<EditorPaneAction> {
    let full   = ui.available_rect_before_wrap();
    let minimap = minimap_enabled(paint.editor);

    let strip_w = if minimap { STRIP_W } else { SCROLLBAR_W };
    let mut editor_rect = Rect::from_min_max(
        full.min,
        egui::pos2(full.right() - strip_w, full.bottom()),
    );
    let strip_rect = Rect::from_min_max(
        egui::pos2(full.right() - strip_w, full.top()),
        full.max,
    );

    let visible_cols = ((editor_rect.width()  / char_w).floor() as usize).max(1);

    // ── Horizontal scrollbar reservation (focused pane) ─────────────────
    // A bottom strip appears only when the visible content is wider than the
    // text area; text rows shrink so the bar never overlaps the grid.
    let h_focused = paint.editor.view_tree.focused_window()
        .and_then(|wid| paint.editor.view_tree.window(wid).cloned());
    let h_buf = h_focused.as_ref().and_then(|p| p.buffer_id);
    let h_geom = h_buf.and_then(|buf_id| {
        let total_lines = paint.editor.buffers.get(buf_id)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
            .unwrap_or(1);
        let gutter = crate::render::frame::gutter_cols(paint.editor, buf_id, total_lines);
        let content_cols = visible_cols.saturating_sub(gutter);
        let extent = crate::render::frame::horizontal_extent(paint.editor, buf_id);
        (content_cols > 0 && extent > content_cols).then_some((content_cols, extent))
    });
    let h_bar_h = if h_geom.is_some() { H_BAR_H } else { 0.0 };
    let h_bar_rect = Rect::from_min_max(
        egui::pos2(editor_rect.left(), editor_rect.bottom() - h_bar_h),
        editor_rect.max,
    );
    if h_bar_h > 0.0 {
        editor_rect = Rect::from_min_max(
            editor_rect.min,
            egui::pos2(editor_rect.right(), editor_rect.bottom() - h_bar_h),
        );
    }

    let visible_rows = ((editor_rect.height() / line_h).floor() as usize).max(1);
    let ed = &mut *paint.editor;
    ed.view_tree.resize(visible_cols as u16, visible_rows as u16);
    // The surface must match the painted extent so no unpainted cells
    // reach the screen.
    let extent_w = ed.view_tree.panes().iter()
        .map(|p| p.x + p.width).max().unwrap_or(visible_cols as u16);
    let extent_h = ed.view_tree.panes().iter()
        .map(|p| p.y + p.height).max().unwrap_or(visible_rows as u16);
    let surface = &mut *paint.surface;
    surface.resize(extent_w.max(visible_cols as u16), extent_h.max(visible_rows as u16));
    sync_viewport_scroll(ed);
    render_frame_with(ed, surface, CursorPaint::Skip);

    let (r, g, b) = ed.theme_color("gui-bg");
    let bg_color = egui::Color32::from_rgb(r, g, b);

    if paint.use_gpu && let Some(fmt) = paint.target_format {
        let bg = [
            bg_color.r() as f32 / 255.0,
            bg_color.g() as f32 / 255.0,
            bg_color.b() as f32 / 255.0,
            1.0,
        ];
        let glyphs = build_glyph_instances(
            surface, paint.atlas, char_w, line_h,
            editor_rect.min.x, editor_rect.min.y, true,
        );
        let rects = build_rect_instances(
            surface, char_w, line_h,
            editor_rect.min.x, editor_rect.min.y, bg, true,
        );
        let callback = MagmaPaintCallback {
            glyphs,
            rects,
            char_w,
            line_h,
            target_format: fmt,
            atlas_width:   paint.atlas.atlas_width,
            atlas_height:  paint.atlas.atlas_height,
            atlas_pixels:  paint.atlas_pixels.clone(),
        };
        ui.painter_at(editor_rect).add(callback.into_paint_callback(editor_rect));
    } else {
        present(ui, paint.editor, surface, editor_rect, char_w, line_h, 0);
    }

    // ── Native overlays — cursor, completion, hover card, context menu ──
    {
        let focused_pane = paint.editor.view_tree.focused_window()
            .and_then(|wid| paint.editor.view_tree.window(wid).cloned());
        let focused_buf = focused_pane.as_ref().and_then(|p| p.buffer_id);
        if let (Some(pane), Some(buf_id)) = (&focused_pane, focused_buf) {
            let is_focused = paint.editor.view_tree.focused_window() == Some(pane.id);
            let header_rows = paint.editor.buffers.get(buf_id)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).header_line.is_some() as i32)
                .unwrap_or(0) as f32;
            let st = pane_scroll_state(paint.editor, pane, buf_id, visible_rows);
            let scroll_left = crate::render::frame::pane_scroll_left(pane, buf_id);

            // Cursor pixel anchor — shared by the cursor rect and popups.
            let cursor_px = st.cursor_line.checked_sub(st.scroll_top)
                .filter(|&vis_row| vis_row < visible_rows
                    && st.cursor_col >= scroll_left
                    && st.cursor_col - scroll_left < visible_cols)
                .map(|vis_row| {
                    let prefix_margin = crate::render::decorations::prefix_margin_width(paint.editor, buf_id);
                    let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };
                    let total_lines = paint.editor.buffers.get(buf_id)
                        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count()).unwrap_or(1);
                    let gutter_px = ((prefix_cols + paint.editor.gutter.total_width(total_lines)) as f32) * char_w;
                    let x = editor_rect.min.x + gutter_px
                        + (st.cursor_col - scroll_left) as f32 * char_w;
                    let y = editor_rect.min.y + (header_rows + vis_row as f32) * line_h;
                    (x, y)
                });

            if is_focused && paint.cursor_visible
                && let Some((x, y)) = cursor_px {
                let (cr, cg, cb) = paint.editor.theme_color("cursor");
                let color = egui::Color32::from_rgb(cr, cg, cb);
                let rect = match paint.editor.cursor_shape.as_str() {
                    "underline" => Rect::from_min_size(
                        Pos2::new(x, y + line_h - 3.0), Vec2::new(char_w, 3.0)),
                    "block" => Rect::from_min_size(
                        Pos2::new(x, y), Vec2::new(char_w, line_h)),
                    _ => {
                        let w = (char_w * 0.12).max(2.0);
                        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, line_h))
                    }
                };
                ui.painter().rect_filled(rect, 1.0, color);
            }

            // IME preedit — composition renders inline at the cursor with
            // an underline, exactly where the committed text will land.
            if is_focused
                && let Some(pre) = paint.preedit.as_ref()
                && !pre.text.is_empty()
                && let Some((x, y)) = cursor_px {
                    let (fr, fg, fb) = paint.editor.theme_color("fg");
                    let color = egui::Color32::from_rgb(fr, fg, fb);
                    let font = FontId::new(paint.editor.font_config.size, FontFamily::Monospace);
                    ui.painter().text(
                        Pos2::new(x, y + line_h * 0.5),
                        egui::Align2::LEFT_CENTER,
                        &pre.text,
                        font,
                        color,
                    );
                    let text_w = pre.text.chars().count() as f32 * char_w;
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            Pos2::new(x, y + line_h - 2.0),
                            Vec2::new(text_w.max(2.0), 1.5),
                        ),
                        0.5,
                        color,
                    );
            }

            if is_focused {
                draw_completion_popup(ui, paint.editor, paint.theme,
                    cursor_px, editor_rect, char_w, line_h);
                draw_quickfix_menu(ui, paint.editor, paint.theme,
                    cursor_px, editor_rect, line_h);
            }
        }

        update_hover_card(ui, paint.editor, editor_rect, char_w, line_h);
        draw_hover_card(ui, paint.editor, paint.theme);
        draw_context_menu(ui, paint.editor);
    }

    // ── Right strip: minimap or scrollbar ────────────────────────────────
    let mut action = None;
    let focused_pane = paint.editor.view_tree.focused_window()
        .and_then(|wid| paint.editor.view_tree.window(wid).cloned());
    let focused_buf = focused_pane.as_ref().and_then(|p| p.buffer_id);
    if strip_w > 0.0 && let (Some(pane), Some(buf_id)) = (focused_pane, focused_buf) {
        let scroll = pane_scroll_state(paint.editor, &pane, buf_id, visible_rows);
        let total = paint.editor.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count()).unwrap_or(1);
        if minimap {
            if let Some(line0) =
                Minimap::show(ui, paint.editor, buf_id, strip_rect, paint.theme, scroll.scroll_top, visible_rows)
            {
                action = Some(EditorPaneAction::ScrollViewport(line0 + 1));
            }
        } else {
            if let Some(line0) = scrollbar(ui, strip_rect, paint.theme, total, &scroll, visible_rows) {
                action = Some(EditorPaneAction::ScrollViewport(line0 + 1));
            }
        }
    }

    // ── Horizontal scrollbar (bottom strip) ──────────────────────────────
    if let (Some((content_cols, extent)), Some(buf_id)) = (h_geom, h_buf) {
        let scroll_left = h_focused.as_ref()
            .map(|p| crate::render::frame::pane_scroll_left(p, buf_id))
            .unwrap_or(0);
        if let Some(col) = h_scrollbar(ui, h_bar_rect, paint.theme, content_cols, extent, scroll_left) {
            action = Some(EditorPaneAction::ScrollLeft(col));
        }
    }

    // ── Mouse dispatch (pixel → cell coords, editor rect only) ───────────
    if !raw.is_empty() {
        let ed = &mut *paint.editor;
        for ev in raw.drain(..) {
            let pos = egui::pos2(ev.px, ev.py);
            // The horizontal bar owns its strip — never let a drag there
            // extend a text selection.
            if h_bar_h > 0.0 && h_bar_rect.contains(pos) {
                continue;
            }
            if !editor_rect.contains(pos)
                && matches!(ev.kind, kernel::input::event::MouseEventKind::Click
                    | kernel::input::event::MouseEventKind::Release)
            {
                continue;
            }
            let cell_x = ((ev.px - editor_rect.min.x) / char_w).max(0.0) as u16;
            let cell_y = ((ev.py - editor_rect.min.y) / line_h).max(0.0) as u16;
            mouse::dispatch_mouse(ed, surface, &MouseEvent {
                kind:      ev.kind,
                x:         cell_x,
                y:         cell_y,
                button:    ev.button,
                modifiers: ev.modifiers,
            });
        }
    }

    action
}

/// Classic scrollbar: track + thumb, click/drag to jump.  Returns a 0-based line.
fn scrollbar(
    ui:           &mut egui::Ui,
    rect:         Rect,
    theme:        &GuiTheme,
    total_lines:  usize,
    scroll:       &crate::render::frame::ScrollState,
    visible_rows: usize,
) -> Option<usize> {
    if total_lines <= visible_rows || rect.height() < 8.0 { return None; }
    ui.allocate_rect(rect, egui::Sense::click_and_drag());

    // No track fill: the editor background shows through, so the strip never
    // reads as a dark band beside the text.
    let track_h = rect.height();
    let frac   = visible_rows as f32 / total_lines as f32;
    let thumb_h = (track_h * frac).max(THUMB_MIN_H).min(track_h);
    let max_top = (track_h - thumb_h).max(0.0);
    let thumb_top = scroll.scroll_top as f32 / (total_lines - visible_rows).max(1) as f32 * max_top;
    let thumb = Rect::from_min_size(
        egui::pos2(rect.left() + 2.0, rect.top() + thumb_top),
        egui::vec2(SCROLLBAR_W - 4.0, thumb_h),
    );
    ui.painter().rect_filled(thumb, egui::CornerRadius::same(4), theme.surface1);

    let ptr = ui.ctx().pointer_hover_pos()?;
    if !rect.contains(ptr) { return None; }
    let pressed = ui.ctx().input(|i| i.pointer.primary_down());
    if !pressed { return None; }
    let frac_y = ((ptr.y - rect.top() - thumb_h / 2.0) / max_top).clamp(0.0, 1.0);
    Some((frac_y * total_lines.saturating_sub(1) as f32) as usize)
}

fn minimap_enabled(ed: &Editor) -> bool {
    ed.options.get("minimap-enabled").map(|v| v.as_str()) == Some("true")
}

/// Bottom horizontal scrollbar: track + thumb, click/drag to pan.  Returns the
/// first visible column the drag asks for.
fn h_scrollbar(
    ui:           &mut egui::Ui,
    rect:         Rect,
    theme:        &GuiTheme,
    content_cols: usize,
    extent:       usize,
    scroll_left:  usize,
) -> Option<usize> {
    if rect.height() < 4.0 || extent <= content_cols { return None; }
    ui.allocate_rect(rect, egui::Sense::click_and_drag());
    // Transparent track — the bottom strip must not look like a black tape.

    let track_w = rect.width();
    let max_left = (extent - content_cols) as f32;
    let frac = content_cols as f32 / extent as f32;
    let thumb_w = (track_w * frac).max(THUMB_MIN_H).min(track_w);
    let max_slide = (track_w - thumb_w).max(0.0);
    let thumb_x = if max_left > 0.0 { scroll_left as f32 / max_left * max_slide } else { 0.0 };
    let thumb = Rect::from_min_size(
        egui::pos2(rect.left() + thumb_x, rect.top() + 2.0),
        egui::vec2(thumb_w, (rect.height() - 4.0).max(2.0)),
    );
    ui.painter().rect_filled(thumb, egui::CornerRadius::same(4), theme.surface1);

    let ptr = ui.ctx().pointer_hover_pos()?;
    if !rect.contains(ptr) { return None; }
    if !ui.ctx().input(|i| i.pointer.primary_down()) { return None; }
    let frac_x = ((ptr.x - rect.left() - thumb_w / 2.0) / max_slide.max(1.0)).clamp(0.0, 1.0);
    Some((frac_x * max_left) as usize)
}

/// Completion popup — anchored at the cursor, one row per candidate, with a
/// selected-row highlight and accent bar.  Native rects, not surface cells.
fn draw_completion_popup(
    ui: &egui::Ui,
    ed: &Editor,
    theme: &GuiTheme,
    cursor_px: Option<(f32, f32)>,
    rect: Rect,
    _char_w: f32,
    line_h: f32,
) {
    if !ed.completion.visible || ed.completion.items.is_empty() { return; }
    let Some((cx, cy)) = cursor_px else { return };
    let items = &ed.completion.items;
    let shown = items.len().min(10);
    let font = FontId::new(line_h * 0.8, FontFamily::Monospace);
    let row_h = line_h * 1.15;
    let item_w = items.iter().take(shown)
        .map(|i| ui.fonts_mut(|f| f.layout_no_wrap(
            format!("{}  {}", i.label, i.kind), font.clone(), theme.text).size().x))
        .fold(48.0, f32::max);
    let w = (item_w + 22.0).min(rect.width() * 0.6).max(80.0);
    let h = row_h * shown as f32 + 8.0;
    let below = cy + line_h + 4.0;
    let y = if below + h <= rect.bottom() { below } else { (cy - h - 4.0).max(rect.top() + 2.0) };
    let x = cx.min(rect.right() - w - 4.0).max(rect.left() + 2.0);
    let area = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));

    let painter = ui.painter_at(area);
    painter.rect_filled(area, 6.0, theme.surface0);
    painter.rect_stroke(area, 6.0, egui::Stroke::new(1.0, theme.surface1), egui::StrokeKind::Inside);
    let sel = ed.completion.idx;
    for (i, item) in items.iter().take(shown).enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(x + 3.0, y + 4.0 + i as f32 * row_h),
            Vec2::new(w - 6.0, row_h),
        );
        let is_sel = i == sel;
        if is_sel {
            painter.rect_filled(row, 4.0, theme.surface1);
            painter.rect_filled(
                Rect::from_min_size(row.min, Vec2::new(2.5, row_h)), 1.0, theme.blue);
        }
        painter.text(
            Pos2::new(row.left() + 10.0, row.center().y),
            Align2::LEFT_CENTER,
            &item.label,
            font.clone(),
            if is_sel { theme.text } else { theme.subtext1 },
        );
        if !item.kind.is_empty() {
            painter.text(
                Pos2::new(row.right() - 8.0, row.center().y),
                Align2::RIGHT_CENTER,
                &item.kind,
                FontId::new(line_h * 0.62, FontFamily::Monospace),
                theme.subtext0,
            );
        }
    }
}

/// Pointer-dwell state machine: resolve the word under the pointer and
/// (re)arm the hover card when it changes.  Suppressed while dragging or
/// completing.
fn update_hover_card(ui: &egui::Ui, ed: &mut Editor, rect: Rect, char_w: f32, line_h: f32) {
    let pointer = ui.ctx().pointer_hover_pos();
    let busy = ed.mouse_state.pressed || ed.mouse_state.drag_active || ed.completion.visible;
    let pointer = pointer.filter(|p| rect.contains(*p) && !busy);
    let Some(p) = pointer else {
        ed.hover = None;
        return;
    };

    let target = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.window(wid).cloned())
        .and_then(|pane| {
            let buf_id = pane.buffer_id?;
            let cell_x = ((p.x - rect.min.x) / char_w).max(0.0) as u16;
            let cell_y = ((p.y - rect.min.y) / line_h).max(0.0) as u16;
            let offset = kernel::input::mouse::resolve_click_to_offset(ed, buf_id, &pane, cell_x, cell_y)?;
            let arc = ed.buffers.get(buf_id)?;
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let (ws, we) = kernel::input::mouse::word_range_at(&buf, offset)?;
            let diag_line = buf.position_of(offset).line;
            let diag = buf.get_diagnostics().into_iter()
                .find(|d| d.line == diag_line)
                .map(|d| d.display());
            Some((buf.slice(ws, we), diag, offset, buf_id))
        });

    // Ask the language server once per dwelling word.  Requesting on every
    // repaint would supersede the previous request each frame, and the server
    // answers each superseded one with ContentModified — a `[lsp/error]` wall.
    let same_word = matches!(
        (&target, ed.hover.as_ref()),
        (Some((w, _, _, _)), Some(card)) if &card.word == w
    );
    if !same_word
        && let Some((_, _, offset, buf_id)) = &target {
            request_lsp_hover(ed, *buf_id, *offset);
        }

    let new_card = match (&target, ed.hover.as_ref()) {
        (None, _) => None,
        (Some(_), Some(card)) if Some(&card.word) == target.as_ref().map(|t| &t.0) => None,
        (Some((w, diag, _, _)), _) => Some(build_hover_card(ed, w.clone(), diag.clone(), p.x, p.y)),
    };
    if target.is_none() {
        ed.hover = None;
    } else if let Some(card) = new_card {
        ed.hover = Some(card);
    }
}

/// Ask the buffer's language server for hover text at `offset`.  Silent
/// when no server is attached; the response lands via `attach_lsp_hover`.
fn request_lsp_hover(ed: &Editor, buf_id: usize, offset: usize) {
    let Some(lang) = ed.lsp.language_for_buffer(buf_id) else { return };
    let Some(server) = ed.lsp.get(&lang) else { return };
    let Some(arc) = ed.buffers.get(buf_id) else { return };
    let (uri, position, revision) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = buf.path.as_deref() else { return };
        (format!("file://{path}"), buf.lsp_position(offset.min(buf.len())), buf.revision())
    };
    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": position.0, "character": position.1 },
    });
    server.request("textDocument/hover", params, buf_id, revision);
}

fn build_hover_card(ed: &mut Editor, word: String, diag: Option<String>, px: f32, py: f32) -> kernel::state::HoverCard {
    let mut lines = Vec::new();
    let defs: Vec<(String, &kernel::index::extract::IndexedSymbol)> =
        ed.index.symbols_with_prefix(&word)
            .into_iter()
            .filter(|(_, sym)| sym.name == word)
            .take(3)
            .collect();
    if defs.is_empty() {
        lines.push(format!("{word} — no indexed definition"));
    }
    for (path, sym) in defs {
        let parts: Vec<&str> = path.rsplit('/').take(2).collect();
        let short = parts.iter().rev().copied().collect::<Vec<_>>().join("/");
        lines.push(format!("{} {word}  ·  {short}:{}", sym.kind, sym.line + 1));
    }
    if let Some(d) = diag {
        lines.push(d);
    }
    kernel::state::HoverCard { word, px, py, since: std::time::Instant::now(), lines }
}

/// The hover card itself — appears after the pointer dwells on a word.
fn draw_hover_card(ui: &egui::Ui, ed: &Editor, theme: &GuiTheme) {
    const DWELL: std::time::Duration = std::time::Duration::from_millis(450);
    let Some(card) = &ed.hover else { return };
    if card.since.elapsed() < DWELL || card.lines.is_empty() { return; }

    let title_font = FontId::new(13.0, FontFamily::Monospace);
    let body_font = FontId::new(12.0, FontFamily::Proportional);
    let row_h = 18.0;
    let w = card.lines.iter()
        .map(|l| ui.fonts_mut(|f|
            f.layout_no_wrap(l.clone(), title_font.clone(), theme.text).size().x))
        .fold(120.0, f32::max)
        + 20.0;
    let h = 10.0 + row_h * card.lines.len() as f32;
    let x = (card.px + 14.0).min(ui.clip_rect().right() - w - 8.0);
    let y = (card.py + 20.0).min(ui.clip_rect().bottom() - h - 8.0);
    let area = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));

    let painter = ui.painter_at(area);
    painter.rect_filled(area, 6.0, theme.crust);
    painter.rect_stroke(area, 6.0, egui::Stroke::new(1.0, theme.surface1), egui::StrokeKind::Inside);
    for (i, line) in card.lines.iter().enumerate() {
        let (font, color) = if i == 0 {
            (title_font.clone(), theme.text)
        } else {
            (body_font.clone(), theme.subtext1)
        };
        painter.text(
            Pos2::new(x + 10.0, y + 5.0 + i as f32 * row_h + row_h / 2.0),
            Align2::LEFT_CENTER, line, font, color,
        );
    }
}

/// Right-click menu over the editor: standard clipboard actions.  State
/// (open + position) lives in egui memory like the tab-drag state.
fn draw_context_menu(ui: &egui::Ui, ed: &mut Editor) {
    let ctx = ui.ctx();
    let state_id = egui::Id::new("magma_ctx_menu");

    let ptr = ctx.input(|i| i.pointer.interact_pos());
    if ctx.input(|i| i.pointer.secondary_clicked())
        && ptr.map(|p| ui.max_rect().contains(p)).unwrap_or(false) {
        ctx.data_mut(|d| d.insert_temp(state_id, ptr));
    }

    let Some(open_at) = ctx.data(|d| d.get_temp::<egui::Pos2>(state_id)) else { return };
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));

    let mut chosen: Option<&'static str> = None;
    egui::Area::new(egui::Id::new("magma_ctx_menu_area"))
        .fixed_pos(open_at)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(120.0);
                for (label, cmd) in [
                    ("Copy", "editor-copy"),
                    ("Cut", "editor-cut"),
                    ("Paste", "editor-paste"),
                    ("Select All", "select-all"),
                ] {
                    if ui.button(label).clicked() {
                        chosen = Some(cmd);
                        close = true;
                    }
                }
            });
        });

    // A click anywhere else dismisses the menu.
    if ctx.input(|i| i.pointer.primary_clicked()) && chosen.is_none() {
        close = true;
    }

    if let Some(cmd) = chosen {
        let _ = kernel::command::execute_command(ed, cmd, &std::collections::HashMap::new());
        ed.frame_dirty = true;
    }
    if close {
        ctx.data_mut(|d| d.remove::<egui::Pos2>(state_id));
    }
}

/// The quick-fix picker — server code actions at the cursor, one row per
/// action, navigated with up/down and applied with return.
fn draw_quickfix_menu(
    ui: &egui::Ui,
    ed: &Editor,
    theme: &GuiTheme,
    cursor_px: Option<(f32, f32)>,
    rect: Rect,
    line_h: f32,
) {
    let Some(menu) = &ed.quickfix_menu else { return };
    if menu.items.is_empty() { return }
    let Some((cx, cy)) = cursor_px else { return };

    let font = FontId::new(line_h * 0.8, FontFamily::Monospace);
    let row_h = line_h * 1.15;
    let shown = menu.items.len().min(12);
    let item_w = menu.items.iter().take(shown)
        .map(|a| ui.fonts_mut(|f| f.layout_no_wrap(
            format!("{}  {}", a.title, a.kind), font.clone(), theme.text).size().x))
        .fold(60.0, f32::max);
    let w = (item_w + 26.0).min(rect.width() * 0.7).max(120.0);
    let h = row_h * shown as f32 + 10.0;
    let below = cy + line_h + 4.0;
    let y = if below + h <= rect.bottom() { below } else { (cy - h - 4.0).max(rect.top() + 2.0) };
    let x = cx.min(rect.right() - w - 4.0).max(rect.left() + 2.0);
    let area = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));

    let painter = ui.painter_at(area);
    painter.rect_filled(area, 6.0, theme.crust);
    painter.rect_stroke(area, 6.0, egui::Stroke::new(1.0, theme.blue), egui::StrokeKind::Inside);
    for (i, action) in menu.items.iter().take(shown).enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(x + 3.0, y + 5.0 + i as f32 * row_h),
            Vec2::new(w - 6.0, row_h),
        );
        let is_sel = i == menu.idx;
        if is_sel {
            painter.rect_filled(row, 4.0, theme.surface1);
            painter.rect_filled(
                Rect::from_min_size(row.min, Vec2::new(2.5, row_h)), 1.0, theme.blue);
        }
        painter.text(
            Pos2::new(row.left() + 10.0, row.center().y),
            Align2::LEFT_CENTER,
            &action.title,
            font.clone(),
            if is_sel { theme.text } else { theme.subtext1 },
        );
        if !action.kind.is_empty() {
            painter.text(
                Pos2::new(row.right() - 8.0, row.center().y),
                Align2::RIGHT_CENTER,
                action.kind.rsplit('.').next().unwrap_or(&action.kind),
                FontId::new(line_h * 0.62, FontFamily::Monospace),
                theme.subtext0,
            );
        }
    }
}
