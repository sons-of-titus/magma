//! Modal dialogs + search UI — Phase 8.
//!
//! `DialogManager` owns at most one open dialog and renders it as a floating
//! egui window.  Dialogs never mutate the editor directly — they return
//! `DialogAction`s that `GuiApp::update` applies with a write lock, matching
//! the `TabAction` pattern used by the other chrome widgets.
//!
//! Colon-mode (`:`) is untouched: dialogs are an alternative UX, not a
//! replacement.

use std::collections::HashMap;
use eframe::egui::{self, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_goto_dialogs::PickerSource;
use super::gui_search_dialog::SearchDialog;
use super::gui_theme::{GuiTheme, icons};

// ── Dialog kinds + actions ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DialogKind {
    Search,
    GotoFile,
    GotoSymbol,
    CommandPalette,
    Settings,
}

impl DialogKind {
    pub fn title(self) -> &'static str {
        match self {
            DialogKind::Search         => "Search in Buffers",
            DialogKind::GotoFile       => "Go to File",
            DialogKind::GotoSymbol     => "Go to Symbol",
            DialogKind::CommandPalette => "Command Palette",
            DialogKind::Settings       => "Settings",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            DialogKind::Search         => icons::SEARCH,
            DialogKind::GotoFile       => icons::FILE,
            DialogKind::GotoSymbol     => icons::SYMBOL,
            DialogKind::CommandPalette => icons::COMMAND,
            DialogKind::Settings       => icons::SETTINGS,
        }
    }
}

/// A dialog-initiated editor mutation, applied by the GUI shell.
pub enum DialogAction {
    OpenFile(String),
    /// 1-indexed line in the focused buffer.
    GotoLine(usize),
    FocusBufferAndGoto { buf_id: usize, line: usize },
    /// `line` is the symbol index's 0-based definition line.
    GotoSymbol { file: String, line: usize },
    ExecuteCommand(String),
    SetOption { name: String, value: String },
    ReplaceAllInFocused { find: String, replace: String },
}

impl DialogAction {
    pub fn apply(self, ed: &mut Editor) {
        use crate::kernel::command::{execute_command, args::ArgValue};
        match self {
            DialogAction::OpenFile(path) => {
                let mut args = HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path));
                let _ = execute_command(ed, "open-file", &args);
            }
            DialogAction::GotoLine(line) => {
                let mut args = HashMap::new();
                args.insert("line".to_string(), ArgValue::Integer(line as i64));
                let _ = execute_command(ed, "goto-line", &args);
            }
            DialogAction::FocusBufferAndGoto { buf_id, line } => {
                let wid = ed.view_tree.panes().iter()
                    .find(|p| p.buffer_id == Some(buf_id))
                    .map(|p| p.id);
                match wid {
                    Some(id) => ed.view_tree.focus(id),
                    None => {
                        if let Some(win) = ed.view_tree.focused_window_mut() {
                            win.buffer_id = Some(buf_id);
                        }
                    }
                }
                DialogAction::GotoLine(line).apply(ed);
            }
            DialogAction::GotoSymbol { file, line } => {
                DialogAction::OpenFile(file.clone()).apply(ed);
                // `open-file` does not attach the new buffer to a window
                // (Janet normally reacts to BUFFER_CREATED) — find it by
                // path and focus-or-attach so the jump always lands.
                let buf_id = ed.buffers.iter()
                    .find(|(_, arc)| {
                        arc.lock().unwrap().path.as_deref() == Some(file.as_str())
                    })
                    .map(|(k, _)| k);
                if let Some(buf_id) = buf_id {
                    DialogAction::FocusBufferAndGoto { buf_id, line: line + 1 }.apply(ed);
                }
            }
            DialogAction::ExecuteCommand(name) => {
                let _ = execute_command(ed, &name, &HashMap::new());
            }
            DialogAction::SetOption { name, value } => {
                ed.options.insert(name, value);
            }
            DialogAction::ReplaceAllInFocused { find, replace } => {
                replace_all_in_focused(ed, &find, &replace);
            }
        }
    }
}

