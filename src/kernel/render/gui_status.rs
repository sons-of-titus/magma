//! Native status bar widget for the GUI.

use eframe::egui::{self, Color32};
use crate::kernel::state::Editor;
use super::gui_fonts::mode_color;
use super::gui_render::safe_boundary;

pub struct StatusBar;

impl StatusBar {
    pub fn show(ui: &mut egui::Ui, editor: &Editor) {
        let mode = editor.vim_mode_name();
        let pill_color = mode_color(&mode);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(" {} ", mode))
                    .background_color(pill_color)
                    .color(Color32::WHITE)
                    .monospace()
                    .strong(),
            );

            ui.separator();

            if let Some(wid) = editor.view_tree.focused_window()
                && let Some(buf_id) = editor.view_tree.buffer(wid)
                && let Some(arc) = editor.buffers.get(buf_id)
            {
                let buf = arc.lock().unwrap();
                let name = buf.name.clone();
                let modified = buf.modified();
                let cursor_offset = editor.views.get(&buf_id)
                    .map(|v| v.cursor_offset())
                    .unwrap_or(0);
                let content = buf.slice(0, buf.len());
                drop(buf);

                let safe = safe_boundary(&content, cursor_offset);
                let before = &content[..safe];
                let row = before.chars().filter(|&c| c == '\n').count() + 1;
                let last = before.rfind('\n').map(|p| &before[p + 1..]).unwrap_or(before);
                let col = last.chars().count() + 1;
                let dirty = if modified { " [+]" } else { "" };

                ui.label(format!("{}{}", name, dirty));
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Ln {}, Col {}", row, col));
                });
            }
        });
    }
}
