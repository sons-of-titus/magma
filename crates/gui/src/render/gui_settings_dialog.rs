//! Settings dialog — a JetBrains-style two-pane window: a category list on the
//! left, a search box on top, and the selected category's typed options on the
//! right.  Known options come from `state::options::OPTION_SPECS`; anything the
//! registry doesn't know renders under "Advanced" as raw strings.  Mutations
//! are `DialogAction::SetOption`s the shell applies — same contract as every
//! other dialog.

use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::options::{self, OptionCategory, OptionKind, OPTION_SPECS};
use crate::kernel::state::Editor;
use super::gui_dialog::DialogAction;
use super::gui_theme::{GuiTheme, icons};

pub struct SettingsDialog {
    section:   usize,
    search:    String,
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

const SECTIONS: &[(&str, Option<OptionCategory>)] = &[
    ("Appearance", Some(OptionCategory::Appearance)),
    ("Editing",    Some(OptionCategory::Editing)),
    ("Behavior",   Some(OptionCategory::Behavior)),
    ("Advanced",   None),
];

impl SettingsDialog {
    pub fn new() -> Self {
        Self { section: 0, search: String::new(), new_name: String::new(), new_value: String::new() }
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
                && ui.small_button(icons::RESET).on_hover_text("Reset to default").clicked() {
                changed = Some(spec.default.to_string());
            }
            if value != spec.default {
                name.on_hover_text(format!("default: {}", spec.default));
            }

            if let Some(v) = changed {
                actions.push(DialogAction::SetOption { name: spec.name.to_string(), value: v });
            }
        });
    }

    fn search_matches(spec: &options::OptionSpec, q: &str) -> bool {
        spec.name.to_lowercase().contains(q)
            || spec.description.to_lowercase().contains(q)
            || spec.category.title().to_lowercase().contains(q)
    }

    fn render_specs(
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &GuiTheme,
        specs: impl Iterator<Item = &'static options::OptionSpec>,
        actions: &mut Vec<DialogAction>,
    ) {
        let mut any = false;
        for spec in specs {
            any = true;
            Self::spec_row(ui, editor, theme, spec, actions);
        }
        if !any {
            ui.label(RichText::new("No matching settings.").size(12.0).color(theme.overlay0));
        }
    }

    fn render_advanced(
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &GuiTheme,
        state: &mut Self,
        actions: &mut Vec<DialogAction>,
    ) {
        ui.label(RichText::new("Custom (not in the registry)").size(11.0).color(theme.overlay0).strong());
        let custom = sorted_options(editor);
        if custom.is_empty() {
            ui.label(RichText::new("No custom options set.").size(12.0).color(theme.overlay0));
        }
        for (name, value) in custom {
            ui.horizontal(|ui| {
                ui.label(RichText::new(name.clone()).strong().color(theme.text));
                let mut field = value.clone();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut field)
                        .desired_width(180.0)
                        .font(FontId::new(12.0, FontFamily::Monospace)),
                );
                if resp.changed() && field != value {
                    actions.push(DialogAction::SetOption { name: name.clone(), value: field });
                }
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Add a setting").size(11.0).color(theme.overlay0).strong());
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut state.new_name)
                .hint_text("option name").desired_width(140.0));
            ui.add(egui::TextEdit::singleline(&mut state.new_value)
                .hint_text("value").desired_width(140.0));
            if ui.button("Add").clicked() && !state.new_name.is_empty() {
                actions.push(DialogAction::SetOption {
                    name:  std::mem::take(&mut state.new_name),
                    value: std::mem::take(&mut state.new_value),
                });
            }
        });
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme, actions: &mut Vec<DialogAction>) -> bool {
        // Search across every option.
        ui.horizontal(|ui| {
            ui.label(RichText::new(super::gui_theme::icons::SEARCH).color(theme.overlay0));
            ui.add(egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search settings")
                .desired_width(f32::INFINITY));
        });
        ui.separator();

        let query = self.search.trim().to_lowercase();
        ui.horizontal_top(|ui| {
            // Left: category list.
            ui.vertical(|ui| {
                ui.set_width(170.0);
                ui.set_min_height(420.0);
                for (i, (label, _)) in SECTIONS.iter().enumerate() {
                    if ui.selectable_label(self.section == i, *label).clicked() {
                        self.section = i;
                    }
                }
            });
            ui.separator();

            // Right: the selected category (or search results).
            egui::ScrollArea::vertical()
                .id_salt("settings_pane")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(520.0);
                    if !query.is_empty() {
                        Self::render_specs(ui, editor, theme,
                            OPTION_SPECS.iter().filter(|s| Self::search_matches(s, &query)), actions);
                        return;
                    }
                    match SECTIONS[self.section.min(SECTIONS.len() - 1)].1 {
                        Some(category) => Self::render_specs(ui, editor, theme,
                            OPTION_SPECS.iter().filter(|s| s.category == category), actions),
                        None => Self::render_advanced(ui, editor, theme, self, actions),
                    }
                });
        });
        false
    }
}
