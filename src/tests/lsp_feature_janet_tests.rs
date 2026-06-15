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

// ── C function registration ─────────────────────────────────────────────

#[test]
fn lsp_request_c_function_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/request \"test-lang\" \"test/method\" \"{}\")");
    assert_eq!(result, "ok", "lsp/request should return ok");
}

#[test]
fn lsp_hover_c_function_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/hover \"rust\")");
    assert_eq!(result, "ok", "lsp/hover should return ok");
}

#[test]
fn lsp_code_actions_c_function_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/code-actions \"rust\")");
    assert_eq!(result, "ok", "lsp/code-actions should return ok");
}

#[test]
fn lsp_completion_c_function_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/completion \"rust\")");
    assert_eq!(result, "ok", "lsp/completion should return ok");
}

#[test]
fn lsp_rename_c_function_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/rename \"rust\" \"new_name\")");
    assert_eq!(result, "ok", "lsp/rename should return ok");
}

#[test]
fn lsp_apply_edit_c_function_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/apply-edit \"{}\")");
    assert_eq!(result, "ok", "lsp/apply-edit should return ok");
}

#[test]
fn lsp_request_with_params_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(
        &mut ed,
        r#"(lsp/request "rust" "textDocument/hover" "{\"pos\":{}}")"#,
    );
    assert_eq!(result, "ok", "lsp/request with params should return ok");
}

#[test]
fn lsp_start_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/start \"test-lang\" \"echo\" \"arg1\")");
    assert_eq!(result, "ok", "lsp/start should return ok");
}

#[test]
fn lsp_notify_registered() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let result = janet_bridge::eval(&mut ed, "(lsp/notify \"test-lang\" \"test/notification\" \"{}\")");
    assert_eq!(result, "ok", "lsp/notify should return ok");
}

// ── lsp-hover event dispatch ─────────────────────────────────────────────

#[test]
fn lsp_hover_event_dispatches_from_rust() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let mut data = std::collections::HashMap::new();
    data.insert("path".into(), "test".into());
    data.insert("contents".into(), "hover result".into());
    ed.events.emit("lsp-hover", data);
    ed.events.drain_and_dispatch();

    let sub_count = ed.events.subscriber_count("lsp-hover");
    // lsp.janet registers a handler for lsp-hover
    assert!(sub_count > 0, "lsp-hover should have subscribers from lsp.janet");
}

// ── lsp-definition event dispatch ────────────────────────────────────────

#[test]
fn lsp_definition_event_has_subscribers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("lsp-definition");
    assert!(count > 0, "lsp-definition should be subscribed in lsp.janet");
}

// ── lsp-code-actions event dispatch ──────────────────────────────────────

#[test]
fn lsp_code_actions_event_has_subscribers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("lsp-code-actions");
    assert!(count > 0, "lsp-code-actions should be subscribed in lsp.janet");
}

// ── lsp-completion-items event dispatch ──────────────────────────────────

#[test]
fn lsp_completion_items_event_has_subscribers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("lsp-completion-items");
    assert!(count > 0, "lsp-completion-items should be subscribed in lsp.janet");
}

// ── lsp-rename-result event dispatch ─────────────────────────────────────

#[test]
fn lsp_rename_result_event_has_subscribers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("lsp-rename-result");
    assert!(count > 0, "lsp-rename-result should be subscribed in lsp.janet");
}

// ── lsp-progress event dispatch ──────────────────────────────────────────

#[test]
fn lsp_progress_event_has_subscribers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("lsp-progress");
    assert!(count > 0, "lsp-progress should be subscribed in lsp.janet");
}

// ── lsp-response event dispatch ──────────────────────────────────────────

#[test]
fn lsp_response_event_has_subscribers() {
    let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ed = make_editor("");
    janet_bridge::init(&mut ed);

    let count = ed.events.subscriber_count("lsp-response");
    assert!(count > 0, "lsp-response should be subscribed in lsp.janet");
}
