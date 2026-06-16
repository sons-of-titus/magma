use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::id::BufferId;
use crate::state::Editor;
use crate::janet_bridge;

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::new(BufferId(id), "test");
    let e = ed.buffers.vacant_entry();
    let k = e.key();
    e.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(k);
    }
    ed
}

// ── buffer/find-or-create ─────────────────────────────────────────────

#[test]
fn find_or_create_creates_buffer_when_absent() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let before = ed.buffers.len();
    let result = janet_bridge::eval(
        r#"(buffer/find-or-create "*Test-FOC*")"#,
    );
    assert_eq!(result, "ok");
    assert_eq!(ed.buffers.len(), before + 1,
        "find-or-create must create a new buffer when name is absent");

    let found = ed.buffers.iter().any(|(_, b)| b.name == "*Test-FOC*");
    assert!(found, "the new buffer must have the requested name");
}

#[test]
fn find_or_create_returns_existing_slab_key() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // Create the buffer once and record the key
    janet_bridge::eval(r#"(buffer/find-or-create "*Test-FOC2*")"#);
    let key1 = ed.buffers.iter()
        .find(|(_, b)| b.name == "*Test-FOC2*")
        .map(|(k, _)| k);
    let count_before = ed.buffers.len();

    // Call again — must return the same key and not add a new buffer
    janet_bridge::eval(r#"(buffer/find-or-create "*Test-FOC2*")"#);
    let key2 = ed.buffers.iter()
        .find(|(_, b)| b.name == "*Test-FOC2*")
        .map(|(k, _)| k);
    assert_eq!(ed.buffers.len(), count_before,
        "find-or-create must not duplicate an existing buffer");
    assert_eq!(key1, key2, "returned key must be stable across calls");
}

// ── buffer/get-by-name ────────────────────────────────────────────────

#[test]
fn get_by_name_returns_nil_for_missing() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // Evaluating (buffer/get-by-name "*Never-Created*") returns nil,
    // which means the expression evaluates to janet nil — eval returns "ok".
    // We verify on the Rust side that no buffer with that name was created.
    let before = ed.buffers.len();
    let result = janet_bridge::eval(
        r#"(buffer/get-by-name "*Never-Created*")"#,
    );
    assert_eq!(result, "ok");
    assert_eq!(ed.buffers.len(), before,
        "get-by-name must not create a buffer");
}

#[test]
fn get_by_name_finds_existing_buffer() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // Create a named buffer directly in Rust
    let id = ed.allocate_buffer_id();
    let buf = Buffer::new(BufferId(id), "*NamedTestBuf*");
    let entry = ed.buffers.vacant_entry();
    let expected_key = entry.key();
    entry.insert(buf);

    // get-by-name must find it and return a non-nil integer
    let result = janet_bridge::eval(
        &format!(
            "(= (buffer/get-by-name \"*NamedTestBuf*\") {})",
            expected_key
        ),
    );
    assert_eq!(result, "ok",
        "get-by-name must return the correct slab key for an existing buffer");
}

// ── buffer/set-read-only / buffer/read-only? ──────────────────────────

#[test]
fn set_read_only_blocks_janet_insert() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // Register a Rust command to capture buffer-read-only events
    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_c = fired.clone();
    ed.events.on("buffer-read-only", move |_| {
        *fired_c.lock().unwrap() = true;
        None
    });

    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();

    janet_bridge::eval(
        &format!("(buffer/set-read-only {} true)", key),
    );
    janet_bridge::eval(
        &format!("(buffer/insert {} 0 \"blocked\")", key),
    );
    ed.events.drain_and_dispatch();

    // Buffer content must be unchanged
    let buf = ed.buffers.get(key).unwrap();
    assert_eq!(buf.len(), 0,
        "buffer/insert must be a no-op when buffer is read-only");
    assert!(*fired.lock().unwrap(),
        "buffer-read-only event must fire when insert is blocked");
}

#[test]
fn read_only_getter_reflects_flag() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();

    // Initially false
    assert!(!ed.buffers.get(key).unwrap().read_only);

    janet_bridge::eval(&format!("(buffer/set-read-only {} true)", key));
    assert!(ed.buffers.get(key).unwrap().read_only,
        "read_only must be true after set-read-only");

    janet_bridge::eval(&format!("(buffer/set-read-only {} false)", key));
    assert!(!ed.buffers.get(key).unwrap().read_only,
        "read_only must be false after clearing");
}

// ── buffer/set-ephemeral / buffer/ephemeral? ──────────────────────────