fn replace_all_in_focused(ed: &mut Editor, find: &str, replace: &str) {
    if find.is_empty() { return; }
    let Some(buf_id) = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid)) else { return };
    let Some(arc) = ed.buffers.get_mut(buf_id) else { return };
    let mut buf = arc.lock().unwrap();
    let text = buf.slice(0, buf.len());
    // Replace back-to-front so earlier offsets stay valid.
    let mut starts: Vec<usize> = text.match_indices(find).map(|(s, _)| s).collect();
    starts.reverse();
    for start in starts {
        buf.replace(start, start + find.len(), replace);
    }
    let len = buf.len();
    drop(buf);
    if let Some(view) = ed.view_of_buffer_mut(buf_id) {
        view.set_cursor(view.cursor_offset().min(len));
    }
}

// ── SettingsDialog ───────────────────────────────────────────────────────────

pub struct SettingsDialog {
    new_name:  String,
    new_value: String,
}

/// Options shown in the dialog; sorted for stable display.
pub fn sorted_options(editor: &Editor) -> Vec<(String, String)> {
    let mut opts: Vec<(String, String)> = editor.options.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    opts.sort();
    opts
}

impl SettingsDialog {
    pub fn new() -> Self {
        Self { new_name: String::new(), new_value: String::new() }
    }

    fn show(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &GuiTheme, actions: &mut Vec<DialogAction>) -> bool {
        ui.label(
            RichText::new("gui-theme").strong().color(theme.text),
        );
        ui.horizontal(|ui| {
            let current = editor.options.get("gui-theme").map(|s| s.as_str()).unwrap_or("mocha");
            for preset in ["mocha", "latte"] {
                if ui.selectable_label(current == preset, preset).clicked() {
                    actions.push(DialogAction::SetOption {
                        name:  "gui-theme".into(),
                        value: preset.into(),
                    });
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(300.0)
            .show(ui, |ui| {
                for (name, value) in sorted_options(editor) {
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

// ── DialogManager ────────────────────────────────────────────────────────────

pub struct DialogManager {
    pub open:     Option<DialogKind>,
    pub search:   SearchDialog,
    pub settings: SettingsDialog,
    pub picker:   super::gui_goto_dialogs::PickerDialog,
}

impl DialogManager {
    pub fn new() -> Self {
        Self {
            open:     None,
            search:   SearchDialog::new(),
            settings: SettingsDialog::new(),
            picker:   super::gui_goto_dialogs::PickerDialog::new(),
        }
    }

    pub fn is_open(&self) -> bool { self.open.is_some() }

    pub fn open_dialog(&mut self, kind: DialogKind) {
        if self.open == Some(kind) {
            self.open = None; // toggle
        } else {
            self.open = Some(kind);
            self.picker.reset();
            if kind == DialogKind::Search {
                self.search.query.clear();
                self.search.last_query.clear();
                self.search.results.clear();
            }
        }
    }

    pub fn close(&mut self) { self.open = None; }

    /// Map a GUI key string to the dialog it opens, if any.
    pub fn shortcut(key: &str) -> Option<DialogKind> {
        match key {
            "ctrl-p"         => Some(DialogKind::CommandPalette),
            "ctrl-shift-f"   => Some(DialogKind::Search),
            "ctrl-o"         => Some(DialogKind::GotoFile),
            "ctrl-shift-o"   => Some(DialogKind::GotoSymbol),
            "ctrl-comma"     => Some(DialogKind::Settings),
            _ => None,
        }
    }

    /// Render the open dialog.  Returns actions for the shell to apply.
    pub fn show(&mut self, ctx: &egui::Context, editor: &Editor, theme: &GuiTheme) -> Vec<DialogAction> {
        let Some(kind) = self.open else { return Vec::new() };
        let mut actions = Vec::new();
        let mut close = false;

        let title = format!("{}  {}", kind.icon(), kind.title());
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
            .default_width(480.0)
            .show(ctx, |ui| {
                match kind {
                    DialogKind::Search         => close = self.search.show(ui, editor, &mut actions),
                    DialogKind::Settings       => close = self.settings.show(ui, editor, theme, &mut actions),
                    DialogKind::GotoFile       => close = self.picker.show(ui, editor, theme, PickerSource::Files, &mut actions),
                    DialogKind::GotoSymbol     => close = self.picker.show(ui, editor, theme, PickerSource::Symbols, &mut actions),
                    DialogKind::CommandPalette => close = self.picker.show(ui, editor, theme, PickerSource::Commands, &mut actions),
                }
            });

        if close { self.open = None; }
        actions
    }
}
