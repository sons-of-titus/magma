use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::kernel::command;
use crate::tests::helpers;

// ── buffer-created carries path field ─────────────────────────────────

#[test]
fn buffer_created_event_includes_path() {
    let mut ed = helpers::make_editor_with_buffer("");
    let tmp = std::env::temp_dir().join("sprint1_open.txt");
    std::fs::write(&tmp, "hello").unwrap();
    let path = tmp.to_string_lossy().to_string();

    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    ed.events.on("buffer-created", move |data| {
        *cap.lock().unwrap() = data.get("path").cloned();
        None
    });

    let mut args = HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(path.clone()));
    command::execute_command(&mut ed, "open-file", &args).unwrap();
    ed.events.drain_and_dispatch();

    let got = captured.lock().unwrap().clone();
    assert_eq!(got.as_deref(), Some(path.as_str()),
        "buffer-created event must include the path field");

    std::fs::remove_file(&tmp).ok();
}

// ── buffer-before-save / buffer-after-save ────────────────────────────

#[test]
fn save_buffer_emits_before_and_after_save() {
    let mut ed = helpers::make_editor_with_buffer("content");
    let tmp = std::env::temp_dir().join("sprint1_save.txt");
    let path = tmp.to_string_lossy().to_string();
    let key = helpers::focused_key(&ed);
    ed.buffers.get(key).unwrap().lock().unwrap().path = Some(path.clone());

    let before_fired: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let after_fired:  Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let b = before_fired.clone();
    let a = after_fired.clone();
    ed.events.on("buffer-before-save", move |_| { *b.lock().unwrap() = true; None });
    ed.events.on("buffer-after-save",  move |_| { *a.lock().unwrap() = true; None });

    helpers::run(&mut ed, "save-buffer");
    ed.events.drain_and_dispatch();

    assert!(*before_fired.lock().unwrap(), "buffer-before-save must fire");
    assert!(*after_fired.lock().unwrap(),  "buffer-after-save must fire");

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn save_buffer_no_longer_emits_buffer_saved() {
    let mut ed = helpers::make_editor_with_buffer("x");
    let tmp = std::env::temp_dir().join("sprint1_nosaved.txt");
    let path = tmp.to_string_lossy().to_string();
    let key = helpers::focused_key(&ed);
    ed.buffers.get(key).unwrap().lock().unwrap().path = Some(path.clone());

    let old_fired: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let o = old_fired.clone();
    ed.events.on("buffer-saved", move |_| { *o.lock().unwrap() = true; None });

    helpers::run(&mut ed, "save-buffer");
    ed.events.drain_and_dispatch();

    assert!(!*old_fired.lock().unwrap(),
        "buffer-saved must not be emitted (replaced by buffer-after-save)");
    std::fs::remove_file(&tmp).ok();
}

// ── buffer-focused from navigation commands ───────────────────────────

fn make_editor_two_buffers() -> crate::kernel::state::Editor {
    let mut ed = helpers::make_editor();
    ed.create_buffer_from_str("buf2", "bbb");
    ed
}

#[test]
fn buffer_next_emits_buffer_focused() {
    let mut ed = make_editor_two_buffers();
    let fired: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let f = fired.clone();
    ed.events.on("buffer-focused", move |data| {
        *f.lock().unwrap() = data.get("buffer-id").cloned();
        None
    });
    helpers::run(&mut ed, "buffer-next");
    ed.events.drain_and_dispatch();
    assert!(fired.lock().unwrap().is_some(), "buffer-focused must fire on buffer-next");
}

#[test]
fn buffer_prev_emits_buffer_focused() {
    let mut ed = make_editor_two_buffers();
    let fired: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let f = fired.clone();
    ed.events.on("buffer-focused", move |_| { *f.lock().unwrap() = true; None });
    helpers::run(&mut ed, "buffer-prev");
    ed.events.drain_and_dispatch();
    assert!(*fired.lock().unwrap(), "buffer-focused must fire on buffer-prev");
}

#[test]
fn alternate_buffer_emits_buffer_focused() {
    let mut ed = make_editor_two_buffers();
    // prime alternate by switching once
    helpers::run(&mut ed, "buffer-next");
    ed.events.drain_and_dispatch();
    let fired: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let f = fired.clone();
    ed.events.on("buffer-focused", move |_| { *f.lock().unwrap() = true; None });
    helpers::run(&mut ed, "alternate-buffer");
    ed.events.drain_and_dispatch();
    assert!(*fired.lock().unwrap(), "buffer-focused must fire on alternate-buffer");
}

// ── close-buffer command ──────────────────────────────────────────────

#[test]
fn close_buffer_emits_buffer_closed_and_removes() {
    let mut ed = make_editor_two_buffers();
    let key = helpers::focused_key(&ed);
    let closed_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let c = closed_id.clone();
    ed.events.on("buffer-closed", move |data| {
        *c.lock().unwrap() = data.get("buffer-id").cloned();
        None
    });
    helpers::run(&mut ed, "close-buffer");
    ed.events.drain_and_dispatch();

    assert!(closed_id.lock().unwrap().is_some(), "buffer-closed must fire");
    assert_eq!(closed_id.lock().unwrap().as_deref(),
        Some(key.to_string().as_str()),
        "buffer-closed must report the correct buffer-id");
    assert!(!ed.buffers.contains(key), "buffer must be removed from slab after close-buffer");
}

#[test]
fn close_buffer_switches_focused_window_when_current() {
    let mut ed = make_editor_two_buffers();
    let old_key = helpers::focused_key(&ed);
    helpers::run(&mut ed, "close-buffer");
    ed.events.drain_and_dispatch();
    let new_key = helpers::focused_key(&ed);
    assert_ne!(new_key, old_key, "focused window must switch away from closed buffer");
    assert!(ed.buffers.contains(new_key), "new focused buffer must exist");
}

// ── Buffer.local_options ──────────────────────────────────────────────

#[test]
fn local_options_are_isolated_from_global() {
    let mut ed = helpers::make_editor_with_buffer("");
    ed.options.insert("tab-width".to_string(), "4".to_string());

    let key = helpers::focused_key(&ed);
    ed.buffers.get(key).unwrap().lock().unwrap()
        .local_options.insert("tab-width".to_string(), "2".to_string());

    let local_val = ed.buffers.get(key).unwrap().lock().unwrap()
        .local_options.get("tab-width").cloned();
    let global_val = ed.options.get("tab-width").cloned();

    assert_eq!(local_val.as_deref(), Some("2"), "local option must be 2");
    assert_eq!(global_val.as_deref(), Some("4"), "global option must remain 4");
}

#[test]
fn local_options_absent_by_default() {
    let ed = helpers::make_editor_with_buffer("");
    let key = helpers::focused_key(&ed);
    assert!(ed.buffers.get(key).unwrap().lock().unwrap().local_options.is_empty(),
        "new buffer must have no local options");
}

// ── buffer/major-mode and per-buffer option API via Janet ─────────────
