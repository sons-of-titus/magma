//! Central editor pane — the Surface-based editing area plus its scrollbar
//! and minimap strip.
//!
//! The pane shrinks the character grid to leave room for a right-hand strip:
//! a classic scrollbar (default) or the code-overview minimap
//! (`minimap-enabled` option).  Mouse events that land inside the strip are
//! consumed by the strip; only editor-rect events reach `dispatch_mouse`.

use std::sync::{Arc, RwLock};

use eframe::egui::{self, Rect};
use eframe::wgpu::TextureFormat;

use crate::kernel::input::event::MouseEvent;
use crate::kernel::input::mouse;
use crate::kernel::render::frame::{compute_scroll_state, render_frame};
use crate::kernel::render::gpu_atlas::{build_glyph_instances, build_rect_instances, GpuGlyphAtlas};
use crate::kernel::render::gpu_paint_callback::MagmaPaintCallback;
use crate::kernel::render::surface::Surface;
use crate::kernel::state::Editor;
use super::gui_input::RawMouseEvent;
use super::gui_minimap::{Minimap, STRIP_W};
use super::gui_render::present;
use super::gui_theme::GuiTheme;

const SCROLLBAR_W: f32 = 10.0;
const THUMB_MIN_H: f32 = 18.0;

/// Everything the pane needs to paint one frame.
pub struct PanePaint<'a> {
    pub editor:        &'a Arc<RwLock<Editor>>,
    pub atlas:         &'a GpuGlyphAtlas,
    pub use_gpu:       bool,
    pub target_format: Option<TextureFormat>,
    /// Atlas pixels to re-upload this frame, if dirty.
    pub atlas_pixels:  Option<Vec<u8>>,
    pub theme:         &'a GuiTheme,
}

/// Action produced by pane chrome, applied by the caller after the frame.
pub enum EditorPaneAction {
    /// Jump the focused cursor to a 1-indexed line (scrollbar / minimap click).
    GotoLine(usize),
}

/// Render the central editor pane.  Returns a chrome-initiated action.
pub fn render_editor_pane(
    ui:       &mut egui::Ui,
    paint:    &PanePaint<'_>,
    char_w:   f32,
    line_h:   f32,
    raw:      &mut Vec<RawMouseEvent>,
) -> Option<EditorPaneAction> {
    let full   = ui.available_rect_before_wrap();
    let minimap = {
        let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
        minimap_enabled(&ed)
    };

    let strip_w = if minimap { STRIP_W } else { SCROLLBAR_W };
    let editor_rect = Rect::from_min_max(
        full.min,
        egui::pos2(full.right() - strip_w, full.bottom()),
    );
    let strip_rect = Rect::from_min_max(
        egui::pos2(full.right() - strip_w, full.top()),
        full.max,
    );

    let visible_cols = ((editor_rect.width()  / char_w).floor() as usize).max(1);
    let visible_rows = ((editor_rect.height() / line_h).floor() as usize).max(1);
    let mut surface = Surface::new(visible_cols as u16, visible_rows as u16);
    {
        let mut ed = paint.editor.write().unwrap_or_else(|e| e.into_inner());
        ed.view_tree.resize(visible_cols as u16, visible_rows as u16);
        // The layout reserves a row (TUI status bar); the surface must match
        // the painted extent so no unpainted cells reach the screen.
        let extent_w = ed.view_tree.panes().iter()
            .map(|p| p.x + p.width).max().unwrap_or(visible_cols as u16);
        let extent_h = ed.view_tree.panes().iter()
            .map(|p| p.y + p.height).max().unwrap_or(visible_rows as u16);
        if extent_w as usize != visible_cols || extent_h as usize != visible_rows {
            surface = Surface::new(extent_w, extent_h);
        }
    }
    {
        let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
        render_frame(&ed, &mut surface, true);
    }

    let bg_color = {
        let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
        let (r, g, b) = ed.theme_color("gui-bg");
        egui::Color32::from_rgb(r, g, b)
    };

    if paint.use_gpu && let Some(fmt) = paint.target_format {
        let bg = [
            bg_color.r() as f32 / 255.0,
            bg_color.g() as f32 / 255.0,
            bg_color.b() as f32 / 255.0,
            1.0,
        ];
        let glyphs = build_glyph_instances(
            &surface, paint.atlas, char_w, line_h,
            editor_rect.min.x, editor_rect.min.y, true,
        );
        let rects = build_rect_instances(
            &surface, char_w, line_h,
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
        present(ui, paint.editor, &surface, editor_rect, char_w, line_h, 0);
    }

    // ── Right strip: minimap or scrollbar ────────────────────────────────
    let mut action = None;
    let focused_buf = {
        let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
        ed.view_tree.focused_window().and_then(|wid| ed.view_tree.buffer(wid))
    };
    if strip_w > 0.0 && let Some(buf_id) = focused_buf {
        let scroll = {
            let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
            compute_scroll_state(&ed, buf_id, visible_rows)
        };
        let total = {
            let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
            ed.buffers.get(buf_id).map(|a| a.lock().unwrap().line_count()).unwrap_or(1)
        };
        if minimap {
            let ed = paint.editor.read().unwrap_or_else(|e| e.into_inner());
            if let Some(line0) =
                Minimap::show(ui, &ed, buf_id, strip_rect, paint.theme, scroll.scroll_top, visible_rows)
            {
                action = Some(EditorPaneAction::GotoLine(line0 + 1));
            }
        } else {
            if let Some(line0) = scrollbar(ui, strip_rect, paint.theme, total, &scroll, visible_rows) {
                action = Some(EditorPaneAction::GotoLine(line0 + 1));
            }
        }
    }

    // ── Mouse dispatch (pixel → cell coords, editor rect only) ───────────
    if !raw.is_empty() {
        let mut ed = paint.editor.write().unwrap_or_else(|e| e.into_inner());
        for ev in raw.drain(..) {
            if !editor_rect.contains(egui::pos2(ev.px, ev.py))
                && matches!(ev.kind, crate::kernel::input::event::MouseEventKind::Click
                    | crate::kernel::input::event::MouseEventKind::Release)
            {
                continue;
            }
            let cell_x = ((ev.px - editor_rect.min.x) / char_w).max(0.0) as u16;
            let cell_y = ((ev.py - editor_rect.min.y) / line_h).max(0.0) as u16;
            mouse::dispatch_mouse(&mut ed, &surface, &MouseEvent {
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
    scroll:       &crate::kernel::render::frame::ScrollState,
    visible_rows: usize,
) -> Option<usize> {
    if total_lines <= visible_rows || rect.height() < 8.0 { return None; }
    ui.allocate_rect(rect, egui::Sense::click_and_drag());

    ui.painter().rect_filled(rect, 0.0, theme.crust);

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
