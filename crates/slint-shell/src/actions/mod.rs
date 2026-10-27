//! Callback wiring: Slint signals → commands and state changes.
//!
//! Every component callback lands here.  Actions go through the command
//! registry whenever one exists, so the Slint shell behaves exactly like the
//! keymap, the menu, and the egui chrome.  Wiring is grouped by surface:
//! editor input, docks, the agent tool window, and dialogs/settings.

mod agent;
mod dock;
mod editor;
mod settings;

use std::cell::RefCell;
use std::rc::Rc;

use crate::AppState;

pub(crate) fn install(app: &crate::AppWindow, state: Rc<RefCell<AppState>>) {
    editor::install(&app, Rc::clone(&state));
    dock::install(&app, Rc::clone(&state));
    agent::install(&app, Rc::clone(&state));
    settings::install(&app, Rc::clone(&state));
}
