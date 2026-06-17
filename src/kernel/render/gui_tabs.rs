//! Native tab bar widget for the GUI — JetBrains-style.
//!
//! Each pane in the ViewTree becomes one tab.  The active tab is highlighted;
//! inactive tabs are dimmed.  A blue dot marks modified buffers.  The close
//! button is only shown when the tab is hovered or active.  Tabs can be
//! drag-reordered.

use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Vec2};
use crate::kernel::state::id::WindowId;
use crate::kernel::state::Editor;

// ── Catppuccin Mocha colours ──────────────────────────────────────────────────
const ACTIVE_BG:    Color32 = Color32::from_rgb(30,  30,  46);  // base
const INACTIVE_BG:  Color32 = Color32::from_rgb(17,  17,  27);  // crust
const ACTIVE_FG:    Color32 = Color32::from_rgb(205, 214, 244); // text
const INACTIVE_FG:  Color32 = Color32::from_rgb(108, 112, 134); // overlay0
const MODIFIED_DOT: Color32 = Color32::from_rgb(137, 180, 250); // blue
const CLOSE_FG:     Color32 = Color32::from_rgb(166, 173, 200); // subtext0
const CLOSE_HOVER:  Color32 = Color32::from_rgb(243, 139, 168); // red
const DROP_BORDER:  Color32 = Color32::from_rgb(137, 180, 250); // blue

/// A single tab's display data, derived from a Pane + Buffer.
#[derive(Clone)]
pub struct TabItem {
    pub label:     String,
    pub window_id: WindowId,
    pub modified:  bool,
    pub closable:  bool,
}

/// Action the caller should apply to the editor after `TabBar::show` returns.
pub enum TabAction {
    Focus(WindowId),
    Close(WindowId),
    /// Swap two pane positions in the ViewTree.
    Reorder { from: WindowId, to: WindowId },
}

impl TabAction {
    pub fn apply(self, editor: &mut Editor) {
        match self {
            TabAction::Focus(id)           => editor.view_tree.focus(id),
            TabAction::Close(id)           => editor.view_tree.close(id),
            TabAction::Reorder { from, to } => editor.view_tree.reorder(from, to),
        }
    }
}

pub struct TabBar;

impl TabBar {
    fn collect_tabs(editor: &Editor) -> Vec<TabItem> {
        editor.view_tree.panes().iter().filter_map(|pane| {
            let buf_id = pane.buffer_id?;
            let (label, modified) = editor.buffers.get(buf_id)
                .map(|arc| {
                    let buf = arc.lock().unwrap();
                    (buf.name.clone(), buf.modified())
                })
                .unwrap_or_else(|| ("?".to_string(), false));
            let short = label.rsplit('/').next().unwrap_or(&label).to_string();
            Some(TabItem { label: short, window_id: pane.id, modified, closable: true })
        }).collect()
    }

