//! Project file tree sidebar widget for the GUI.

use eframe::egui;
use crate::kernel::state::Editor;

pub struct ProjectTree;

impl ProjectTree {
    pub fn show(ui: &mut egui::Ui, editor: &Editor) {
        ui.strong("Project");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for path in &editor.project_manager.project.files {
                let name = path.rsplit('/').next().unwrap_or(path.as_str());
                ui.label(name);
            }
        });
    }
}
