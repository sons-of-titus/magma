use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::id::BufferId;
use crate::state::Editor;
use crate::janet_bridge;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
    ed
}

// ── editor/set-cursor-shape ───────────────────────────────────────────

#[test]
fn editor_set_cursor_shape_block() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        "(editor/set-cursor-shape \"block\")");
    assert_eq!(result, "ok");
    assert_eq!(ed.cursor_shape, "block");
}

#[test]
fn editor_set_cursor_shape_beam() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        "(editor/set-cursor-shape \"beam\")");
    assert_eq!(result, "ok");
    assert_eq!(ed.cursor_shape, "beam");
}

#[test]
fn editor_set_cursor_shape_underline() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        "(editor/set-cursor-shape \"underline\")");
    assert_eq!(result, "ok");
    assert_eq!(ed.cursor_shape, "underline");
}

#[test]
fn editor_cursor_shape_returns_current() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        "(do (editor/set-cursor-shape \"beam\") (editor/cursor-shape))");
    assert_eq!(result, "ok");
    assert_eq!(ed.cursor_shape, "beam");
}
