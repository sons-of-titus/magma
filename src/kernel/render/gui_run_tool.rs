//! Run toolbar — the run-configuration selector, Run/Stop buttons, and the
//! config-editor entry point, rendered right-aligned in the nav band.
//! Actions-as-data: the shell applies them at its write boundary.

use eframe::egui::{self, RichText};
use crate::kernel::state::Editor;
use crate::kernel::task::TaskStatus;
use super::gui_theme::GuiTheme;

pub enum RunAction {
    Run,
    Debug,
    Stop,
    Select(String),
    Edit,
}

pub struct RunBar;

impl RunBar {
    pub fn show(ui: &mut egui::Ui, ed: &Editor, theme: &GuiTheme) -> Vec<RunAction> {
        let mut actions = Vec::new();
        let names = ed.project_manager.run_configs.names();
        let active = ed.project_manager.run_configs.active.clone()
            .or_else(|| names.first().cloned());
        let running = active.as_deref()
            .and_then(|n| ed.task_scheduler.task_by_name(n))
            .and_then(|id| ed.task_scheduler.status(id))
            == Some(&TaskStatus::Running);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.set_min_height(24.0);
            ui.add_space(8.0);

            if ui.button(RichText::new("⚙").size(13.0))
                .on_hover_text("Edit run configurations")
                .clicked() {
                actions.push(RunAction::Edit);
            }
            if ui.add_enabled(running, egui::Button::new(RichText::new("■").size(12.0)))
                .on_hover_text(if running { "Stop" } else { "Nothing running" })
                .clicked() {
                actions.push(RunAction::Stop);
            }
            let debuggable = active.as_deref()
                .and_then(|n| ed.project_manager.run_configs.named(n))
                .map(|c| c.debuggable)
                .unwrap_or(false);
            let debug_resp = ui.add_enabled(debuggable,
                egui::Button::new(RichText::new("🐞 Debug").size(12.0)));
            if debug_resp.on_hover_text(match &active {
                Some(n) if debuggable => format!("Debug '{n}'"),
                Some(_) => "Configuration is not debuggable".to_string(),
                None => "No run configuration".to_string(),
            }).clicked() {
                actions.push(RunAction::Debug);
            }
            let run_resp = ui.add_enabled(active.is_some(),
                egui::Button::new(RichText::new("▶ Run").size(12.0)));
            if run_resp.on_hover_text(match &active {
                Some(n) => format!("Run '{n}'"),
                None => "No run configuration".to_string(),
            }).clicked() {
                actions.push(RunAction::Run);
            }

            let mut selected = active.clone();
            egui::ComboBox::from_id_salt("magma_run_config_select")
                .selected_text(active.as_deref().unwrap_or("no configurations"))
                .width(130.0)
                .show_ui(ui, |ui| {
                    for name in &names {
                        ui.selectable_value(&mut selected, Some(name.clone()), name);
                    }
                    if names.is_empty() {
                        ui.label(RichText::new("no configurations").size(12.0).color(theme.overlay0));
                    }
                });
            if selected != active
                && let Some(name) = selected {
                    actions.push(RunAction::Select(name));
                }
        });
        actions
    }
}
