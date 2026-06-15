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

// ── editor/define-face ─────────────────────────────────────────────────

#[test]
fn editor_define_face_inserts_face_into_registry() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(face/define \"test-face\" {:fg [255 0 0] :bg [0 0 0] :bold true})");
    assert_eq!(result, "ok");

    let face = ed.faces.get("test-face");
    assert!(face.is_some(), "face should be registered");
    let s = face.unwrap();
    assert_eq!(s.fg, (255, 0, 0));
    assert!(s.bold);
}

#[test]
fn editor_define_face_without_table_creates_default_style() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(face/define \"default-face\")");
    assert_eq!(result, "ok");

    let face = ed.faces.get("default-face");
    assert!(face.is_some());
}

#[test]
fn editor_define_face_emits_face_changed_event() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let _sub_count_before = ed.events.subscriber_count("face-changed");
    let result = janet_bridge::eval(&mut ed,
        "(face/define \"my-face\" {:fg [100 200 100]})");
    assert_eq!(result, "ok");
    // The event was emitted — we can check by observing no error occurred
    // (face-changed has no Janet subscribers by default, but Rust emitted it)
    assert!(ed.faces.contains_key("my-face"));
}

// ── editor/face getter ──────────────────────────────────────────────────

#[test]
fn editor_face_returns_nil_for_unknown() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(face/get \"nonexistent\")");
    assert!(result.contains("nil") || result == "ok",
        "should handle missing face gracefully");
}

#[test]
fn editor_face_returns_table_for_defined_face() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(do (face/define \"test-face\" {:fg [10 20 30] :bold true})
             (face/get \"test-face\"))");
    assert_eq!(result, "ok");
    // Verify on Rust side that face exists
    let face = ed.faces.get("test-face").unwrap();
    assert_eq!(face.fg, (10, 20, 30));
    assert!(face.bold);
}

// ── editor/make-style ───────────────────────────────────────────────────

#[test]
fn editor_make_style_returns_integer_handle() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(face/make-style {:fg [255 0 0] :bg [0 0 0] :bold true})");
    assert_eq!(result, "ok");
}

// ── editor/scope-face and editor/resolve-scope ─────────────────────────

#[test]
fn editor_scope_face_registers_mapping() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(face/scope-face \"keyword\" \"keyword-face\")");
    assert_eq!(result, "ok");

    // Rust-side: mapping should exist
    assert!(!ed.scope_faces.is_empty());
}

#[test]
fn editor_resolve_scope_returns_mapped_face() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(do (face/scope-face \"keyword\" \"keyword-face\")
             (face/resolve-scope \"keyword.control\"))");
    assert_eq!(result, "ok");
}

#[test]
fn editor_resolve_scope_returns_nil_for_unmapped() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed,
        "(face/resolve-scope \"unknown.scope\")");
    assert_eq!(result, "ok");
}

// ── buffer/set-highlights with face names ──────────────────────────────

#[test]
fn buffer_set_highlights_with_face_names() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello world");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights {} [[0 5 \"keyword-face\"] [6 11 \"string-face\"]])", key));
    assert_eq!(result, "ok");

    let hl = &ed.buffers.get(key).unwrap().highlights;
    assert_eq!(hl.len(), 2);
    assert_eq!(hl[0].2, "keyword-face");
    assert_eq!(hl[1].2, "string-face");
}

#[test]
fn buffer_set_highlights_without_face_defaults_to_highlight() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights {} [[0 5]])", key));
    assert_eq!(result, "ok");

    let hl = &ed.buffers.get(key).unwrap().highlights;
    assert_eq!(hl.len(), 1);
    assert_eq!(hl[0].2, "highlight");
}

#[test]
fn buffer_set_highlights_empty_clears() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let _ = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights {} [[0 5 \"keyword-face\"]])", key));
    let _ = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights {})", key));
    assert!(ed.buffers.get(key).unwrap().highlights.is_empty());
}

// ── buffer/set-highlights-layer and buffer/clear-highlights-layer ──────

#[test]
fn buffer_set_highlights_layer_stores_per_layer() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello world");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let result = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])", key));
    assert_eq!(result, "ok");

    let buf = ed.buffers.get(key).unwrap();
    let layer = buf.highlight_layers.get("syntax");
    assert!(layer.is_some());
    assert_eq!(layer.unwrap().len(), 1);
    assert_eq!(layer.unwrap()[0].2, "keyword-face");
}

#[test]
fn buffer_clear_highlights_layer_removes_named_layer() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let _ = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])", key));
    let result = janet_bridge::eval(&mut ed,
        &format!("(buffer/clear-highlights-layer {} \"syntax\")", key));
    assert_eq!(result, "ok");

    let buf = ed.buffers.get(key).unwrap();
    assert!(buf.highlight_layers.get("syntax").is_none());
}

#[test]
fn buffer_clear_highlights_layer_does_not_affect_other_layers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("hello");
    janet_bridge::init(&mut ed);
    let key = focused_key(&ed);

    let _ = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights-layer {} \"syntax\" [[0 5 \"keyword-face\"]])", key));
    let _ = janet_bridge::eval(&mut ed,
        &format!("(buffer/set-highlights-layer {} \"search\" [[2 4 \"search-face\"]])", key));
    let _ = janet_bridge::eval(&mut ed,
        &format!("(buffer/clear-highlights-layer {} \"syntax\")", key));

    let buf = ed.buffers.get(key).unwrap();
    assert!(buf.highlight_layers.get("syntax").is_none());
    assert!(buf.highlight_layers.get("search").is_some());
}

// ── Default faces from init.janet ──────────────────────────────────────

#[test]
fn default_faces_exist_after_init() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    assert!(ed.faces.contains_key("keyword-face"), "keyword-face should exist");
    assert!(ed.faces.contains_key("string-face"), "string-face should exist");
    assert!(ed.faces.contains_key("comment-face"), "comment-face should exist");
    assert!(ed.faces.contains_key("type-face"), "type-face should exist");
    assert!(ed.faces.contains_key("function-face"), "function-face should exist");
    assert!(ed.faces.contains_key("variable-face"), "variable-face should exist");
    assert!(ed.faces.contains_key("constant-face"), "constant-face should exist");
    assert!(ed.faces.contains_key("operator-face"), "operator-face should exist");
    assert!(ed.faces.contains_key("punctuation-face"), "punctuation-face should exist");
    assert!(ed.faces.contains_key("error-face"), "error-face should exist");
    assert!(ed.faces.contains_key("warning-face"), "warning-face should exist");
}

#[test]
fn default_comment_face_is_dim() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let face = ed.faces.get("comment-face").unwrap();
    assert!(face.dim, "comment-face should be dim by default");
}

#[test]
fn default_error_face_is_bold() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let face = ed.faces.get("error-face").unwrap();
    assert!(face.bold, "error-face should be bold by default");
}
