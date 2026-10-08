//! Settings dialog — the typed option registry, rendered by category.
//!
//! Known options come from `state::options::OPTION_SPECS` with typed
//! widgets, effective values, per-option reset, and descriptions on hover.
//! Anything the user set that the registry doesn't know about renders under
//! "Custom" as raw strings.  Mutations are `DialogAction::SetOption`s the
//! shell applies — same contract as every other dialog.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::options::{self, OptionCategory, OptionKind, OPTION_SPECS};
use crate::kernel::state::Editor;
use super::gui_dialog::DialogAction;
use super::gui_theme::GuiTheme;

pub struct SettingsDialog {
    new_name:  String,
    new_value: String,
}

/// Options the registry doesn't know about; sorted for stable display.
pub fn sorted_options(editor: &Editor) -> Vec<(String, String)> {
    let known: Vec<&str> = OPTION_SPECS.iter().map(|s| s.name).collect();
    let mut opts: Vec<(String, String)> = editor.options.iter()
        .filter(|(k, _)| !known.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    opts.sort();
    opts
}

impl SettingsDialog {
    pub fn new() -> Self {
        Self { new_name: String::new(), new_value: String::new() }
    }

    fn category_header(ui: &mut egui::Ui, theme: &GuiTheme, title: &str) {
        ui.add_space(4.0);
        ui.label(
            RichText::new(title.to_ascii_uppercase())
                .size(11.0)
                .color(theme.overlay0)
                .strong(),
        );
        ui.separator();
    }

    fn spec_row(
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &GuiTheme,
        spec: &options::OptionSpec,
        actions: &mut Vec<DialogAction>,
    ) {
        let value = options::effective(editor, spec);
        ui.horizontal(|ui| {
            let name = ui.label(
                RichText::new(spec.name).strong().color(theme.text),
            ).on_hover_text(spec.description);

            let mut changed: Option<String> = None;
            match spec.kind {
                OptionKind::Bool => {
                    let mut on = value == "true";
                    if ui.checkbox(&mut on, "").changed() {
                        changed = Some(on.to_string());
                    }
                }
                OptionKind::Int => {
                    let mut n: i64 = value.parse().unwrap_or(0);
                    if ui.add(egui::DragValue::new(&mut n).range(0..=10_000)).changed() {
                        changed = Some(n.to_string());
                    }
                }
                OptionKind::Enum => {
                    egui::ComboBox::from_id_salt(spec.name)
                        .selected_text(value.clone())
                        .show_ui(ui, |ui| {
                            for choice in spec.choices {
                                if ui.selectable_label(value == *choice, *choice).clicked()
                                    && value != *choice {
                                    changed = Some(choice.to_string());
                                }
                            }
                        });
                }
                OptionKind::Str => {
                    let mut field = value.clone();
                    if ui.add(
                        egui::TextEdit::singleline(&mut field)
                            .desired_width(140.0)
                            .font(FontId::new(12.0, FontFamily::Monospace)),
                    ).changed() && field != value {
                        changed = Some(field);
                    }
                }
            }

            if value != spec.default
                && ui.small_button("↺").on_hover_text("Reset to default").clicked() {
                changed = Some(spec.default.to_string());
            }
            if value != spec.default {
                name.on_hover_text(format!("default: {}", spec.default));
            }

            if let Some(v) = changed {
                actions.push(DialogAction::SetOption {
                    name: spec.name.to_string(),
                    value: v,
                });
            }
        });
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme, actions: &mut Vec<DialogAction>) -> bool {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(360.0)
            .show(ui, |ui| {
                for category in [OptionCategory::Appearance, OptionCategory::Editing, OptionCategory::Behavior] {
                    Self::category_header(ui, theme, category.title());
                    for spec in OPTION_SPECS.iter().filter(|s| s.category == category) {
                        Self::spec_row(ui, editor, theme, spec, actions);
                    }
                }

                let custom = sorted_options(editor);
                if !custom.is_empty() {
                    Self::category_header(ui, theme, "Custom (not in the registry)");
                    for (name, value) in custom {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name.clone()).strong().color(theme.text));
                            let mut field = value.clone();
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut field)
                                    .desired_width(140.0)
                                    .font(FontId::new(12.0, FontFamily::Monospace)),
                            );
                            if resp.changed() && field != value {
                                actions.push(DialogAction::SetOption { name: name.clone(), value: field });
                            }
                        });
                    }
                }
            });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("new:");
            ui.add(egui::TextEdit::singleline(&mut self.new_name)
                .hint_text("option name").desired_width(110.0));
            ui.add(egui::TextEdit::singleline(&mut self.new_value)
                .hint_text("value").desired_width(110.0));
            if ui.button("Add").clicked() && !self.new_name.is_empty() {
                actions.push(DialogAction::SetOption {
                    name:  std::mem::take(&mut self.new_name),
                    value: std::mem::take(&mut self.new_value),
                });
            }
        });
        false
    }
}
