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
use eframe::egui::{self, RichText};
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
    CommandLine,
    Settings,
    QuitConfirm,
    RunConfig,
}

impl DialogKind {
    pub fn title(self) -> &'static str {
        match self {
            DialogKind::Search         => "Search in Buffers",
            DialogKind::GotoFile       => "Go to File",
            DialogKind::GotoSymbol     => "Go to Symbol",
            DialogKind::CommandPalette => "Command Palette",
            DialogKind::CommandLine    => "Run Command",
            DialogKind::Settings       => "Settings",
            DialogKind::QuitConfirm    => "Unsaved Changes",
            DialogKind::RunConfig      => "Run Configurations",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            DialogKind::Search         => icons::SEARCH,
            DialogKind::GotoFile       => icons::FILE,
            DialogKind::GotoSymbol     => icons::SYMBOL,
            DialogKind::CommandPalette => icons::COMMAND,
            DialogKind::CommandLine    => icons::COMMAND,
            DialogKind::Settings       => icons::SETTINGS,
            DialogKind::QuitConfirm    => icons::CLOSE,
            DialogKind::RunConfig      => icons::SETTINGS,
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
    /// Open (or focus) a file and jump to a 1-based line — quickfix entries,
    /// find-in-files results, and task output all land here.
    OpenFileAtLine { path: String, line: usize },
    ExecuteCommand(String),
    /// A full `"name args…"` line from the command line or the palette's
    /// argument prompt — parsed by `execute_command_line`.
    ExecuteCommandLine(String),
    SetOption { name: String, value: String },
    ReplaceAllInFocused { find: String, replace: String },
    UpsertRunConfig {
        original: Option<String>,
        name: String,
        command: String,
        args: String,
        cwd: String,
        build_first: String,
        debuggable: bool,
    },
}

impl DialogAction {
    pub fn apply(self, ed: &mut Editor) {
        use crate::kernel::command::{execute_command, args::ArgValue};
        use crate::kernel::state::MessageKind;
        ed.frame_dirty = true;
        // Dialog-initiated commands report failure instead of discarding it —
        // a silent dialog is how users learn the editor cannot be trusted.
        let report = |ed: &mut Editor, result: Result<(), String>| {
            if let Err(e) = result {
                ed.show_message(MessageKind::Error, e);
            }
        };
        match self {
            DialogAction::OpenFile(path) => {
                let mut args = HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path));
                let result = execute_command(ed, "open-file", &args);
                report(ed, result);
            }
            DialogAction::GotoLine(line) => {
                let mut args = HashMap::new();
                args.insert("line".to_string(), ArgValue::Integer(line as i64));
                let result = execute_command(ed, "goto-line", &args);
                report(ed, result);
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
                DialogAction::OpenFileAtLine { path: file, line: line + 1 }.apply(ed);
            }
            DialogAction::OpenFileAtLine { path, line } => {
                DialogAction::OpenFile(path.clone()).apply(ed);
                let buf_id = ed.buffers.iter()
                    .find(|(_, arc)| {
                        arc.lock().unwrap().path.as_deref() == Some(path.as_str())
                    })
                    .map(|(k, _)| k);
                if let Some(buf_id) = buf_id {
                    DialogAction::FocusBufferAndGoto { buf_id, line }.apply(ed);
                }
            }
            DialogAction::ExecuteCommand(name) => {
                let result = execute_command(ed, &name, &HashMap::new());
                report(ed, result);
            }
            DialogAction::ExecuteCommandLine(line) => {
                let result = crate::kernel::command::execute_command_line(ed, &line);
                report(ed, result);
            }
            DialogAction::UpsertRunConfig { original, name, command, args, cwd, build_first, debuggable } => {
                upsert_run_config(ed, original, name, command, args, cwd, build_first, debuggable);
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

fn upsert_run_config(
    ed: &mut Editor,
    original: Option<String>,
    name: String,
    command: String,
    args: String,
    cwd: String,
    build_first: String,
    debuggable: bool,
) {
    use crate::kernel::state::MessageKind;
    let cfg = crate::kernel::run_config::RunConfig {
        name,
        command,
        args: args.split_whitespace().map(str::to_string).collect(),
        cwd: if cwd.trim().is_empty() { None } else { Some(cwd.trim().to_string()) },
        env: HashMap::new(),
        build_before_run: if build_first.trim().is_empty() { None } else { Some(build_first.trim().to_string()) },
        debuggable,
    };
    match crate::kernel::run_config::upsert_config(ed, original, cfg) {
        Ok(saved) => ed.show_message(MessageKind::Info, format!("Saved run configuration '{saved}'")),
        Err(e) => ed.show_message(MessageKind::Error, e),
    }
}

// ── SettingsDialog ───────────────────────────────────────────────────────────
// The typed settings UI lives in `gui_settings_dialog.rs`; re-exported here
// so `DialogManager` and existing call sites keep one import path.

pub use super::gui_settings_dialog::SettingsDialog;

// ── DialogManager ────────────────────────────────────────────────────────────

pub struct DialogManager {
    pub open:     Option<DialogKind>,
    pub search:   SearchDialog,
    pub settings: SettingsDialog,
    pub picker:   super::gui_goto_dialogs::PickerDialog,
    pub command_line: super::gui_command_line::CommandLineDialog,
    pub run_config: super::gui_run_config_dialog::RunConfigDialog,
}

impl DialogManager {
    pub fn new() -> Self {
        Self {
            open:     None,
            search:   SearchDialog::new(),
            settings: SettingsDialog::new(),
            picker:   super::gui_goto_dialogs::PickerDialog::new(),
            command_line: super::gui_command_line::CommandLineDialog::new(),
            run_config: super::gui_run_config_dialog::RunConfigDialog::new(),
        }
    }

    pub fn is_open(&self) -> bool { self.open.is_some() }

    pub fn open_dialog(&mut self, kind: DialogKind) {
        if self.open == Some(kind) {
            self.open = None; // toggle
        } else {
            self.open = Some(kind);
            self.picker.reset();
            self.command_line.reset();
            if kind == DialogKind::RunConfig {
                self.run_config.reset();
            }
            if kind == DialogKind::Search {
                self.search.query.clear();
                self.search.last_query.clear();
                self.search.results.clear();
                self.search.selected = 0;
            }
        }
    }

    /// Open the run-configuration editor pre-loaded with the active config.
    pub fn open_run_config_editor(&mut self, ed: &Editor) {
        if self.open == Some(DialogKind::RunConfig) {
            self.open = None;
        } else {
            self.open = Some(DialogKind::RunConfig);
            self.run_config.load_from(ed);
        }
    }

    pub fn close(&mut self) { self.open = None; }

    /// Handle a quit request from ctrl-q or the window close button.
    /// With dirty buffers, opens the confirmation dialog and returns false
    /// so the caller can veto the close; otherwise runs the guarded `quit`
    /// command and returns true.
    pub fn request_quit(&mut self, ed: &mut Editor) -> bool {
        if !ed.dirty_buffer_names().is_empty() {
            if self.open != Some(DialogKind::QuitConfirm) {
                self.open = Some(DialogKind::QuitConfirm);
            }
            return false;
        }
        let _ = crate::kernel::command::execute_command(ed, "quit", &HashMap::new());
        true
    }

    /// Map a GUI key string to the dialog it opens, if any.
    ///
    /// These are the ONLY keys the GUI takes from the keymap, and only in
    /// normal/visual mode — text modes (insert/replace) keep every chord
    /// for the keymap, so vim's insert ctrl-p still reaches completion.
    pub fn shortcut(key: &str) -> Option<DialogKind> {
        match key {
            "ctrl-p"         => Some(DialogKind::CommandPalette),
            "alt-x"          => Some(DialogKind::CommandLine),
            "ctrl-shift-f"   => Some(DialogKind::Search),
            "ctrl-shift-n"   => Some(DialogKind::GotoFile),
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
                    DialogKind::CommandPalette => close = self.picker.show(ui, editor, theme, PickerSource::Everywhere, &mut actions),
                    DialogKind::CommandLine    => close = self.command_line.show(ui, editor, theme, &mut actions),
                    DialogKind::RunConfig      => close = self.run_config.show(ui, editor, theme, &mut actions),
                    DialogKind::QuitConfirm    => {
                        let names = editor.dirty_buffer_names().join(", ");
                        ui.label(RichText::new(format!("Unsaved changes in: {names}")).color(theme.text));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Save all, quit").clicked() {
                                actions.push(DialogAction::ExecuteCommand("save-all".into()));
                                actions.push(DialogAction::ExecuteCommand("quit".into()));
                                close = true;
                            }
                            if ui.button("Discard, quit").clicked() {
                                actions.push(DialogAction::ExecuteCommand("force-quit".into()));
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    }
                }
            });

        if close { self.open = None; }
        actions
    }
}
