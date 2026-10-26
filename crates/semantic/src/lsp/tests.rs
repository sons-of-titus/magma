//! Client-level tests: delivery through the sink, response correlation,
//! cancellation, EOF, startup failure, and ordered shutdown.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::{Value, json};

use super::protocol;
use super::{Core, Event, EventSink, LspHandle, LspRegistry, Pending, ServerState};
use crate::lsp_proto;

#[derive(Clone, Default)]
struct Collect(Arc<Mutex<Vec<Event>>>);

impl Collect {
    fn new() -> Self {
        Self::default()
    }

    fn events(&self) -> Vec<Event> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl EventSink for Collect {
    fn send(&self, event: Event) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(event);
    }
}

type Signal = std::sync::mpsc::Receiver<Result<(), String>>;
type Outbound = tokio::sync::mpsc::UnboundedReceiver<Value>;

fn core() -> (Arc<Core>, Signal, Outbound) {
    let (outbound, outbound_rx) = tokio::sync::mpsc::unbounded_channel();
    let (init_tx, init_rx) = std::sync::mpsc::channel();
    let core = Arc::new(Core {
        outbound,
        pending: Mutex::new(HashMap::new()),
        state: Mutex::new(ServerState::Initializing),
        next_id: AtomicU64::new(1),
        init_signal: Mutex::new(init_tx),
    });
    (core, init_rx, outbound_rx)
}

fn add_pending(core: &Core, id: u64, method: &str) {
    core.pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            id,
            Pending {
                method: method.to_string(),
                buf_key: Some(7),
                revision: 3,
                sent_at: Instant::now(),
            },
        );
}

fn handle(core: Arc<Core>) -> LspHandle {
    LspHandle {
        core,
        language_id: "rust".to_string(),
        child: Arc::new(Mutex::new(None)),
    }
}

#[test]
fn initialize_response_signals_the_handshake() {
    let (core, init_rx, _outbound) = core();
    let sink = Collect::new();

    protocol::dispatch(&core, "rust", &sink, json!({"id": 1, "result": {}}));

    assert_eq!(init_rx.recv().unwrap(), Ok(()));
    assert_eq!(core.state(), ServerState::Ready);
    assert!(
        sink.events().is_empty(),
        "the initialize response is not a user event"
    );
}

#[test]
fn initialize_error_signals_failure() {
    let (core, init_rx, _outbound) = core();
    let sink = Collect::new();

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({"id": 1, "error": {"code": -32603, "message": "nope"}}),
    );

    assert!(init_rx.recv().unwrap().is_err());
}

#[test]
fn diagnostics_notification_is_delivered() {
    let (core, _init, _outbound) = core();
    let sink = Collect::new();

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": "file:///src/lib.rs",
                "version": 4,
                "diagnostics": [{
                    "range": {"start": {"line": 1, "character": 2}, "end": {"line": 1, "character": 5}},
                    "severity": 1,
                    "message": "broken",
                }],
            },
        }),
    );

    match &sink.events()[0] {
        Event::Diagnostics {
            path,
            version,
            diagnostics,
        } => {
            assert_eq!(path, "file:///src/lib.rs");
            assert_eq!(*version, Some(4));
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].message, "broken");
            assert_eq!(diagnostics[0].line, 1);
            assert_eq!(diagnostics[0].column, 2);
        }
        other => panic!("expected diagnostics, got {other:?}"),
    }
}

#[test]
fn cancellation_is_silent_and_drops_the_pending_request() {
    let (core, _init, _outbound) = core();
    let sink = Collect::new();
    add_pending(&core, 9, "textDocument/hover");

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({"id": 9, "error": {"code": -32800, "message": "content modified"}}),
    );

    assert!(
        sink.events().is_empty(),
        "cancellation is not an error event"
    );
    assert!(
        core.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty(),
        "the cancelled request must not linger"
    );
}

#[test]
fn a_real_error_is_reported_against_its_request() {
    let (core, _init, _outbound) = core();
    let sink = Collect::new();
    add_pending(&core, 9, "textDocument/hover");

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({"id": 9, "error": {"code": -32603, "message": "boom"}}),
    );

    match &sink.events()[0] {
        Event::Response {
            method,
            result,
            buf_key,
            revision,
            ..
        } => {
            assert_eq!(method, "error");
            assert!(result.contains("boom"));
            assert_eq!(*buf_key, Some(7));
            assert_eq!(*revision, 3);
        }
        other => panic!("expected response, got {other:?}"),
    }
}

#[test]
fn hover_response_carries_request_context() {
    let (core, _init, _outbound) = core();
    let sink = Collect::new();
    add_pending(&core, 9, "textDocument/hover");

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({"id": 9, "result": {"contents": {"kind": "markdown", "value": "fn f()"}}}),
    );

    match &sink.events()[0] {
        Event::Hover {
            contents,
            buf_key,
            revision,
            ..
        } => {
            assert_eq!(contents, "fn f()");
            assert_eq!(*buf_key, Some(7));
            assert_eq!(*revision, 3);
        }
        other => panic!("expected hover, got {other:?}"),
    }
}

