//! Picker surfaces: Go to File, Go to Symbol, the buffer switcher, the
//! command palette, the branch switcher, and plain text prompts (go-to-line,
//! new file/folder, rename).  One overlay, one keyboard contract; the entries
//! are collected from live kernel state at open time and filtered as the user
//! types.  Commands requested from the keymap arrive as `Editor::ui_requests`
//! and are drained into `open` here.

use std::collections::HashMap;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use slint::{ModelRc, VecModel};

use crate::{AppState, AppWindow, PickerRow, sync::refresh};

pub(crate) const PAYLOAD_SEP: char = '\u{1f}';

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PickerKind {
    None,
    Files,
    Symbols,
    Buffers,
    Commands,
    Branches,
    /// Plain numeric prompt.
    GotoLine,
    /// Create a file inside the payload directory.
    NewFile(String),
    /// Create a directory inside the payload directory.
    NewFolder(String),
    /// Rename the payload path to the typed name.
    Rename(String),
    /// Rename the symbol at the cursor via LSP.
    RenameSymbol,
    /// A plugin-registered source: entries come from the kernel's
    /// `picker_state`, collected by a Janet handler per query.
    Source(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PickerItem {
    pub title: String,
    pub detail: String,
    pub meta: String,
    pub payload: String,
}

mod sources;

use sources::{collect_items, header, ranked};

/// Re-filter the entries against the current query and project the rows.
fn refresh_rows(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    let query = s.picker_query.trim().to_string();
    let prompt = matches!(
        s.picker,
        PickerKind::GotoLine
            | PickerKind::NewFile(_)
            | PickerKind::NewFolder(_)
            | PickerKind::Rename(_)
            | PickerKind::RenameSymbol
    );
    let indices = ranked(s, &query, prompt);
    s.picker_selected = s.picker_selected.min(indices.len().saturating_sub(1));
    s.visible_items = indices.clone();
    let rows: Vec<PickerRow> = indices
        .iter()
        .map(|i| {
            let item = &s.picker_items[*i];
            PickerRow {
                title: item.title.clone().into(),
                detail: item.detail.clone().into(),
                meta: item.meta.clone().into(),
            }
        })
        .collect();
    if let Some(a) = weak.upgrade() {
        a.set_picker_selected(s.picker_selected as i32);
        a.set_picker_rows(ModelRc::new(VecModel::from(rows)));
    }
}

/// Re-project rows after the query was preset from outside (rename prefills
/// the old name).
/// Open the picker on a surface, collecting its entries.
pub(crate) fn open(weak: &slint::Weak<AppWindow>, s: &mut AppState, kind: PickerKind) {
    if kind == PickerKind::Branches && s.editor.vc.branches.is_empty() {
        s.message = "No branches — refresh version control first.".into();
        s.editor.frame_dirty = true;
        refresh(weak, s);
        return;
    }
    let (title, placeholder, hint) = header(&kind);
    s.picker = kind.clone();
    s.picker_query.clear();
    // Prefill rename with the symbol under the cursor (JetBrains Shift+F6).
    if matches!(kind, PickerKind::RenameSymbol) {
        let buf_key = kernel::input::focused_buffer_id(&s.editor);
        if let Some(view) = s.editor.view_of_buffer(buf_key) {
            let pos = view.cursor_offset();
            let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let start = buf
                .rfind_char_before(pos, |c| !is_word(c))
                .map(|i| i + 1)
                .unwrap_or(0);
            let end = buf
                .find_char_from(start, |c| !is_word(c))
                .unwrap_or(buf.len());
            if start < end {
                s.picker_query = buf.slice(start, end).to_string();
            }
        }
    }
    s.picker_selected = 0;
    s.picker_items = collect_items(s, &kind);
    if let Some(a) = weak.upgrade() {
        a.set_picker_title(title.into());
        a.set_picker_placeholder(placeholder.into());
        a.set_picker_hint(hint.into());
        a.set_picker_query(s.picker_query.clone().into());
    }
    refresh_rows(weak, s);
}


pub(crate) fn preset(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    refresh_rows(weak, s);
}

pub(crate) fn query_changed(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    if let Some(a) = weak.upgrade() {
        s.picker_query = a.get_picker_query().to_string();
    }
    // Symbols and Commands rank against live state (the index search and
    // the palette's "Run …" row), so their entries are collected per query.
    if matches!(s.picker, PickerKind::Symbols | PickerKind::Commands) {
        let kind = s.picker.clone();
        s.picker_items = collect_items(s, &kind);
    }
    if let PickerKind::Source(id) = s.picker.clone() {
        let query = s.picker_query.clone();
        emit_source_query(s, &id, &query);
    }
    s.picker_selected = 0;
    refresh_rows(weak, s);
}

pub(crate) fn move_selection(weak: &slint::Weak<AppWindow>, s: &mut AppState, delta: i32) {
    let count = s.visible_items.len();
    if count == 0 {
        return;
    }
    let next = (s.picker_selected as i32 + delta).clamp(0, count as i32 - 1);
    s.picker_selected = next as usize;
    if let Some(a) = weak.upgrade() {
        a.set_picker_selected(s.picker_selected as i32);
    }
}

pub(crate) fn row_clicked(weak: &slint::Weak<AppWindow>, s: &mut AppState, row: i32) {
    s.picker_selected = row.max(0) as usize;
    confirm(weak, s);
}

pub(crate) fn cancel(s: &mut AppState) {
    s.picker = PickerKind::None;
    s.picker_items.clear();
    s.visible_items.clear();
    s.picker_query.clear();
    s.editor.frame_dirty = true;
}

pub(crate) fn confirm(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    let kind = s.picker.clone();
    let payload = s
        .visible_items
        .get(s.picker_selected)
        .and_then(|i| s.picker_items.get(*i))
        .map(|item| item.payload.clone());
    match kind {
        PickerKind::None => {}
        PickerKind::Files => {
            if let Some(path) = payload {
                cancel(s);
                run(s, "open-file", ("path", ArgValue::Path(path)));
            }
        }
        PickerKind::Symbols => {
            if let Some(p) = payload {
                let mut parts = p.split(PAYLOAD_SEP);
                let path = parts.next().unwrap_or_default().to_string();
                let line = parts
                    .next()
                    .and_then(|l| l.parse::<i64>().ok())
                    .unwrap_or(1);
                cancel(s);
                run(s, "open-file", ("path", ArgValue::Path(path)));
                run(s, "goto-line", ("line", ArgValue::Integer(line)));
            }
        }
        PickerKind::Buffers => {
            if let Some(key) = payload.and_then(|p| p.parse::<usize>().ok()) {
                cancel(s);
                s.editor.db.active_tab = None;
                if let Some(win) = s.editor.view_tree.focused_window_mut() {
                    win.buffer_id = Some(key);
                }
            }
        }
        PickerKind::Commands => {
            if let Some(line) = payload.clone() {
                cancel(s);
                // The one destructive bypass the old prompt allowed: a
                // force-quit typed into a search-looking surface discarded
                // unsaved work without the guard.  The palette routes it
                // through the same confirmation a refused quit gets.
                if line.trim() == "force-quit" && !s.editor.dirty_buffer_names().is_empty() {
                    s.quit_confirm_visible = true;
                } else {
                    match kernel::command::execute_command_line(&mut s.editor, &line) {
                        Ok(()) => s.message.clear(),
                        // The palette only ever runs a command the registry
                        // or the script runtime knows — never a guess.
                        Err(e) => s.message = e,
                    }
                }
            }
        }
        PickerKind::Source(_) => {
            if let Some(p) = payload {
                if let Some(line) = p.strip_prefix("cmd:") {
                    let line = line.to_string();
                    cancel(s);
                    match kernel::command::execute_command_line(&mut s.editor, &line) {
                        Ok(()) => s.message.clear(),
                        Err(e) => s.message = e,
                    }
                } else {
                    let mut parts = p.split(PAYLOAD_SEP);
                    let path = parts.next().unwrap_or_default().to_string();
                    let line = parts
                        .next()
                        .and_then(|l| l.parse::<i64>().ok())
                        .unwrap_or(1);
                    cancel(s);
                    run(s, "open-file", ("path", ArgValue::Path(path)));
                    run(s, "goto-line", ("line", ArgValue::Integer(line)));
                }
            }
        }
        PickerKind::Branches => {
            if let Some(branch) = payload {
                cancel(s);
                run(s, "vc-checkout-branch", ("branch", ArgValue::String(branch)));
            }
        }
        PickerKind::GotoLine => {
            let text = s.picker_query.trim().to_string();
            cancel(s);
            match text.parse::<i64>() {
                Ok(line) => run(s, "goto-line", ("line", ArgValue::Integer(line))),
                Err(_) => s.message = "Go to line needs a number, like 42.".into(),
            }
        }
        PickerKind::NewFile(parent) => {
            let name = s.picker_query.trim().to_string();
            cancel(s);
            if name.is_empty() {
                s.message = "Name the file first.".into();
            } else {
                let path = join_path(&parent, &name);
                run(s, "create-file", ("path", ArgValue::Path(path)));
                s.tree_epoch = u64::MAX;
            }
        }
        PickerKind::NewFolder(parent) => {
            let name = s.picker_query.trim().to_string();
            let parent = parent.clone();
            cancel(s);
            if name.is_empty() {
                s.message = "Name the folder first.".into();
            } else {
                let path = join_path(&parent, &name);
                match std::fs::create_dir_all(&path) {
                    Ok(()) => {
                        s.message.clear();
                        s.tree_epoch = u64::MAX;
                    }
                    Err(e) => s.message = format!("Cannot create folder: {e}"),
                }
            }
        }
        PickerKind::Rename(from) => {
            let name = s.picker_query.trim().to_string();
            cancel(s);
            if name.is_empty() {
                s.message = "Name it something first.".into();
            } else {
                let parent = std::path::Path::new(&from)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_default();
                let to = join_path(&parent.to_string_lossy(), &name);
                let mut args = HashMap::new();
                args.insert("from".to_string(), ArgValue::Path(from));
                args.insert("to".to_string(), ArgValue::Path(to));
                match execute_command(&mut s.editor, "rename-path", &args) {
                    Ok(()) => {
                        s.message.clear();
                        s.tree_epoch = u64::MAX;
                    }
                    Err(e) => s.message = e,
                }
            }
        }
        PickerKind::RenameSymbol => {
            let name = s.picker_query.trim().to_string();
            cancel(s);
            if name.is_empty() {
                s.message = "Name it something first.".into();
            } else {
                let mut args = HashMap::new();
                args.insert("arg".to_string(), ArgValue::String(name));
                match execute_command(&mut s.editor, "lsp-rename", &args) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                }
            }
        }
    }
    s.editor.frame_dirty = true;
    refresh(weak, s);
}

fn run(s: &mut AppState, command: &str, arg: (&str, ArgValue)) {
    let mut args = HashMap::new();
    args.insert(arg.0.to_string(), arg.1);
    match execute_command(&mut s.editor, command, &args) {
        Ok(()) => s.message.clear(),
        Err(e) => s.message = e,
    }
}

fn join_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

/// Open a plugin-registered source: metadata from the kernel registry,
/// entries collected asynchronously — the query event fires now and the
/// collector's `picker/push-items` answer projects on the following frame.
pub(crate) fn open_source(
    weak: &slint::Weak<AppWindow>,
    s: &mut AppState,
    id: &str,
) {
    let Some(source) = s.editor.picker_sources.get(id).cloned() else {
        s.message = format!("no picker source registered: {id}");
        s.editor.frame_dirty = true;
        return;
    };
    s.picker = PickerKind::Source(id.to_string());
    s.picker_query.clear();
    s.picker_items.clear();
    s.picker_selected = 0;
    if let Some(a) = weak.upgrade() {
        a.set_picker_title(source.title.clone().into());
        a.set_picker_placeholder(source.placeholder.clone().into());
        a.set_picker_hint("".into());
        a.set_picker_query("".into());
        a.set_picker_visible(true);
    }
    emit_source_query(s, id, "");
    refresh_rows(weak, s);
}

/// Ask the collector for the current query; the answer lands in
/// `editor.picker_state` on the next dispatch.
fn emit_source_query(s: &mut AppState, id: &str, query: &str) {
    s.editor.events.emit(
        "picker-source-query",
        [
            ("source".to_string(), id.to_string()),
            ("query".to_string(), query.to_string()),
        ]
        .into_iter()
        .collect(),
    );
}

/// Project the collector's cached answer into the rows — called every
/// frame while a source picker is open, since the answer arrives
/// asynchronously.
pub(crate) fn sync_source_items(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    let PickerKind::Source(id) = s.picker.clone() else {
        return;
    };
    let fresh: Vec<PickerItem> = s
        .editor
        .picker_state
        .items
        .iter()
        .map(|e| PickerItem {
            title: e.title.clone(),
            detail: e.detail.clone(),
            meta: e.meta.clone(),
            payload: e.payload.clone(),
        })
        .collect();
    if fresh != s.picker_items {
        s.picker_items = fresh;
        refresh_rows(weak, s);
    }
    let _ = id;
}

/// Route a `gui-*` ui_request from the keymap to its surface.  Returns true
/// when the request was recognized.
pub(crate) fn handle_request(
    weak: &slint::Weak<AppWindow>,
    s: &mut AppState,
    request: &str,
) -> bool {
    match request {
        "goto-file" => open(weak, s, PickerKind::Files),
        "goto-symbol" => open(weak, s, PickerKind::Symbols),
        "buffers" => open(weak, s, PickerKind::Buffers),
        "commands" => open(weak, s, PickerKind::Commands),
        "branches" => open(weak, s, PickerKind::Branches),
        "goto-line" => open(weak, s, PickerKind::GotoLine),
        "rename-symbol" => open(weak, s, PickerKind::RenameSymbol),
        r if r.starts_with("picker-source:") => {
            open_source(weak, s, &r["picker-source:".len()..])
        }
        "find" => {
            s.find_visible = true;
            s.find_replace_visible = false;
        }
        "replace" => {
            s.find_visible = true;
            s.find_replace_visible = true;
        }
        "project-find" => {
            s.bottom = 1;
        }
        _ => return false,
    }
    true
}
