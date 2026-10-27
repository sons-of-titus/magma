//! Dock render paths for `ToolWindowManager`: the bottom tool bar, the docked
//! bottom/right panels, the right icon rail, and floating windows.
//!
//! Split from `tool_window` (framework) at the render boundary.

use eframe::egui::{self, Align2, FontFamily, FontId, Pos2, Sense};
use magma::kernel::state::Editor;
use super::gui_theme::GuiTheme;
use super::tool_window::{DEBUG_TOOL, DockPosition, ToolAction, ToolWindowManager};

fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

/// A JetBrains dock-rail button: 28×28, active gets a blue edge bar.  Used by
/// both the left and right rails.
pub(crate) fn dock_button(
    ui:           &mut egui::Ui,
    icon:         &str,
    active:       bool,
    tooltip:      &str,
    theme:        &GuiTheme,
    vertical:     bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::click());
    if active {
        let edge = if vertical {
            egui::Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + 3.0),
                Pos2::new(rect.left() + 2.5, rect.bottom() - 3.0),
            )
        } else {
            egui::Rect::from_min_max(
                Pos2::new(rect.left() + 3.0, rect.top()),
                Pos2::new(rect.right() - 3.0, rect.top() + 2.5),
            )
        };
        ui.painter().rect_filled(edge, 1.0, theme.blue);
    }
    if resp.hovered() {
        ui.painter().rect_filled(rect, 2, theme.surface0.gamma_multiply(0.5));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        prop(15.0),
        if active { theme.blue } else { theme.overlay0 },
    );
    resp.on_hover_text(tooltip).clicked()
}

impl ToolWindowManager {
    /// The always-visible bottom tool bar (JetBrains bottom tool-window
    /// buttons).  Returns the clicked entry index.
    pub fn show_bottom_bar(&self, ui: &mut egui::Ui, theme: &GuiTheme) -> Option<usize> {
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.set_min_height(24.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, entry) in self.entries.iter().enumerate() {
                if entry.dock != DockPosition::Bottom { continue }
                let active = entry.visible && i == self.active_bottom;
                let label = format!("{}  {}", entry.window.icon(), entry.window.title());
                let w = ui.painter()
                    .layout_no_wrap(label.clone(), prop(12.0), theme.text)
                    .size().x + 18.0;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::click());
                if active {
                    ui.painter().rect_filled(rect, 2, theme.surface0);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 2, theme.surface0.gamma_multiply(0.5));
                }
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    &label,
                    prop(12.0),
                    if active { theme.text } else { theme.overlay0 },
                );
                if resp.clicked() { clicked = Some(i); }
            }
        });
        clicked
    }

    /// Render the active bottom-docked tool window as a resizable panel.
    pub fn show_docked_bottom(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let visible: Vec<usize> = self.entries.iter().enumerate()
            .filter(|(_, e)| e.visible && e.dock == DockPosition::Bottom)
            .map(|(i, _)| i)
            .collect();
        if visible.is_empty() { return Vec::new() }

        let active_idx = visible.iter().find(|&&i| i == self.active_bottom)
            .copied().unwrap_or(visible[0]);
        self.active_bottom = active_idx;

        // Bottom panels default to an unbounded max height; cap it so a long
        // tool output can never swallow the editor.
        let max_h = (ui.available_height() - 120.0).max(120.0);
        let mut actions = Vec::new();
        egui::containers::Panel::bottom("magma_tools_bottom")
            .resizable(true)
            .default_size(220.0)
            .max_size(max_h)
            .frame(egui::Frame::NONE.fill(theme.mantle).inner_margin(egui::Margin::same(0)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        actions = self.entries[active_idx].window.ui(ui, editor, theme);
                    });
            });
        actions
    }

    /// Render the Debugger as the left dock's content when it is docked there
    /// (the caller owns the left panel).  Returns no content otherwise.
    pub fn show_debug_left(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let Some(entry) = self.entries.get(DEBUG_TOOL) else { return Vec::new() };
        if entry.dock != DockPosition::Left {
            return Vec::new();
        }
        egui::ScrollArea::vertical()
            .id_salt("magma_debug_left")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.entries[DEBUG_TOOL].window.ui(ui, editor, theme)
            })
            .inner
    }

    /// Render the active right-docked tool window (the Agent) as a resizable
    /// panel on the right edge of the editor.  A hard `max_size` keeps its
    /// content (a text field wanting full width, long transcript lines) from
    /// expanding the panel across the window.
    pub fn show_docked_right(&mut self, ui: &mut egui::Ui, editor: &mut Editor, theme: &GuiTheme, width: f32) -> Vec<ToolAction> {
        let Some(idx) = self.active_right_index() else { return Vec::new() };
        let max_w = (ui.available_width() - 360.0).clamp(280.0, 560.0);
        let mut actions = Vec::new();
        egui::containers::Panel::right("magma_agent_dock")
            .resizable(true)
            .min_size(260.0)
            .max_size(max_w)
            .default_size(width.clamp(260.0, max_w))
            .frame(egui::Frame::NONE.fill(theme.mantle).inner_margin(egui::Margin::same(0)))
            .show(ui, |ui| {
                actions = self.entries[idx].window.ui(ui, editor, theme);
            });
        actions
    }

    /// The right icon rail (Agent).  Returns the clicked entry index.
    pub fn show_right_rail(&self, ui: &mut egui::Ui, theme: &GuiTheme) -> Option<usize> {
        let mut clicked = None;
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            for (i, entry) in self.entries.iter().enumerate() {
                if entry.dock != DockPosition::Right { continue }
                let icon = entry.window.icon().to_string();
                let active = entry.visible && i == self.active_right;
                if dock_button(ui, &icon, active, entry.window.title(), theme, true) {
                    clicked = Some(i);
                }
                ui.add_space(4.0);
            }
        });
        clicked
    }

    /// Render floating tool windows as independent egui Windows.
    pub fn show_floating(&mut self, ctx: &egui::Context, editor: &mut Editor, theme: &GuiTheme) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        for entry in &mut self.entries {
            if !entry.visible || entry.dock != DockPosition::Floating { continue }
            let title = entry.window.title().to_string();
            let mut out = Vec::new();
            egui::Window::new(&title)
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    out = entry.window.ui(ui, editor, theme);
                });
            actions.append(&mut out);
        }
        actions
    }
}