#[test]
fn apply_edit_is_forwarded_and_not_answered_by_the_reader() {
    let (core, _init, mut outbound) = core();
    let sink = Collect::new();

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({"id": 3, "method": "workspace/applyEdit", "params": {"edit": {"changes": {}}}}),
    );

    match &sink.events()[0] {
        Event::ApplyEdit {
            language,
            request_id,
            edit_json,
        } => {
            assert_eq!(language, "rust");
            assert_eq!(*request_id, 3);
            assert!(edit_json.contains("changes"));
        }
        other => panic!("expected apply-edit, got {other:?}"),
    }
    assert!(
        outbound.try_recv().is_err(),
        "the foreground owns the answer"
    );
}

#[test]
fn unknown_server_request_is_answered_with_null() {
    let (core, _init, mut outbound) = core();
    let sink = Collect::new();

    protocol::dispatch(
        &core,
        "rust",
        &sink,
        json!({"id": 4, "method": "workspace/configuration", "params": {}}),
    );

    let reply = outbound.try_recv().unwrap();
    assert_eq!(reply["id"], 4);
    assert!(reply["result"].is_null());
    assert!(sink.events().is_empty());
}

#[test]
fn eof_drains_pending_requests_then_reports_exit() {
    let (core, _init, _outbound) = core();
    let sink = Collect::new();
    add_pending(&core, 5, "textDocument/hover");

    super::reader_loop(
        Cursor::new(Vec::<u8>::new()),
        core.clone(),
        "rust".into(),
        sink.clone(),
    );

    let events = sink.events();
    assert_eq!(events.len(), 2, "one dead request, then the exit notice");
    match &events[0] {
        Event::Response {
            method,
            result,
            buf_key,
            revision,
            ..
        } => {
            assert_eq!(method, "textDocument/hover");
            assert_eq!(result, "language server exited");
            assert_eq!(*buf_key, Some(7));
            assert_eq!(*revision, 3);
        }
        other => panic!("expected drained response, got {other:?}"),
    }
    assert!(matches!(&events[1], Event::ServerExited { language } if language == "rust"));
    assert_eq!(core.state(), ServerState::Exited);
}

#[test]
fn a_malformed_frame_is_skipped_and_the_stream_continues() {
    let (core, _init, _outbound) = core();
    let sink = Collect::new();
    let mut wire = b"Content-Length: nope\r\n\r\n".to_vec();
    wire.extend_from_slice(&lsp_proto::encode(&json!({
        "method": "textDocument/publishDiagnostics",
        "params": {"uri": "file:///a.rs", "diagnostics": []},
    })));

    super::reader_loop(Cursor::new(wire), core, "rust".into(), sink.clone());

    let events = sink.events();
    assert!(matches!(&events[0], Event::Diagnostics { path, .. } if path == "file:///a.rs"));
    assert!(matches!(events.last(), Some(Event::ServerExited { .. })));
}

#[test]
fn start_reports_a_missing_binary() {
    let err = LspHandle::start(
        "magma-no-such-lsp-binary-9f1c",
        &[],
        "rust",
        None,
        Collect::new(),
    )
    .unwrap_err();

    assert!(
        err.contains("Failed to spawn LSP server"),
        "unexpected error: {err}"
    );
}

#[test]
fn request_ids_are_sequential_and_keep_their_context() {
    let (core, _init, mut outbound) = core();
    let handle = handle(core.clone());

    let first = handle.request("textDocument/hover", json!({}), 7, 3);
    let second = handle.request_workspace("workspace/symbol", json!({}));

    assert_eq!((first, second), (1, 2));
    assert_eq!(outbound.try_recv().unwrap()["method"], "textDocument/hover");
    assert_eq!(outbound.try_recv().unwrap()["method"], "workspace/symbol");
    let pending = core.pending.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(pending.get(&1).map(|p| p.buf_key), Some(Some(7)));
    assert_eq!(pending.get(&1).map(|p| p.revision), Some(3));
    assert_eq!(pending.get(&2).map(|p| p.buf_key), Some(None));
}

#[test]
fn shutdown_sends_the_request_then_exit_before_killing() {
    let (core, _init, mut outbound) = core();
    let handle = handle(core);

    handle.shutdown();

    let request = outbound.try_recv().unwrap();
    assert_eq!(request["method"], "shutdown");
    assert_eq!(request["id"], 1);
    let notification = outbound.try_recv().unwrap();
    assert_eq!(notification["method"], "exit");
    assert!(outbound.try_recv().is_err());
}

#[test]
fn restart_budget_stops_a_crash_loop() {
    let mut registry = LspRegistry::default();
    let sink = Collect::new();

    for _ in 0..3 {
        let err = registry
            .ensure_started(
                "magma-no-such-lsp-binary-9f1c",
                &[],
                "rust",
                None,
                sink.clone(),
            )
            .unwrap_err();
        assert!(err.contains("Failed to spawn"));
    }
    let err = registry
        .ensure_started("magma-no-such-lsp-binary-9f1c", &[], "rust", None, sink)
        .unwrap_err();
    assert!(
        err.contains("exceeded restart attempts"),
        "unexpected error: {err}"
    );
}
