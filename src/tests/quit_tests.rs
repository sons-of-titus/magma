//! Quit safety — `quit` protects unsaved work; overrides are explicit.

use std::collections::HashMap;

use crate::kernel::command::{self, builtin};
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::state::Editor;

fn editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

fn focus(ed: &mut Editor, key: usize) {
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
}

fn edit(ed: &mut Editor, key: usize, at: usize, text: &str) {
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(at, text);
}

fn run(ed: &mut Editor, cmd: &str) -> Result<(), String> {
    command::execute_command(ed, cmd, &HashMap::new())
}

#[test]
fn fresh_buffers_are_clean() {
    let mut ed = editor();
    let loaded = ed.create_buffer_from_str("file.rs", "content");
    let empty = ed.create_buffer("*scratch*");
    assert!(!ed.buffers.get(loaded).unwrap().lock().unwrap().modified(),
        "loaded content is the saved baseline");
    assert!(!ed.buffers.get(empty).unwrap().lock().unwrap().modified());
}

#[test]
fn quit_refuses_and_names_dirty_buffers() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("hand.rs", "content");
    focus(&mut ed, key);
    edit(&mut ed, key, 0, "x");

    let err = run(&mut ed, "quit").unwrap_err();
    assert!(err.contains("Unsaved changes"), "got: {err}");
    assert!(err.contains("hand.rs"), "got: {err}");
    assert!(ed.running, "refusal must keep the editor alive");
}

#[test]
fn quit_succeeds_when_clean() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("clean.rs", "content");
    focus(&mut ed, key);
    run(&mut ed, "quit").unwrap();
    assert!(!ed.running);
}

#[test]
fn quit_ignores_read_only_buffers() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("*shell*", "generated");
    focus(&mut ed, key);
    edit(&mut ed, key, 0, "x");
    ed.buffers.get(key).unwrap().lock().unwrap().read_only = true;
    run(&mut ed, "quit").unwrap();
    assert!(!ed.running);
}

#[test]
fn force_quit_ignores_dirty_buffers() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("dirty.rs", "content");
    focus(&mut ed, key);
    edit(&mut ed, key, 0, "x");
    run(&mut ed, "force-quit").unwrap();
    assert!(!ed.running);
}

#[test]
fn refused_quit_arms_the_confirm_flag() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("hand.rs", "content");
    focus(&mut ed, key);
    edit(&mut ed, key, 0, "x");
    let _ = run(&mut ed, "quit");
    assert!(ed.quit_refused, "refusal must arm the frontend's confirm dialog");
    assert!(ed.running);
}

#[test]
fn successful_quit_disarms_the_confirm_flag() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("hand.rs", "content");
    focus(&mut ed, key);
    edit(&mut ed, key, 0, "x");
    let _ = run(&mut ed, "quit");
    assert!(ed.quit_refused);
    // Save through the buffer's own path so the write stays synchronous.
    ed.buffers
        .get_mut(key)
        .unwrap()
        .lock()
        .unwrap()
        .mark_saved();
    run(&mut ed, "quit").unwrap();
    assert!(!ed.quit_refused, "a later clean quit must disarm the dialog");
    assert!(!ed.running);
}

#[test]
fn force_quit_leaves_the_confirm_flag_alone() {
    let mut ed = editor();
    let key = ed.create_buffer_from_str("dirty.rs", "content");
    focus(&mut ed, key);
    edit(&mut ed, key, 0, "x");
    run(&mut ed, "force-quit").unwrap();
    assert!(!ed.quit_refused);
    assert!(!ed.running);
}

#[test]
fn save_all_writes_pathed_dirty_buffers_and_leaves_unpathed() {
    let dir = std::env::temp_dir().join(format!("magma-save-all-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path_a = dir.join("a.txt");
    let path_b = dir.join("b.txt");

    let mut ed = editor();
    let a = ed.create_buffer_from_str("a.txt", "alpha");
    ed.buffers.get_mut(a).unwrap().lock().unwrap().path = Some(path_a.to_str().unwrap().to_string());
    let b = ed.create_buffer_from_str("b.txt", "beta");
    ed.buffers.get_mut(b).unwrap().lock().unwrap().path = Some(path_b.to_str().unwrap().to_string());
    let scratch = ed.create_buffer_from_str("*scratch*", "typed");
    edit(&mut ed, a, 0, "A-");
    edit(&mut ed, b, 0, "B-");
    edit(&mut ed, scratch, 0, "S-");

    run(&mut ed, "save-all").unwrap();
    assert_eq!(std::fs::read_to_string(&path_a).unwrap(), "A-alpha");
    assert_eq!(std::fs::read_to_string(&path_b).unwrap(), "B-beta");
    assert!(!ed.buffers.get(a).unwrap().lock().unwrap().modified());
    assert!(!ed.buffers.get(b).unwrap().lock().unwrap().modified());
    assert!(ed.buffers.get(scratch).unwrap().lock().unwrap().modified(),
        "no path to write: still unsaved");

    let err = run(&mut ed, "quit").unwrap_err();
    assert!(err.contains("*scratch*"), "got: {err}");
    std::fs::remove_dir_all(&dir).ok();
}
