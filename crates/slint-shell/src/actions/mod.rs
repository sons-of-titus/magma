//! Callback wiring: Slint signals → commands and state changes.
//!
//! Every component callback lands here.  Actions go through the command
//! registry whenever one exists, so the Slint shell behaves exactly like the
//! keymap, the menu, and the egui chrome.  Wiring is grouped by surface:
//! editor input, docks, the agent tool window, and dialogs/settings.

mod agent;
mod db;
mod debug;
mod dock;
mod editor;
pub(crate) mod fonts;
mod settings;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;

use crate::AppState;

pub(crate) fn install(app: &crate::AppWindow, state: Rc<RefCell<AppState>>) {
    editor::install(&app, Rc::clone(&state));
    dock::install(&app, Rc::clone(&state));
    db::install(&app, Rc::clone(&state));
    debug::install(&app, Rc::clone(&state));
    agent::install(&app, Rc::clone(&state));
    settings::install(&app, Rc::clone(&state));
}

/// Folder-picker → `open-project`.  Shared by the editor's menu action and
/// the launcher window's Open Folder button.  Callers refresh afterwards.
pub(crate) fn open_project_dialog(s: &mut AppState) {
    let Some(dir) = rfd::FileDialog::new().pick_folder() else {
        return;
    };
    open_project_path(s, &dir.to_string_lossy());
}

/// `open-project` with a path, reporting failures on the status message.
pub(crate) fn open_project_path(s: &mut AppState, path: &str) {
    let mut args = HashMap::new();
    args.insert("path".to_string(), ArgValue::Path(path.to_string()));
    match execute_command(&mut s.editor, "open-project", &args) {
        Ok(()) => s.message.clear(),
        Err(e) => s.message = e,
    }
    s.tree_epoch = u64::MAX;
}
