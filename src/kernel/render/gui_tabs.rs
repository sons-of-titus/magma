//! Native tab bar widget for the GUI — JetBrains-style.
//!
//! Each pane in the ViewTree becomes one tab.  The active tab is highlighted
//! and its underline fades in; inactive tabs are dimmed.  A blue dot marks
//! modified buffers.  The close button is only shown when the tab is hovered
//! or active.  Tabs can be drag-reordered.

use eframe::egui::{self, FontFamily, FontId, Pos2, Rect, CornerRadius, Sense, Vec2};
use crate::kernel::state::id::WindowId;
use crate::kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};

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
            TabAction::Focus(id)            => editor.view_tree.focus(id),
            TabAction::Close(id)            => editor.view_tree.close(id),
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

    fn top_rounding() -> CornerRadius {
        CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }
    }

    /// Render the tab bar; return any action to apply to the editor.
    pub fn show(ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme) -> Option<TabAction> {
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

        let font_id   = FontId::new(FONT_SIZE, FontFamily::Proportional);
        let close_fid = FontId::new(11.0, FontFamily::Proportional);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.set_min_height(TAB_H);

            let ptr: Option<Pos2> = ui.ctx().pointer_hover_pos();

            for tab in &tabs {
                let is_active = focused == Some(tab.window_id);

                let label_w: f32 = ui.fonts_mut(|f| {
                    f.layout_no_wrap(tab.label.clone(), font_id.clone(), theme.text)
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
                let bg = if is_active { theme.base } else { theme.crust };
                ui.painter().rect_filled(tab_rect, Self::top_rounding(), bg);

                if is_drag_over {
                    ui.painter().rect_stroke(
                        tab_rect,
                        Self::top_rounding(),
                        egui::Stroke::new(2.0, theme.blue),
                        egui::StrokeKind::Inside,
                    );
                }

                // Active-tab underline — fades in over ~150 ms.
                if is_active {
                    let anim = ui.ctx().animate_value_with_time(
                        egui::Id::new("tab_active").with(tab.window_id),
                        1.0,
                        0.15,
                    );
                    if anim > 0.01 {
                        let line = Rect::from_min_max(
                            Pos2::new(tab_rect.left(),  tab_rect.bottom() - ACTIVE_LINE_H),
                            Pos2::new(tab_rect.right(), tab_rect.bottom()),
                        );
                        ui.painter().rect_filled(
                            line, CornerRadius::ZERO,
                            theme.blue.gamma_multiply(anim.min(1.0)),
                        );
                    }
                }

                // ── Content ─────────────────────────────────────────────────
                let mut cx = tab_rect.left() + H_PAD;
                let cy = tab_rect.center().y;

                if tab.modified {
                    ui.painter().circle_filled(
                        Pos2::new(cx + DOT_R, cy),
                        DOT_R,
                        theme.blue,
                    );
                    cx += DOT_R * 2.0 + 4.0;
                }

                let fg = if is_active { theme.text } else { theme.overlay0 };
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
                    let cc = if on_close { theme.red } else { theme.subtext0 };
                    ui.painter().text(
                        close_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icons::CLOSE,
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
                            theme.text.gamma_multiply(0.63),
                        );
                        ui.ctx().request_repaint();
                    }
                }
            }
        });

        action
    }
}
