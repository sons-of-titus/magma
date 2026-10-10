use std::collections::HashMap;

use serde_json::json;

use crate::kernel::semantic::lsp;
use crate::kernel::semantic::lsp_proto;
use crate::tests::helpers;

// ── Wire protocol ────────────────────────────────────────────────────────

#[test]
fn test_parse_uri_to_path() {
    assert_eq!(lsp_proto::uri_to_path("file:///home/user/file.rs"), "/home/user/file.rs");
    assert_eq!(lsp_proto::uri_to_path("/raw/path"), "/raw/path");
}

#[test]
fn test_publish_diagnostics_typed() {
    let body = json!({"uri": "file:///test.rs", "diagnostics": [
        {"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":1}},"severity":1,"message":"error msg"},
        {"range":{"start":{"line":2,"character":5},"end":{"line":2,"character":10}},"severity":2,"message":"warn msg"},
        {"range":{"start":{"line":3,"character":0},"end":{"line":3,"character":1}},"severity":4,"message":"hint msg"}
    ]});
    let diags = lsp::parse_diagnostics(&body);
    assert_eq!(diags.len(), 3);
    assert_eq!(diags[0].severity.as_str(), "error");
    assert_eq!(diags[0].line, 0);
    assert_eq!(diags[0].end_line, 1);
    assert_eq!(diags[1].severity.as_str(), "warning");
    assert_eq!(diags[1].column, 5);
    assert_eq!(diags[2].severity.as_str(), "hint");
}

#[test]
fn test_publish_diagnostics_empty() {
    let body = json!({"uri": "file:///empty.rs", "diagnostics": []});
    assert!(lsp::parse_diagnostics(&body).is_empty());
}

#[test]
fn test_hover_text_markdown() {
    let result = json!({"contents": {"kind": "markdown", "value": "# Title\n\nBody text"}});
    let text = lsp::extract_hover_text(Some(&result));
    assert!(text.contains("Title"));
    assert!(text.contains("Body"));
}

#[test]
fn test_hover_text_plain_and_null() {
    let result = json!({"contents": "plain string"});
    assert_eq!(lsp::extract_hover_text(Some(&result)), "plain string");
    assert_eq!(lsp::extract_hover_text(Some(&serde_json::Value::Null)), "");
}

#[test]
fn test_definition_single_location() {
    let result = json!({"uri": "file:///src/lib.rs",
        "range": {"start": {"line": 10, "character": 5}, "end": {"line": 10, "character": 20}}});
    let (uri, sl, sc, el, ec) = lsp::first_location(&result).unwrap();
    assert_eq!(uri, "file:///src/lib.rs");
    assert_eq!((sl, sc, el, ec), (10, 5, 10, 20));
}

#[test]
fn test_definition_array_of_locations() {
    let result = json!([
        {"uri": "file:///a.rs", "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 2}}},
        {"uri": "file:///b.rs", "range": {"start": {"line": 9, "character": 0}, "end": {"line": 9, "character": 2}}}
    ]);
    let (uri, sl, _sc, _el, _ec) = lsp::first_location(&result).unwrap();
    assert_eq!(uri, "file:///a.rs", "the first location wins");
    assert_eq!(sl, 1);
}

#[test]
fn test_definition_null_is_none() {
    assert!(lsp::first_location(&serde_json::Value::Null).is_none());
}

// ── BackgroundEvent LSP variants emit correct events ─────────────────────

#[test]
fn test_lsp_hover_event_dispatch() {
    let mut ed = helpers::make_editor_with_buffer("hello world");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-hover", move |data| {
        *cap.lock().unwrap() = data.get("contents").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("path".into(), "test".into());
    data.insert("contents".into(), "hover info".into());
    ed.events.emit("lsp-hover", data);
    ed.events.drain_and_dispatch();

    let contents = captured.lock().unwrap().take();
    assert_eq!(contents, Some("hover info".into()));
}

#[test]
fn test_lsp_definition_event_dispatch() {
    let mut ed = helpers::make_editor_with_buffer("test");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-definition", move |data| {
        *cap.lock().unwrap() = data.get("uri").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("uri".into(), "/target.rs".into());
    ed.events.emit("lsp-definition", data);
    ed.events.drain_and_dispatch();

    assert_eq!(captured.lock().unwrap().take(), Some("/target.rs".into()));
}

#[test]
fn test_lsp_code_actions_event_dispatch() {
    let mut ed = helpers::make_editor_with_buffer("test");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-code-actions", move |data| {
        *cap.lock().unwrap() = data.get("actions").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("actions".into(), "[]".into());
    ed.events.emit("lsp-code-actions", data);
    ed.events.drain_and_dispatch();

    assert_eq!(captured.lock().unwrap().take(), Some("[]".into()));
}

#[test]
fn test_lsp_completion_items_event_dispatch() {
    let mut ed = helpers::make_editor_with_buffer("test");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-completion-items", move |data| {
        *cap.lock().unwrap() = data.get("items").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("items".into(), r#"["a","b"]"#.into());
    ed.events.emit("lsp-completion-items", data);
    ed.events.drain_and_dispatch();

    assert_eq!(captured.lock().unwrap().take(), Some(r#"["a","b"]"#.into()));
}

#[test]
fn test_lsp_rename_result_event() {
    let mut ed = helpers::make_editor_with_buffer("test");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-rename-result", move |data| {
        *cap.lock().unwrap() = data.get("edit").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("edit".into(), "{}".into());
    ed.events.emit("lsp-rename-result", data);
    ed.events.drain_and_dispatch();

    assert_eq!(captured.lock().unwrap().take(), Some("{}".into()));
}

#[test]
fn test_lsp_progress_event() {
    let mut ed = helpers::make_editor_with_buffer("test");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-progress", move |data| {
        *cap.lock().unwrap() = data.get("token").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("token".into(), "test-progress".into());
    ed.events.emit("lsp-progress", data);
    ed.events.drain_and_dispatch();

    assert_eq!(captured.lock().unwrap().take(), Some("test-progress".into()));
}

#[test]
fn test_lsp_response_event() {
    let mut ed = helpers::make_editor_with_buffer("test");
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let cap = captured.clone();
    ed.events.on("lsp-response", move |data| {
        *cap.lock().unwrap() = data.get("method").cloned();
        None
    });

    let mut data = HashMap::new();
    data.insert("method".into(), "custom/method".into());
    data.insert("result".into(), "{}".into());
    ed.events.emit("lsp-response", data);
    ed.events.drain_and_dispatch();

    assert_eq!(captured.lock().unwrap().take(), Some("custom/method".into()));
}

// ── Request shaping ──────────────────────────────────────────────────────

#[test]
fn test_build_request_format() {
    let req = lsp_proto::request(1, "textDocument/hover", json!({"pos": {}}));
    assert_eq!(req["id"], 1);
    assert_eq!(req["method"], "textDocument/hover");
    assert_eq!(req["params"]["pos"], json!({}));
}

#[test]
fn test_initialize_params_shape() {
    let params = lsp_proto::initialize_params(Some("/Users/me/my proj"));
    assert_eq!(params["rootUri"], "file:///Users/me/my%20proj");
    let params = lsp_proto::initialize_params(None);
    assert!(params["rootUri"].is_null());
}
