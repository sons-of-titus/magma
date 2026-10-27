//! Reproduce the palette path: DialogAction::ExecuteCommand("terminal-start")
//! is just execute_command with empty args — ground truth for whether the
//! command works outside the colon path.  Terminals are dock sessions: the
//! contract is a focused manager session and *no* buffer or pane takeover.

use std::collections::HashMap;

use crate::kernel::command::{self, builtin};
use crate::kernel::runtime::BackgroundHandle;
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;

fn editor_with_background() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer("*scratch*");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(BackgroundHandle::new(rt, tx));
    ed
}

fn assert_docked_session(ed: &mut Editor, buffers_before: usize, focused_before: Option<usize>) {
    assert_eq!(ed.editor_mode.name, "terminal", "terminal mode engaged");

    let id = ed.io.terminals.focused.expect("a session holds the keyboard");
    assert!(ed.io.terminals.get(id).is_some(), "session lives in the manager");

    assert_eq!(ed.buffers.len(), buffers_before, "no terminal buffer created");
    let focused_now = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w));
    assert_eq!(focused_now, focused_before, "the focused pane keeps its buffer");

    // Release restores normal editing and cleans up the child process.
    command::execute_command(ed, "exit-terminal-mode", &HashMap::new()).unwrap();
    assert_eq!(ed.editor_mode.name, "normal");
    assert!(ed.io.terminals.focused.is_none());
    ed.io.terminals.close(id);
}

#[test]
fn terminal_start_via_command_spawns_docked_session_not_buffer() {
    let mut ed = editor_with_background();
    let buffers_before = ed.buffers.len();
    let focused_before = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w));

    command::execute_command(&mut ed, "terminal-start", &HashMap::new())
        .expect("terminal-start failed");

    assert_docked_session(&mut ed, buffers_before, focused_before);
}

#[test]
fn terminal_start_with_custom_shell_command() {
    let mut ed = editor_with_background();
    let mut args = HashMap::new();
    args.insert("shell".to_string(),
        crate::kernel::command::args::ArgValue::String("/bin/sh".to_string()));
    command::execute_command(&mut ed, "terminal-start", &args).expect("terminal-start failed");
    let id = ed.io.terminals.focused.expect("session focused");
    let session = ed.io.terminals.get(id).expect("session exists");
    assert_eq!(session.title, "sh 1", "default names read \"shell index\"");
    command::execute_command(&mut ed, "exit-terminal-mode", &HashMap::new()).unwrap();
    ed.io.terminals.close(id);
}
