use std::collections::HashMap;
use crate::lsp::parse;
use crate::state::Editor;
use crate::tests::helpers;

// ── parse.rs unit tests ──────────────────────────────────────────────────

#[test]
fn test_parse_uri_to_path() {
    assert_eq!(parse::uri_to_path("file:///home/user/file.rs"), "/home/user/file.rs");
    assert_eq!(parse::uri_to_path("/raw/path"), "/raw/path");
    assert_eq!(parse::uri_to_path("file://localhost/path/file.txt"), "/path/file.txt");
}

#[test]
fn test_parse_diagnostics_mixed() {
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///test.rs","diagnostics":[
        {"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":1}},"severity":1,"message":"error msg"},
        {"range":{"start":{"line":2,"character":5},"end":{"line":2,"character":10}},"severity":2,"message":"warn msg"},
        {"range":{"start":{"line":3,"character":0},"end":{"line":3,"character":1}},"severity":4,"message":"hint msg"}
    ]}}"#;
    let (path, diags) = parse::parse_diagnostics(body).unwrap();
    assert_eq!(path, "/test.rs");
    assert_eq!(diags.len(), 3);
    assert!(diags[0].contains("[E]"), "expected error: {}", diags[0]);
    assert!(diags[1].contains("[W]"), "expected warning: {}", diags[1]);
    assert!(diags[2].contains("[H]"), "expected hint: {}", diags[2]);
}

#[test]
fn test_parse_diagnostics_empty() {
    let body = r#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":"file:///empty.rs","diagnostics":[]}}"#;
    let (path, diags) = parse::parse_diagnostics(body).unwrap();
    assert_eq!(path, "/empty.rs");
    assert!(diags.is_empty());
}

#[test]
fn test_parse_hover_markdown() {
    let body = r##"{"jsonrpc":"2.0","id":1,"result":{"contents":{"kind":"markdown","value":"# Title\n\nBody text"}}}"##;
    let text = parse::parse_hover(body).unwrap();
    assert!(text.contains("Title"), "expected Title in hover: {text}");
    assert!(text.contains("Body"), "expected Body in hover: {text}");
}

#[test]
fn test_parse_hover_plain() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"contents":"plain string"}}"#;
    assert_eq!(parse::parse_hover(body).unwrap(), "plain string");
}

#[test]
fn test_parse_hover_null() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
    assert!(parse::parse_hover(body).is_none());
}

#[test]
fn test_parse_definition_single() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"uri":"file:///src/lib.rs","range":{"start":{"line":10,"character":5},"end":{"line":10,"character":20}}}}"#;
    let (uri, sl, sc, el, ec) = parse::parse_definition(body).unwrap();
    assert_eq!(uri, "/src/lib.rs");
    assert_eq!(sl, 10);
    assert_eq!(sc, 5);
    assert_eq!(el, 10);
    assert_eq!(ec, 20);
}

#[test]
fn test_parse_definition_array() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":[{"uri":"file:///a.rs","range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}}}]}"#;
    let (uri, sl, sc, el, _ec) = parse::parse_definition(body).unwrap();
    assert_eq!(uri, "/a.rs");
    assert_eq!(sl, 0);
}

#[test]
fn test_parse_definition_null() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
    assert!(parse::parse_definition(body).is_none());
}

#[test]
fn test_parse_code_actions() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":[{"title":"Extract function","kind":"refactor.extract"},{"title":"Add type annotation","kind":"quickfix"}]}"#;
    let actions = parse::parse_code_actions(body).unwrap();
    assert_eq!(actions.len(), 2);
    assert!(actions[0].contains("Extract function"));
    assert!(actions[1].contains("Add type annotation"));
}

#[test]
fn test_parse_code_actions_empty() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":[]}"#;
    assert!(parse::parse_code_actions(body).is_none());
}

#[test]
fn test_parse_completion_items() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":[{"label":"push","detail":"Vec::push"},{"label":"pop","detail":"Vec::pop"}]}"#;
    let items = parse::parse_completion_items(body).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn test_parse_completion_list() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"isIncomplete":false,"items":[{"label":"map","detail":"Iterator::map"}]}}"#;
    let items = parse::parse_completion_items(body).unwrap();
    assert_eq!(items.len(), 1);
}

#[test]
fn test_parse_rename() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"changes":{"/file.rs":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":5}},"newText":"newname"}]}}}"#;
    let edit = parse::parse_rename(body).unwrap();
    assert!(edit.contains("changes"));
    assert!(edit.contains("newname"));
}

#[test]
fn test_parse_rename_null() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
    assert!(parse::parse_rename(body).is_none());
}

#[test]
fn test_parse_progress_begin() {
    let body = r#"{"jsonrpc":"2.0","method":"$/progress","params":{"token":"build-1","value":{"kind":"begin","message":"Building...","percentage":0}}}"#;
    let (token, msg, pct) = parse::parse_progress(body).unwrap();
    assert_eq!(token, "build-1");
    assert_eq!(msg, "Building...");
    assert_eq!(pct, "0");
}

#[test]
fn test_parse_progress_report() {
    let body = r#"{"jsonrpc":"2.0","method":"$/progress","params":{"token":42,"value":{"kind":"report","message":"Indexing","percentage":50}}}"#;
    let (token, msg, pct) = parse::parse_progress(body).unwrap();
    assert_eq!(token, "42");
    assert_eq!(msg, "Indexing");
    assert_eq!(pct, "50");
}

#[test]
fn test_is_response_and_method() {
    assert!(parse::is_response(r#"{"id":1,"result":null}"#));
    assert!(!parse::is_response(r#"{"method":"test"}"#));
}

#[test]
fn test_extract_error() {
    let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
    let err = parse::extract_error(body).unwrap();
    assert!(err.contains("Method not found"));
}

#[test]
fn test_has_error() {
    assert!(parse::has_error(r#"{"id":1,"error":{"code":-1,"message":"err"}}"#));
    assert!(!parse::has_error(r#"{"id":1,"result":null}"#));
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

// ── Build_request formatting ─────────────────────────────────────────────

#[test]
fn test_build_request_format() {
    let req = parse::build_request(1, "textDocument/hover", r#"{"pos":{}}"#);
    assert!(req.contains(r#""id":1"#));
    assert!(req.contains(r#""method":"textDocument/hover""#));
    assert!(req.contains(r#""params":{"pos":{}}"#));
}
