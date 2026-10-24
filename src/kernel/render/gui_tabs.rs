//! Native tab bar widget for the GUI — JetBrains-style.
//!
//! Each pane in the ViewTree becomes one tab.  The active tab is highlighted
//! and its underline fades in; inactive tabs are dimmed.  A blue dot marks
//! modified buffers.  The close button is only shown when the tab is hovered
//! or active.  Tabs can be drag-reordered.

use eframe::egui::{self, FontFamily, FontId, Pos2, Rect, CornerRadius, Sense, Vec2};
use crate::kernel::icons::IconKind;
use crate::kernel::state::id::WindowId;
use crate::kernel::state::Editor;
use super::gui_theme::{GuiTheme, icons};
use super::gui_icons;

/// A single tab's display data, derived from a Pane + Buffer.
#[derive(Clone)]
pub struct TabItem {
    pub label:     String,
    pub window_id: WindowId,
    pub modified:  bool,
    pub closable:  bool,
    /// Pack icon for the buffer's file; None for special (`*…*`) buffers.
    pub icon:      Option<IconKind>,
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
        editor.frame_dirty = true;
        match self {
            TabAction::Focus(id) => editor.view_tree.focus(id),
            TabAction::Close(id) => {
                let was_last = editor.view_tree.panes().len() <= 1;
                editor.view_tree.close(id);
                // A pane is never left empty: closing the last tab lands on
                // a fresh scratch buffer instead of nowhere to type.
                let focused_empty = editor.view_tree.focused_window()
                    .and_then(|wid| editor.view_tree.buffer(wid))
                    .is_none();
                if was_last || focused_empty {
                    let key = editor.create_buffer("*scratch*");
                    if let Some(win) = editor.view_tree.focused_window_mut() {
                        win.buffer_id = Some(key);
                    }
                }
            }
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
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    (buf.name.clone(), buf.modified())
                })
                .unwrap_or_else(|| ("?".to_string(), false));
            let short = label.rsplit('/').next().unwrap_or(&label).to_string();
            // Special buffers (*scratch*, *grep*) carry no file icon.
            let icon = (!short.starts_with('*'))
                .then(|| editor.icons.lookup(&short))
                .flatten()
                .cloned();
            Some(TabItem { label: short, window_id: pane.id, modified, closable: true, icon })
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

        const TAB_H:     f32 = 30.0;
        const H_PAD:     f32 = 12.0;
        const CLOSE_W:   f32 = 14.0;
        const DOT_R:     f32 = 3.5;
        const FONT_SIZE: f32 = 13.0;

        let font_id   = FontId::new(FONT_SIZE, FontFamily::Proportional);
        let close_fid = FontId::new(11.0, FontFamily::Proportional);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.set_min_height(TAB_H);

            // Strip baseline — the active tab's fill is painted over it, so
            // the selected tab reads as connected to the editor surface.
            let strip_left = ui.cursor().left();
            let baseline_y = ui.cursor().top() + TAB_H - 1.0;
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::new(strip_left, baseline_y),
                                    Vec2::new(ui.available_width() + strip_left, 1.0)),
                CornerRadius::ZERO,
                theme.surface0,
            );

            let ptr: Option<Pos2> = ui.ctx().pointer_hover_pos();

            for tab in &tabs {
                let is_active = focused == Some(tab.window_id);

                let label_w: f32 = ui.fonts_mut(|f| {
                    f.layout_no_wrap(tab.label.clone(), font_id.clone(), theme.text)
                        .size().x
                });

                let mut tab_w = H_PAD * 2.0 + label_w + CLOSE_W + 6.0;
                if tab.icon.is_some() { tab_w += gui_icons::SLOT; }
                if tab.modified { tab_w += DOT_R * 2.0 + 4.0; }

                let (tab_rect, tab_resp) =
                    ui.allocate_exact_size(Vec2::new(tab_w, TAB_H), Sense::click_and_drag());
                let tab_resp = tab_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                let is_hovered  = ptr.map(|p| tab_rect.contains(p)).unwrap_or(false);
                let is_drag_over = drag_over == Some(tab.window_id)
                    && dragging != Some(tab.window_id);

                // ── Background ──────────────────────────────────────────────
                if is_active {
                    ui.painter().rect_filled(tab_rect, Self::top_rounding(), theme.base);
                } else if is_hovered {
                    ui.painter().rect_filled(tab_rect, Self::top_rounding(),
                                             theme.surface0.gamma_multiply(0.5));
                }

                if is_drag_over {
                    ui.painter().rect_stroke(
                        tab_rect,
                        Self::top_rounding(),
                        egui::Stroke::new(2.0, theme.blue),
                        egui::StrokeKind::Inside,
                    );
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

                let fg = if is_active {
                    theme.text
                } else if is_hovered {
                    theme.subtext1
                } else {
                    theme.overlay0
                };

                if let Some(kind) = &tab.icon {
                    gui_icons::draw(ui, kind, Pos2::new(cx + gui_icons::SLOT / 2.0, cy),
                                    gui_icons::SLOT, font_id.clone(), fg);
                    cx += gui_icons::SLOT;
                }

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
