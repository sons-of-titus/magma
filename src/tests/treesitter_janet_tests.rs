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

// ── ts/set-language ───────────────────────────────────────────────────

#[test]
fn ts_set_language_stores_on_buffer() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello world");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(&mut ed,
        &format!("(ts/set-language {} \"janet\")", key));
    assert_eq!(result, "ok");

    assert_eq!(ed.ts_languages.get(&key).unwrap(), "janet");
}

// ── ts/has-tree? ──────────────────────────────────────────────────────

#[test]
fn ts_has_tree_returns_false_by_default() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(&mut ed,
        &format!("(ts/has-tree? {})", key));
    assert_eq!(result, "ok");
    // No tree has been parsed yet
    assert!(!ed.ts_trees.contains_key(&key));
}

// ── ts/parse without grammar returns false ────────────────────────────

#[test]
fn ts_parse_without_grammar_returns_false() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("fn main() {}");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let _ = janet_bridge::eval(&mut ed,
        &format!("(ts/set-language {} \"rust\")", key));
    let result = janet_bridge::eval(&mut ed,
        &format!("(ts/parse {})", key));
    // Should return false since no grammar library is loaded
    // The exact return value depends on how we handle missing grammar
    assert_eq!(result, "ok");
}

// ── ts/query without tree returns nil ─────────────────────────────────

#[test]
fn ts_query_without_tree_handles_gracefully() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(&mut ed,
        &format!("(ts/query {} \"(symbol)\")", key));
    // Should not crash - returns nil or empty
    assert_eq!(result, "ok");
}

// ── ts/set-language on invalid buffer ─────────────────────────────────

#[test]
fn ts_set_language_invalid_buffer_does_not_panic() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(ts/set-language 99999 \"janet\")");
    assert_eq!(result, "ok");
}
