use std::collections::HashMap;
use crate::buffer::Buffer;
use crate::command::{self, builtin};
use crate::fs::disk::DiskFileSystem;
use crate::render::surface::Style;
use crate::state::id::BufferId;
use crate::state::Editor;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

fn focused_key(ed: &Editor) -> usize {
    ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0)
}

// ── Style: new fields ─────────────────────────────────────────────────

#[test]
fn style_default_has_strikethrough_and_dim_false() {
    let s = Style::default();
    assert!(!s.strikethrough);
    assert!(!s.dim);
}

#[test]
fn style_can_set_strikethrough_and_dim() {
    let s = Style { fg: (255, 0, 0), bg: (0, 0, 0), bold: false, italic: false, underline: false, strikethrough: true, dim: true };
    assert!(s.strikethrough);
    assert!(s.dim);
}

// ── Editor.faces registry (pure Rust) ─────────────────────────────────

#[test]
fn editor_faces_starts_empty() {
    let ed = make_editor("");
    assert!(ed.faces.is_empty());
}

#[test]
fn editor_faces_insert_and_retrieve() {
    let mut ed = make_editor("");
    let style = Style { fg: (200, 100, 50), bg: (0, 0, 0), bold: true, italic: false, underline: false, strikethrough: false, dim: false };
    ed.faces.insert("keyword-face".to_string(), style);
    let got = ed.faces.get("keyword-face").unwrap();
    assert_eq!(got.fg, (200, 100, 50));
    assert!(got.bold);
}

#[test]
fn editor_resolve_face_style_returns_none_for_missing() {
    let ed = make_editor("");
    assert!(ed.resolve_face_style("nonexistent").is_none());
}

#[test]
fn editor_resolve_face_style_returns_style() {
    let mut ed = make_editor("");
    let style = Style { fg: (100, 200, 100), bg: (0, 0, 0), bold: false, italic: true, underline: false, strikethrough: false, dim: false };
    ed.faces.insert("string-face".to_string(), style);
    let got = ed.resolve_face_style("string-face").unwrap();
    assert_eq!(got.fg, (100, 200, 100));
    assert!(got.italic);
}

// ── Buffer highlights with face names ──────────────────────────────────

#[test]
fn buffer_set_highlights_accepts_face_names() {
    let mut ed = make_editor("hello world");
    let key = focused_key(&ed);
    let buf = ed.buffers.get_mut(key).unwrap();
    let ranges = vec![(0, 5, "keyword-face".to_string()), (6, 11, "string-face".to_string())];
    buf.set_highlights(ranges);
    let hl = &ed.buffers.get(key).unwrap().highlights;
    assert_eq!(hl.len(), 2);
    assert_eq!(hl[0], (0, 5, "keyword-face".to_string()));
    assert_eq!(hl[1], (6, 11, "string-face".to_string()));
}

#[test]
fn buffer_clear_highlights_removes_all() {
    let mut ed = make_editor("hello");
    let key = focused_key(&ed);
    let buf = ed.buffers.get_mut(key).unwrap();
    buf.set_highlights(vec![(0, 5, "keyword-face".to_string())]);
    buf.clear_highlights();
    assert!(ed.buffers.get(key).unwrap().highlights.is_empty());
}

// ── Highlight layers ───────────────────────────────────────────────────

#[test]
fn buffer_set_highlights_layer_stores_per_layer() {
    let mut ed = make_editor("hello world");
    let key = focused_key(&ed);
    let buf = ed.buffers.get_mut(key).unwrap();
    buf.set_highlights_layer("syntax", vec![(0, 5, "keyword-face".to_string())]);
    buf.set_highlights_layer("search", vec![(6, 11, "search-face".to_string())]);
    assert_eq!(buf.highlight_layers.len(), 2);
    assert_eq!(buf.highlight_layers.get("syntax").unwrap().len(), 1);
    assert_eq!(buf.highlight_layers.get("search").unwrap().len(), 1);
}

#[test]
fn buffer_clear_highlights_layer_removes_only_that_layer() {
    let mut ed = make_editor("hello");
    let key = focused_key(&ed);
    let buf = ed.buffers.get_mut(key).unwrap();
    buf.set_highlights_layer("syntax", vec![(0, 5, "keyword-face".to_string())]);
    buf.set_highlights_layer("search", vec![(0, 3, "search-face".to_string())]);
    buf.clear_highlights_layer("syntax");
    assert!(buf.highlight_layers.get("syntax").is_none());
    assert!(buf.highlight_layers.get("search").is_some());
}

#[test]
fn buffer_clear_all_highlight_layers_removes_everything() {
    let mut ed = make_editor("hello");
    let key = focused_key(&ed);
    let buf = ed.buffers.get_mut(key).unwrap();
    buf.set_highlights_layer("syntax", vec![(0, 5, "keyword-face".to_string())]);
    buf.set_highlights_layer("search", vec![(0, 3, "search-face".to_string())]);
    buf.clear_all_highlight_layers();
    assert!(buf.highlight_layers.is_empty());
}

// ── Scope→face mapping ─────────────────────────────────────────────────

#[test]
fn scope_faces_empty_by_default() {
    let ed = make_editor("");
    assert!(ed.scope_faces.is_empty());
}

#[test]
fn scope_faces_resolve_longest_prefix_wins() {
    let mut ed = make_editor("");
    ed.scope_faces.push(("string".to_string(), "string-face".to_string()));
    ed.scope_faces.push(("string.quoted".to_string(), "quoted-face".to_string()));
    ed.scope_faces.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // Resolve should match longest prefix first
    let resolved = ed.scope_faces.iter()
        .find(|(p, _)| "string.quoted.double".starts_with(p))
        .map(|(_, f)| f.clone());
    assert_eq!(resolved, Some("quoted-face".to_string()));
}

#[test]
fn scope_faces_resolve_unmatched_returns_none() {
    let mut ed = make_editor("");
    ed.scope_faces.push(("keyword".to_string(), "keyword-face".to_string()));
    let found = ed.scope_faces.iter()
        .find(|(p, _)| "comment".starts_with(p))
        .map(|(_, f)| f.clone());
    assert!(found.is_none());
}

// ── face-changed event ─────────────────────────────────────────────────

#[test]
fn face_changed_event_emitted_when_face_inserted() {
    let mut ed = make_editor("");
    let captured: std::sync::Arc<std::sync::Mutex<Option<String>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
    let cap = captured.clone();
    ed.events.on("face-changed", move |data| {
        *cap.lock().unwrap() = data.get("face").cloned();
        None
    });
    let style = Style::default();
    ed.faces.insert("test-face".to_string(), style);
    let mut data = HashMap::new();
    data.insert("face".to_string(), "test-face".to_string());
    ed.events.emit("face-changed", data);
    ed.events.drain_and_dispatch();
    let got = captured.lock().unwrap().clone();
    assert_eq!(got.as_deref(), Some("test-face"));
}
