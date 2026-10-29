//! Window lifecycle: the launcher ⇄ editor hand-off.
//!
//! One of the two windows is always mapped.  Closing the editor ends the
//! project and returns to the launcher — the process only exits when the
//! launcher itself is closed, or through `quit`.  Quitting with unsaved
//! buffers raises the unsaved-changes dialog (save / cancel / quit anyway)
//! instead of refusing outright.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::execute_command;
use kernel::state::MessageKind;
use kernel::workspace;
use slint::{CloseRequestResponse, ComponentHandle};

use crate::sync;
use crate::{AppState, AppWindow};

pub(crate) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    let weak_app = app.as_weak();
    app.window().on_close_requested(move || {
        let mut s = state.borrow_mut();
        if !s.editor.dirty_buffer_names().is_empty() {
            show_confirm(&weak_app, &mut s);
            return CloseRequestResponse::KeepWindowShown;
        }
        workspace::persist_session(&s.editor);
        workspace::close_editor_session(&mut s.editor);
        // Refresh maps the launcher before the editor window hides: the
        // event loop quits as soon as no window is visible.
        sync::refresh(&weak_app, &mut s);
        CloseRequestResponse::HideWindow
    });
}

/// Raise the unsaved-changes dialog over the editor window.
pub(crate) fn show_confirm(weak: &slint::Weak<AppWindow>, s: &mut AppState) {
    s.quit_confirm_visible = true;
    s.editor.frame_dirty = true;
    sync::refresh(weak, s);
}

/// Dialog button routing: `quit:save` / `quit:cancel` / `quit:force`.
pub(crate) fn confirm_command(weak: &slint::Weak<AppWindow>, s: &mut AppState, command: &str) {
    match command {
        "quit:save" => {
            if let Err(e) = execute_command(&mut s.editor, "save-all", &HashMap::new()) {
                s.editor.show_message(MessageKind::Error, e);
            }
            if s.editor.dirty_buffer_names().is_empty() {
                // Synchronous-write path (no background pool): quit now.
                s.quit_confirm_visible = false;
                let _ = execute_command(&mut s.editor, "quit", &HashMap::new());
            } else {
                // Writes are in flight (or unpathed buffers remain); the
                // frame timer quits once the dirty set drains.
                s.quit_when_clean = true;
            }
        }
        "quit:force" => {
            s.quit_confirm_visible = false;
            s.quit_when_clean = false;
            let _ = execute_command(&mut s.editor, "force-quit", &HashMap::new());
        }
        _ => {
            s.quit_confirm_visible = false;
            s.quit_when_clean = false;
        }
    }
    s.editor.frame_dirty = true;
    sync::refresh(weak, s);
}
