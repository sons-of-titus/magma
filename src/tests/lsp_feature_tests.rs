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

// ── Server hover text reaches the hover card ─────────────────────────

#[test]
fn lsp_hover_splices_into_visible_card() {
    use crate::kernel::runtime::{process_background_event, BackgroundEvent};
    use crate::kernel::state::{HoverCard, MessageKind};
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("fn foo() {}");
    let key = helpers::focused_key(&ed);
    ed.hover = Some(HoverCard {
        word: "foo".into(), px: 10.0, py: 10.0,
        since: std::time::Instant::now(),
        lines: vec!["function foo  ·  lib.rs:1".into()],
    });
    let rev = ed.buffers.get(key).unwrap().lock().unwrap().revision();

    process_background_event(&mut ed, BackgroundEvent::LspHover {
        path: "rust".into(),
        contents: "```rust\nfn foo()\n```\ndoes a thing".into(),
        buf_key: Some(key),
        revision: rev,
    });

    let card = ed.hover.as_ref().expect("card survives the splice");
    assert!(card.lines.iter().take(2).any(|l| l.contains("fn foo()")),
        "server text leads the card: {:?}", card.lines);
    assert!(card.lines.iter().any(|l| l.contains("lib.rs:1")),
        "index lines follow: {:?}", card.lines);
    assert!(ed.status_message.is_none(), "no status spam while a card is up");
    let _ = MessageKind::Info;
}

#[test]
fn lsp_hover_without_card_becomes_a_status_message() {
    use crate::kernel::runtime::{process_background_event, BackgroundEvent};
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("fn foo() {}");
    let key = helpers::focused_key(&ed);
    ed.hover = None; // manual lsp-hover with no pointer dwell
    let rev = ed.buffers.get(key).unwrap().lock().unwrap().revision();

    process_background_event(&mut ed, BackgroundEvent::LspHover {
        path: "rust".into(),
        contents: "fn foo() — does a thing".into(),
        buf_key: Some(key),
        revision: rev,
    });
    let msg = ed.status_message.as_ref().expect("manual hover surfaces");
    assert!(msg.text.contains("does a thing"));
}

#[test]
fn stale_lsp_hover_is_dropped() {
    use crate::kernel::runtime::{process_background_event, BackgroundEvent};
    use crate::kernel::state::HoverCard;
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("fn foo() {}");
    let key = helpers::focused_key(&ed);
    ed.hover = Some(HoverCard {
        word: "foo".into(), px: 0.0, py: 0.0,
        since: std::time::Instant::now(),
        lines: vec!["index only".into()],
    });

    process_background_event(&mut ed, BackgroundEvent::LspHover {
        path: "rust".into(),
        contents: "stale!".into(),
        buf_key: Some(key),
        revision: 999_999, // buffer has moved on
    });
    let card = ed.hover.as_ref().unwrap();
    assert!(card.lines.iter().all(|l| !l.contains("stale")),
        "responses for superseded revisions never render");
}

// ── Code actions → quick-fix picker ──────────────────────────────────

#[test]
fn code_actions_response_opens_the_picker() {
    use crate::kernel::runtime::{process_background_event, BackgroundEvent};
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("let x = 1;");
    let key = helpers::focused_key(&ed);
    let rev = ed.buffers.get(key).unwrap().lock().unwrap().revision();

    process_background_event(&mut ed, BackgroundEvent::LspCodeActions {
        path: "rust".into(),
        actions: r#"[
            {"title":"Qualify `x`","kind":"quickfix","edit":{"changes":{}}},
            {"title":"Inline variable","kind":"refactor.inline"}
        ]"#.into(),
        buf_key: Some(key),
        revision: rev,
    });

    let menu = ed.quickfix_menu.as_ref().expect("picker opened");
    assert_eq!(menu.buf_key, key);
    assert_eq!(menu.items.len(), 2);
    assert_eq!(menu.items[0].title, "Qualify `x`");
    assert!(menu.items[0].edit_json.is_some());
    assert!(menu.items[1].edit_json.is_none());
}

