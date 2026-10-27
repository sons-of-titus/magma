//! List-picker dialogs — goto-file and goto-symbol.
//!
//! One `PickerDialog` widget with two sources.  Items are fuzzy-ranked
//! (`gui_fuzzy`) as the query changes; ↑/↓ move the selection, Enter or a
//! click activates it, Escape closes (handled by the shell's key gate).

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};
use magma::kernel::state::Editor;
use super::gui_dialog::DialogAction;
use super::gui_fuzzy::fuzzy_rank;
use super::gui_theme::GuiTheme;

const MAX_SHOWN: usize = 12;

#[derive(Clone, Copy, PartialEq)]
pub enum PickerSource {
    Files,
    Symbols,
}

/// One activatable row, independent of how it renders.
#[derive(Clone, Debug)]
pub enum PickerEntry {
    /// Absolute path to open.
    File(String),
    Symbol { name: String, file: String, line: usize },
}

impl PickerEntry {
    fn label(&self) -> String {
        match self {
            PickerEntry::File(p)     => p.clone(),
            PickerEntry::Symbol { name, file, line } => format!("{}  ·  {}:{}", name, file, line),
        }
    }
}

/// Collect picker entries for `source` from editor state.
pub fn collect_entries(editor: &Editor, source: PickerSource) -> Vec<PickerEntry> {
    match source {
        PickerSource::Files => {
            let root = editor.project_manager.project.root
                .as_ref().map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut files: Vec<String> = editor.project_manager.project.files.clone();
            for (_, arc) in editor.buffers.iter() {
                let name = arc.lock().unwrap_or_else(|e| e.into_inner()).name.clone();
                if !name.is_empty() && !files.contains(&name) {
                    files.push(name);
                }
            }
            files.into_iter().map(|rel| {
                if rel.starts_with('/') {
                    PickerEntry::File(rel)
                } else {
                    let full = if root.is_empty() {
                        rel
                    } else {
                        format!("{}/{}", root.trim_end_matches('/'), rel)
                    };
                    PickerEntry::File(full)
                }
            }).collect()
        }
        PickerSource::Symbols => {
            let mut syms = editor.semantic.symbol_index.all_symbols();
            syms.sort_by(|a, b| a.name.cmp(&b.name));
            syms.into_iter()
                .map(|s| PickerEntry::Symbol { name: s.name, file: s.file, line: s.line })
                .collect()
        }
    }
}

pub struct PickerDialog {
    pub query:       String,
    pub selected:    usize,
    focus_pending:   bool,
}

impl PickerDialog {
    pub fn new() -> Self {
        Self { query: String::new(), selected: 0, focus_pending: false }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.focus_pending = true;
    }

    fn activate(&self, entry: &PickerEntry, actions: &mut Vec<DialogAction>) {
        match entry.clone() {
            PickerEntry::File(path) => actions.push(DialogAction::OpenFile(path)),
            PickerEntry::Symbol { file, line, .. } => actions.push(DialogAction::GotoSymbol { file, line }),
        }
    }

    /// Returns true when the dialog should close.
    pub fn show(
        &mut self,
        ui:      &mut egui::Ui,
        editor:  &Editor,
        theme:   &GuiTheme,
        source:  PickerSource,
        actions: &mut Vec<DialogAction>,
    ) -> bool {
        let mono = FontId::new(12.0, FontFamily::Monospace);

        let hint = match source {
            PickerSource::Files   => "File name…",
            PickerSource::Symbols => "Symbol name…",
        };
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text(hint)
                .font(FontId::new(13.0, FontFamily::Monospace))
                .desired_width(f32::INFINITY),
        );
        if self.focus_pending {
            resp.request_focus();
            self.focus_pending = false;
        }

        let entries = collect_entries(editor, source);
        let labels: Vec<String> = entries.iter().map(|e| e.label()).collect();
        let ranked = fuzzy_rank(&self.query, &labels, |s| s.as_str());
        if self.selected >= ranked.len() { self.selected = ranked.len().saturating_sub(1); }

        // ↑/↓ + Enter navigation.
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.selected = (self.selected + 1).min(ranked.len().saturating_sub(1));
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.selected = self.selected.saturating_sub(1);
        }

        ui.separator();
        let shown = ranked.iter().take(MAX_SHOWN).enumerate();
        for (vis, (_, idx)) in shown {
            let entry = &entries[*idx];
            let is_sel = vis == self.selected;
            let label = match entry {
                PickerEntry::File(p) => RichText::new(p).font(mono.clone()),
                PickerEntry::Symbol { name, file, line } => RichText::new(format!(
                    "{}  ·  {}:{}",
                    name,
                    file.rsplit('/').next().unwrap_or(file),
                    line + 1,
                )).font(mono.clone()),
            };
            let label = label.color(if is_sel { Color32::WHITE } else { theme.subtext1 });
            if ui.selectable_label(is_sel, label).clicked() {
                self.activate(entry, actions);
                return true;
            }
        }
        if ranked.len() > MAX_SHOWN {
            ui.label(
                RichText::new(format!("… {} of {}", MAX_SHOWN, ranked.len()))
                    .size(11.0).color(theme.overlay0),
            );
        }
        if ranked.is_empty() {
            ui.label(RichText::new("No matches.").size(12.0).color(theme.overlay0));
        }

        if enter && !ranked.is_empty() {
            let (_, idx) = ranked[self.selected.min(ranked.len() - 1)];
            self.activate(&entries[idx], actions);
            return true;
        }
        false
    }
}
