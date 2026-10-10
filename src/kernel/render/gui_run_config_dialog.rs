//! Run-configuration editor dialog — create, edit, and select the launch
//! definitions the run toolbar executes.  Save upserts by name (renaming
//! when `editing` differs), persists to the project's `.magma` store, and
//! makes the saved config active.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_dialog::DialogAction;
use super::gui_theme::GuiTheme;

fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
fn prop(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

pub struct RunConfigDialog {
    /// Original name when editing an existing config — a different `name`
    /// field on save renames it.
    pub editing: Option<String>,
    pub name: String,
    pub command: String,
    pub args: String,
    pub cwd: String,
    pub build_first: String,
    pub debuggable: bool,
}

impl RunConfigDialog {
    pub fn new() -> Self {
        Self {
            editing: None,
            name: String::new(),
            command: String::new(),
            args: String::new(),
            cwd: String::new(),
            build_first: String::new(),
            debuggable: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Prefill the fields from the active run configuration, or blank.
    pub fn load_from(&mut self, ed: &Editor) {
        let pm = &ed.project_manager;
        let cfg = pm.run_configs.active.as_deref()
            .and_then(|name| pm.run_configs.named(name))
            .cloned();
        match cfg {
            Some(cfg) => {
                self.editing = Some(cfg.name.clone());
                self.name = cfg.name;
                self.command = cfg.command;
                self.args = cfg.args.join(" ");
                self.cwd = cfg.cwd.unwrap_or_default();
                self.build_first = cfg.build_before_run.unwrap_or_default();
                self.debuggable = cfg.debuggable;
            }
            None => self.reset(),
        }
    }

    /// Returns true when the dialog should close.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        _editor: &Editor,
        theme: &GuiTheme,
        actions: &mut Vec<DialogAction>,
    ) -> bool {
        let label_w = 90.0;
        let row = |ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str| {
            ui.horizontal(|ui| {
                ui.add_sized(egui::vec2(label_w, 18.0),
                    egui::Label::new(RichText::new(label).font(prop(12.0)).color(theme.subtext1)));
                ui.add(egui::TextEdit::singleline(text)
                    .hint_text(hint)
                    .font(mono(12.0))
                    .desired_width(f32::INFINITY));
            });
        };
        row(ui, "Name", &mut self.name, "build, run, …");
        row(ui, "Command", &mut self.command, "cargo, npm, ./a.out…");
        row(ui, "Arguments", &mut self.args, "space-separated");
        row(ui, "Working dir", &mut self.cwd, "relative to project root (blank = root)");
        row(ui, "Build first", &mut self.build_first, "name of a config to run before this one");
        ui.horizontal(|ui| {
            ui.add_sized(egui::vec2(label_w, 18.0),
                egui::Label::new(RichText::new("Debuggable").font(prop(12.0)).color(theme.subtext1)));
            ui.checkbox(&mut self.debuggable, "");
        });

        ui.add_space(6.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                actions.push(DialogAction::UpsertRunConfig {
                    original: self.editing.clone(),
                    name: self.name.trim().to_string(),
                    command: self.command.trim().to_string(),
                    args: self.args.clone(),
                    cwd: self.cwd.clone(),
                    build_first: self.build_first.clone(),
                    debuggable: self.debuggable,
                });
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
        close
    }
}