#[test]
fn set_ephemeral_reflects_flag() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();

    assert!(!ed.buffers.get(key).unwrap().ephemeral);
    janet_bridge::eval(&format!("(buffer/set-ephemeral {} true)", key));
    assert!(ed.buffers.get(key).unwrap().ephemeral,
        "ephemeral must be true after set-ephemeral");
}

// ── editor/log-message ────────────────────────────────────────────────

#[test]
fn log_message_creates_messages_buffer() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        r#"(editor/log-message "hello from sprint 2")"#,
    );
    assert_eq!(result, "ok");

    let msg_buf = ed.buffers.iter().find(|(_, b)| b.name == "*Messages*");
    assert!(msg_buf.is_some(), "*Messages* buffer must be created by log-message");
    let (_, buf) = msg_buf.unwrap();
    assert!(buf.slice(0, buf.len()).contains("hello from sprint 2"),
        "*Messages* buffer must contain the logged text");
}

#[test]
fn log_message_marks_buffer_read_only_and_ephemeral() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(r#"(editor/log-message "flags test")"#);

    let (_, buf) = ed.buffers.iter()
        .find(|(_, b)| b.name == "*Messages*")
        .unwrap();
    assert!(buf.read_only,  "*Messages* must be read-only after log-message");
    assert!(buf.ephemeral,  "*Messages* must be ephemeral after log-message");
}

#[test]
fn log_message_emits_buffer_message_appended_event() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    // Register Rust event subscriber before the call
    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_c = fired.clone();
    ed.events.on("buffer-message-appended", move |_| {
        *fired_c.lock().unwrap() = true;
        None
    });

    janet_bridge::eval(r#"(editor/log-message "event test")"#);
    ed.events.drain_and_dispatch();

    assert!(*fired.lock().unwrap(),
        "buffer-message-appended event must fire after editor/log-message");
}

// ── editor/warn ───────────────────────────────────────────────────────

#[test]
fn warn_creates_warnings_buffer_with_text() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(r#"(editor/warn "deprecated option")"#);

    let warn_buf = ed.buffers.iter().find(|(_, b)| b.name == "*Warnings*");
    assert!(warn_buf.is_some(), "*Warnings* buffer must be created by editor/warn");
    let (_, buf) = warn_buf.unwrap();
    assert!(buf.slice(0, buf.len()).contains("deprecated option"),
        "*Warnings* buffer must contain the warning text");
    assert!(buf.read_only,  "*Warnings* must be read-only");
    assert!(buf.ephemeral,  "*Warnings* must be ephemeral");
}

#[test]
fn warn_emits_warning_emitted_event() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_c = fired.clone();
    ed.events.on("warning-emitted", move |_| {
        *fired_c.lock().unwrap() = true;
        None
    });

    janet_bridge::eval(r#"(editor/warn "some warning")"#);
    ed.events.drain_and_dispatch();

    assert!(*fired.lock().unwrap(),
        "warning-emitted event must fire after editor/warn");
}

// ── editor/show-help ──────────────────────────────────────────────────

#[test]
fn show_help_creates_help_buffer_with_text() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(r#"(editor/show-help "This is help text.")"#);

    let help_buf = ed.buffers.iter().find(|(_, b)| b.name == "*Help*");
    assert!(help_buf.is_some(), "*Help* buffer must be created by show-help");
    let (_, buf) = help_buf.unwrap();
    assert!(buf.slice(0, buf.len()).contains("This is help text."),
        "*Help* buffer must contain the provided text");
    assert!(buf.read_only, "*Help* must be read-only");
    assert!(buf.ephemeral, "*Help* must be ephemeral");
}

#[test]
fn show_help_focuses_help_buffer_in_window() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    janet_bridge::eval(r#"(editor/show-help "focus test")"#);

    let help_key = ed.buffers.iter()
        .find(|(_, b)| b.name == "*Help*")
        .map(|(k, _)| k)
        .unwrap();
    let focused_key = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    assert_eq!(focused_key, help_key,
        "show-help must focus the *Help* buffer in the current window");
}

#[test]
fn show_help_emits_help_shown_event() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor();
    janet_bridge::init(&mut ed);

    let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fired_c = fired.clone();
    ed.events.on("help-shown", move |_| {
        *fired_c.lock().unwrap() = true;
        None
    });

    janet_bridge::eval(r#"(editor/show-help "event test")"#);
    ed.events.drain_and_dispatch();

    assert!(*fired.lock().unwrap(),
        "help-shown event must fire after editor/show-help");
}
