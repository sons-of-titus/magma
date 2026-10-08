//! Reproduce the palette path: DialogAction::ExecuteCommand("terminal-start")
//! is just execute_command with empty args — ground truth for whether the
//! command works outside the colon path.

use std::collections::HashMap;

use crate::kernel::command::{self, builtin};
use crate::kernel::render::gui_dialog::DialogAction;
use crate::kernel::runtime::BackgroundHandle;
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;

fn editor_with_background() -> (Editor, tokio::sync::mpsc::UnboundedReceiver<crate::kernel::runtime::BackgroundEvent>) {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer("*scratch*");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(BackgroundHandle::new(rt, tx));
    (ed, rx)
}

#[test]
fn terminal_start_via_dialog_action_attaches_and_focuses() {
    let (mut ed, _rx) = editor_with_background();

    // Exactly what the palette's ExecuteCommand action runs.
    let result = command::execute_command(&mut ed, "terminal-start", &HashMap::new());
    assert!(result.is_ok(), "terminal-start failed: {:?}", result.err());

    let term_buf = ed.io.terminal_mode_buf.expect("terminal mode engaged");
    assert_eq!(ed.editor_mode.name, "terminal");

    // The terminal buffer must be on screen, not a background buffer.
    let focused = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w));
    assert_eq!(focused, Some(term_buf), "terminal buffer must be focused");

    // Same via the dialog action wrapper — the exact palette code path.
    let (mut ed, _rx) = editor_with_background();
    DialogAction::ExecuteCommand("terminal-start".into()).apply(&mut ed);
    assert!(ed.io.terminal_mode_buf.is_some(), "dialog action must engage terminal mode");
    assert_eq!(ed.editor_mode.name, "terminal");
}
