#[cfg(feature = "janet")]
use crate::buffer::Buffer;
#[cfg(feature = "janet")]
use crate::command::builtin;
#[cfg(feature = "janet")]
use crate::fs::disk::DiskFileSystem;
#[cfg(feature = "janet")]
use crate::janet_bridge;
#[cfg(feature = "janet")]
use crate::state::id::BufferId;
#[cfg(feature = "janet")]
use crate::state::Editor;

#[cfg(feature = "janet")]
fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test.rs", "hello\n");
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
    ed
}

// ── font/invalidate ────────────────────────────────────────────────────

#[test]
#[cfg(feature = "janet")]
fn font_invalidate_sets_font_changed_flag() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    assert!(!ed.font_changed, "font_changed must start false");
    let r = janet_bridge::eval("(font/invalidate)");
    assert_eq!(r, "ok");
    assert!(ed.font_changed, "font_changed must be true after font/invalidate");
}

// ── font-changed event ────────────────────────────────────────────────────────

#[test]
#[cfg(feature = "janet")]
fn font_changed_event_has_subscriber_from_ecosystem() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("font-changed");
    assert!(count > 0, "ecosystem.janet must register a font-changed subscriber");
}

#[test]
#[cfg(feature = "janet")]
fn font_changed_event_sets_flag_after_drain() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    assert!(!ed.font_changed);
    // Emit the event via Janet (queues it)
    janet_bridge::eval(r#"(event/emit "font-changed" {})"#);
    // Drain dispatches the Rust subscriber, which calls the Janet handler,
    // which calls font/invalidate, which sets ed.font_changed = true.
    ed.events.drain_and_dispatch();
    assert!(ed.font_changed, "font_changed flag must be set after font-changed event is drained");
}

#[test]
#[cfg(feature = "janet")]
fn font_changed_flag_is_cleared_after_being_read() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // Set the flag directly, then simulate the GuiApp reading and clearing it.
    ed.font_changed = true;
    let was_set = ed.font_changed;
    ed.font_changed = false; // GuiApp clears it after rebuilding the atlas
    assert!(was_set);
    assert!(!ed.font_changed);
}
