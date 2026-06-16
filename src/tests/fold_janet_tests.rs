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

fn focused_key(ed: &Editor) -> usize {
    ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0)
}

// ── buffer/fold ───────────────────────────────────────────────────────

#[test]
fn buffer_fold_adds_fold_range() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("line1\nline2\nline3\n");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(
        &format!("(buffer/fold {} 0 12)", key));
    assert_eq!(result, "ok");

    let folds = &ed.buffers.get(key).unwrap().folds;
    assert_eq!(folds.len(), 1);
    assert_eq!(folds[0], (0, 12));
}

#[test]
fn buffer_unfold_removes_fold_range() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("line1\nline2\nline3\n");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let _ = janet_bridge::eval(
        &format!("(buffer/fold {} 0 12)", key));
    let result = janet_bridge::eval(
        &format!("(buffer/unfold {} 0 12)", key));
    assert_eq!(result, "ok");

    let folds = &ed.buffers.get(key).unwrap().folds;
    assert!(folds.is_empty());
}

#[test]
fn buffer_unfold_all_removes_all_folds() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("line1\nline2\nline3\n");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let _ = janet_bridge::eval(
        &format!("(buffer/fold {} 0 6)", key));
    let _ = janet_bridge::eval(
        &format!("(buffer/fold {} 12 18)", key));
    let result = janet_bridge::eval(
        &format!("(buffer/unfold-all {})", key));
    assert_eq!(result, "ok");

    let folds = &ed.buffers.get(key).unwrap().folds;
    assert!(folds.is_empty());
}

#[test]
fn buffer_folds_returns_empty_for_no_folds() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(
        &format!("(buffer/folds {})", key));
    assert_eq!(result, "ok");
}