#[test]
fn picker_accept_applies_the_workspace_edit() {
    use crate::kernel::input::dispatch_key;
    use crate::kernel::state::overlay::{parse_code_actions, QuickfixMenu};
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("let x = 1;");
    let key = helpers::focused_key(&ed);
    // Rewrite the line via a full-range edit on this untitled buffer: the
    // applier matches buffers by path, so give the buffer a path first.
    {
        let arc = ed.buffers.get_mut(key).unwrap();
        arc.lock().unwrap().path = Some("/tmp/qf_action.rs".to_string());
    }
    let actions = parse_code_actions(r#"[{"title":"Rename","kind":"quickfix","edit":{
        "changes":{"/tmp/qf_action.rs":[{"range":{
            "start":{"line":0,"character":0},"end":{"line":0,"character":10}},
            "newText":"let y = 2;"}]}}}]"#);
    ed.quickfix_menu = Some(QuickfixMenu { buf_key: key, items: actions, idx: 0 });

    dispatch_key(&mut ed, "down"); // no-op wrap on a 1-item menu
    dispatch_key(&mut ed, "return");

    assert!(ed.quickfix_menu.is_none(), "accept closes the picker");
    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert_eq!(text, "let y = 2;", "the workspace edit was applied");
}

#[test]
fn server_apply_edit_request_applies_and_answers() {
    use crate::kernel::runtime::{process_background_event, BackgroundEvent};
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("let x = 1;");
    let key = helpers::focused_key(&ed);
    {
        let arc = ed.buffers.get_mut(key).unwrap();
        arc.lock().unwrap().path = Some("/tmp/qf_apply_edit.rs".to_string());
    }

    process_background_event(&mut ed, BackgroundEvent::LspApplyEdit {
        language: "rust".into(),
        request_id: 7,
        edit_json: r#"{"changes":{"/tmp/qf_apply_edit.rs":[{"range":{
            "start":{"line":0,"character":0},"end":{"line":0,"character":10}},
            "newText":"let z = 3;"}]}}"#.into(),
    });

    let text = ed.buffers.get(key).unwrap().lock().unwrap().slice(0, 20);
    assert_eq!(text, "let z = 3;", "server-pushed edit applied");
    // No server for "rust" in this editor: respond() must be a no-op.
    assert!(ed.status_message.is_none());
}

#[test]
fn stale_code_actions_never_open_the_picker() {
    use crate::kernel::runtime::{process_background_event, BackgroundEvent};
    use crate::tests::helpers;

    let mut ed = helpers::make_editor_with_buffer("let x = 1;");
    let key = helpers::focused_key(&ed);
    process_background_event(&mut ed, BackgroundEvent::LspCodeActions {
        path: "rust".into(),
        actions: r#"[{"title":"stale","kind":"quickfix"}]"#.into(),
        buf_key: Some(key),
        revision: 999_999,
    });
    assert!(ed.quickfix_menu.is_none(), "superseded responses are dropped");
}

// ── Cancellation errors are not failures ─────────────────────────────────

/// Regression: servers answer superseded requests with ContentModified /
/// ServerCancelled.  Treating those as errors turned ordinary typing into a
/// wall of `[lsp/error]` lines.
#[test]
fn cancellation_codes_are_recognised() {
    for code in [-32800, -32801, -32802] {
        assert!(lsp::is_cancellation(&json!({"code": code, "message": "content modified"})),
            "code {code} is a cancellation");
    }
}

#[test]
fn real_errors_are_not_cancellations() {
    for code in [-32700, -32600, -32601, -32602, -32603, -32000] {
        assert!(!lsp::is_cancellation(&json!({"code": code, "message": "boom"})),
            "code {code} is a genuine error");
    }
    // A malformed error without a numeric code must not be swallowed.
    assert!(!lsp::is_cancellation(&json!({"message": "no code"})));
}
