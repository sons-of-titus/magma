//! Status message surface — the editor always answers.

use crate::kernel::command::builtin;
use crate::kernel::render::gui_dialog::DialogAction;
use crate::kernel::state::{Editor, MessageKind};
use crate::kernel::storage::disk::DiskFileSystem;

fn editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

#[test]
fn failed_dialog_command_sets_error_message() {
    let mut ed = editor();
    DialogAction::ExecuteCommand("no-such-command".into()).apply(&mut ed);
    let msg = ed.status_message.expect("a failed dialog action must answer");
    assert_eq!(msg.kind, MessageKind::Error);
    assert!(!msg.text.is_empty());
}

#[test]
fn failed_dialog_open_file_reports_missing_path() {
    let mut ed = editor();
    DialogAction::OpenFile("/nonexistent/magma-missing-file.rs".into()).apply(&mut ed);
    let msg = ed.status_message.expect("open-file failure must answer");
    assert_eq!(msg.kind, MessageKind::Error);
    assert!(msg.text.contains("nonexistent"), "got: {}", msg.text);
}

#[test]
fn successful_dialog_action_leaves_message_clear() {
    let mut ed = editor();
    DialogAction::ExecuteCommand("save-all".into()).apply(&mut ed);
    assert!(ed.status_message.is_none(), "success is silent; no news is good news");
}

#[test]
fn failed_keybound_command_sets_error_message() {
    let mut ed = editor();
    ed.keymaps.push_layer("editor");
    ed.keymaps.set_layer("editor", "zzz", "no-such-command");
    crate::kernel::input::dispatch_key(&mut ed, "zzz");
    let msg = ed.status_message.expect("keybound failure must answer");
    assert_eq!(msg.kind, MessageKind::Error);
}

#[test]
fn message_kinds_are_distinct() {
    let mut ed = editor();
    ed.show_message(MessageKind::Info, "hello");
    assert_eq!(ed.status_message.as_ref().unwrap().kind, MessageKind::Info);
    ed.show_message(MessageKind::Error, "boom");
    assert_eq!(ed.status_message.as_ref().unwrap().kind, MessageKind::Error);
}