    /// Render the tab bar; return any action to apply to the editor.
    pub fn show(ui: &mut egui::Ui, editor: &Editor) -> Option<TabAction> {
        let tabs    = Self::collect_tabs(editor);
        let focused = editor.view_tree.focused_window();
        let mut action: Option<TabAction> = None;

        // Per-frame drag state stored in egui context memory.
        let drag_id      = egui::Id::new("tab_drag_source");
        let drag_over_id = egui::Id::new("tab_drag_over");
        let dragging: Option<WindowId> = ui.ctx().data(|d| d.get_temp(drag_id));
        let drag_over: Option<WindowId> = ui.ctx().data(|d| d.get_temp(drag_over_id));

        const TAB_H:     f32 = 28.0;
        const H_PAD:     f32 = 10.0;
        const CLOSE_W:   f32 = 14.0;
        const DOT_R:     f32 = 3.5;
        const FONT_SIZE: f32 = 13.0;
        const ACTIVE_LINE_H: f32 = 2.0;

        let font_id  = FontId::new(FONT_SIZE, FontFamily::Proportional);
        let close_fid = FontId::new(11.0, FontFamily::Proportional);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.set_min_height(TAB_H);

            let ptr: Option<Pos2> = ui.ctx().pointer_hover_pos();

            for tab in &tabs {
                let is_active = focused == Some(tab.window_id);

                let label_w: f32 = ui.fonts(|f| {
                    f.layout_no_wrap(tab.label.clone(), font_id.clone(), ACTIVE_FG)
                        .size().x
                });

                let mut tab_w = H_PAD * 2.0 + label_w + CLOSE_W + 6.0;
                if tab.modified { tab_w += DOT_R * 2.0 + 4.0; }

                let (tab_rect, tab_resp) =
                    ui.allocate_exact_size(Vec2::new(tab_w, TAB_H), Sense::click_and_drag());

                let is_hovered  = ptr.map(|p| tab_rect.contains(p)).unwrap_or(false);
                let is_drag_over = drag_over == Some(tab.window_id)
                    && dragging != Some(tab.window_id);

                // ── Background ──────────────────────────────────────────────
                let bg = if is_active { ACTIVE_BG } else { INACTIVE_BG };
                ui.painter().rect_filled(tab_rect, Rounding::ZERO, bg);

                if is_drag_over {
                    ui.painter().rect_stroke(
                        tab_rect,
                        Rounding::ZERO,
                        egui::Stroke::new(2.0, DROP_BORDER),
                    );
                }

                // Active-tab underline
                if is_active {
                    let line = Rect::from_min_max(
                        Pos2::new(tab_rect.left(),  tab_rect.bottom() - ACTIVE_LINE_H),
                        Pos2::new(tab_rect.right(), tab_rect.bottom()),
                    );
                    ui.painter().rect_filled(line, Rounding::ZERO, MODIFIED_DOT);
                }

                // ── Content ─────────────────────────────────────────────────
                let mut cx = tab_rect.left() + H_PAD;
                let cy = tab_rect.center().y;

                if tab.modified {
                    ui.painter().circle_filled(
                        Pos2::new(cx + DOT_R, cy),
                        DOT_R,
                        MODIFIED_DOT,
                    );
                    cx += DOT_R * 2.0 + 4.0;
                }

                let fg = if is_active { ACTIVE_FG } else { INACTIVE_FG };
                ui.painter().text(
                    Pos2::new(cx, cy),
                    egui::Align2::LEFT_CENTER,
                    &tab.label,
                    font_id.clone(),
                    fg,
                );

                // ── Close button (hover/active only) ────────────────────────
                let close_rect = Rect::from_center_size(
                    Pos2::new(tab_rect.right() - H_PAD - CLOSE_W / 2.0, cy),
                    Vec2::splat(CLOSE_W),
                );
                let on_close = ptr.map(|p| close_rect.contains(p)).unwrap_or(false);

                if (is_hovered || is_active) && tab.closable {
                    let cc = if on_close { CLOSE_HOVER } else { CLOSE_FG };
                    ui.painter().text(
                        close_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "✕",
                        close_fid.clone(),
                        cc,
                    );
                }

                // ── Click: focus or close ───────────────────────────────────
                if tab_resp.clicked() && action.is_none() {
                    if on_close && tab.closable {
                        action = Some(TabAction::Close(tab.window_id));
                    } else if !is_active {
                        action = Some(TabAction::Focus(tab.window_id));
                    }
                }

                // ── Drag: start ─────────────────────────────────────────────
                if tab_resp.drag_started() {
                    ui.ctx().data_mut(|d| d.insert_temp(drag_id, tab.window_id));
                }

                // Update which tab the pointer is over during a drag.
                if dragging.is_some() && is_hovered && dragging != Some(tab.window_id) {
                    ui.ctx().data_mut(|d| d.insert_temp(drag_over_id, tab.window_id));
                }
            }

            // ── Drag: release — apply reorder ───────────────────────────────
            if let Some(from_id) = dragging {
                if ui.ctx().input(|i| i.pointer.primary_released()) {
                    if let Some(to_id) = drag_over {
                        if action.is_none() {
                            action = Some(TabAction::Reorder { from: from_id, to: to_id });
                        }
                    }
                    ui.ctx().data_mut(|d| {
                        d.remove::<WindowId>(drag_id);
                        d.remove::<WindowId>(drag_over_id);
                    });
                }
            }

            // ── Drag ghost label ────────────────────────────────────────────
            if let Some(from_id) = dragging {
                if let Some(p) = ptr {
                    if let Some(lbl) = tabs.iter()
                        .find(|t| t.window_id == from_id)
                        .map(|t| t.label.as_str())
                    {
                        ui.painter().text(
                            p + Vec2::new(8.0, -8.0),
                            egui::Align2::LEFT_TOP,
                            lbl,
                            FontId::new(FONT_SIZE, FontFamily::Proportional),
                            Color32::from_rgba_unmultiplied(205, 214, 244, 160),
                        );
                        ui.ctx().request_repaint();
                    }
                }
            }
        });

        action
    }
}
