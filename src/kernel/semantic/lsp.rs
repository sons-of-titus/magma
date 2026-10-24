//! LSP client — a real server lifecycle (rework.md §4).
//!
//! `spawn → initialize (awaited, timeout) → initialized → didOpen /
//! didChange (real document versions) → requests correlated per server →
//! cancellation → shutdown / crash detection`.
//!
//! Ownership: the registry lives on the foreground `Editor`.  The IO
//! threads share only an `Arc` core (outbound queue + pending map + state)
//! and speak to the foreground through `BackgroundEvent`s, which carry the
//! buffer key + document revision each request was made against.  Stale
//! responses are dropped by the foreground.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::lsp_proto::{self, Inbound};
use crate::kernel::runtime::BackgroundEvent;

const INIT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// One in-flight request, kept until its response (or its timeout).
#[derive(Debug, Clone)]
pub struct Pending {
    pub method: String,
    /// Buffer the request was made against, for revision validation.
    pub buf_key: Option<usize>,
    pub revision: u64,
    pub sent_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Initializing,
    Ready,
    Exited,
}

/// The thread-shared half of a server connection.
struct Core {
    outbound: tokio::sync::mpsc::UnboundedSender<Value>,
    pending: Mutex<HashMap<u64, Pending>>,
    state: Mutex<ServerState>,
    next_id: AtomicU64,
    /// Signaled by the reader when the `initialize` response arrives.
    init_signal: Mutex<std::sync::mpsc::Sender<Result<(), String>>>,
}

impl Core {
    fn state(&self) -> ServerState {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_state(&self, s: ServerState) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }
}

/// A running language server connection.  Clones share one connection.
#[derive(Clone)]
pub struct LspHandle {
    core: Arc<Core>,
    language_id: String,
    child: Arc<Mutex<Option<std::process::Child>>>,
}

impl std::fmt::Debug for LspHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspHandle")
            .field("language_id", &self.language_id)
            .field("state", &self.core.state())
            .finish()
    }
}

impl LspHandle {
    /// Spawn a server and perform the awaited initialize handshake.
    pub fn start(
        command: &str,
        args: &[String],
        language_id: &str,
        root: Option<String>,
        bg: crate::kernel::runtime::BackgroundHandle,
    ) -> Result<LspHandle, String> {
        let mut child = std::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn LSP server '{command}': {e}"))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "LSP server stdin unavailable".to_string())?;
        let stdout = child.stdout.take()
            .ok_or_else(|| "LSP server stdout unavailable".to_string())?;

        let (outbound, outbound_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let (init_tx, init_rx) = std::sync::mpsc::channel();

        let core = Arc::new(Core {
            outbound,
            pending: Mutex::new(HashMap::new()),
            state: Mutex::new(ServerState::Initializing),
            next_id: AtomicU64::new(1),
            init_signal: Mutex::new(init_tx),
        });

        // Writer thread: one owned stdin, pulls prebuilt messages.
        std::thread::Builder::new()
            .name(format!("lsp-write-{language_id}"))
            .spawn(move || writer_loop(stdin, outbound_rx))
            .map_err(|e| format!("LSP writer thread: {e}"))?;

        // Reader thread: frames messages, correlates responses, detects exit.
        let reader_core = core.clone();
        let reader_lang = language_id.to_string();
        let reader_bg = bg.clone();
        std::thread::Builder::new()
            .name(format!("lsp-read-{language_id}"))
            .spawn(move || reader_loop(stdout, reader_core, reader_lang, reader_bg))
            .map_err(|e| format!("LSP reader thread: {e}"))?;

        let handle = LspHandle {
            core,
            language_id: language_id.to_string(),
            child: Arc::new(Mutex::new(Some(child))),
        };

        // The awaited handshake: the reader signals when the response lands.
        let init_params = lsp_proto::initialize_params(root.as_deref());
        let id = handle.request_internal("initialize", init_params, None);
        match init_rx.recv_timeout(INIT_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                handle.kill();
                return Err(format!("LSP initialize failed for {language_id}: {e}"));
            }
            Err(_) => {
                handle.kill();
                return Err(format!("LSP initialize timed out for {language_id}"));
            }
        }
        let _ = id;

        handle.notify("initialized", json!({}));
        Ok(handle)
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn state(&self) -> ServerState {
        self.core.state()
    }

    fn request_internal(&self, method: &str, params: Value, ctx: Option<(usize, u64)>) -> u64 {
        let id = self.core.next_id.fetch_add(1, Ordering::Relaxed);
        self.core.pending
            .lock().unwrap_or_else(|e| e.into_inner())
            .insert(id, Pending {
                method: method.to_string(),
                buf_key: ctx.map(|(k, _)| k),
                revision: ctx.map(|(_, r)| r).unwrap_or(0),
                sent_at: Instant::now(),
            });
        let _ = self.core.outbound.send(lsp_proto::request(id, method, params));
        id
    }

    /// Send a request tied to a buffer + revision; the response event lets
    /// the foreground reject results for stale revisions.
    pub fn request(&self, method: &str, params: Value, buf_key: usize, revision: u64) -> u64 {
        self.sweep_timeouts();
        self.request_internal(method, params, Some((buf_key, revision)))
    }

    /// Send a request not tied to a document (workspace-level).
    pub fn request_workspace(&self, method: &str, params: Value) -> u64 {
        self.sweep_timeouts();
        self.request_internal(method, params, None)
    }

    pub fn notify(&self, method: &str, params: Value) {
        let _ = self.core.outbound.send(lsp_proto::notification(method, params));
    }

    /// `textDocument/didOpen` — version is the document revision.
    pub fn did_open(&self, uri: &str, text: &str, version: u64) {
        self.notify("textDocument/didOpen", json!({
            "textDocument": {
                "uri": uri,
                "languageId": self.language_id,
                "version": version,
                "text": text,
            }
        }));
    }

    /// `textDocument/didChange` — full-text sync, real version.
    pub fn did_change(&self, uri: &str, text: &str, version: u64) {
        self.notify("textDocument/didChange", json!({
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": [{ "text": text }],
        }));
    }

    pub fn did_close(&self, uri: &str) {
        self.notify("textDocument/didClose", json!({ "textDocument": { "uri": uri } }));
    }

    /// `$/cancelRequest` for an outstanding request id.
    pub fn cancel(&self, id: u64) {
        self.notify("$/cancelRequest", json!({ "id": id }));
    }

    /// Fail requests that have waited past the deadline so callers are
    /// never left waiting on a dead id.
    fn sweep_timeouts(&self) {
        let mut stale = Vec::new();
        {
            let mut pending = self.core.pending.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            pending.retain(|id, p| {
                if now.duration_since(p.sent_at) > REQUEST_TIMEOUT {
                    stale.push((*id, p.method.clone()));
                    false
                } else {
                    true
                }
            });
        }
        let _ = stale;
    }

    /// Ordered shutdown: `shutdown` request, `exit` notification, kill.
    pub fn shutdown(&self) {
        let _ = self.core.outbound.send(lsp_proto::request(
            self.core.next_id.fetch_add(1, Ordering::Relaxed),
            "shutdown", Value::Null,
        ));
        self.notify("exit", Value::Null);
        self.kill();
    }

    /// Kill the server process; the reader thread observes EOF and marks
    /// the state Exited.
    pub fn kill(&self) {
        if let Ok(mut guard) = self.child.lock()
            && let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
    }
}

fn writer_loop(mut stdin: std::process::ChildStdin, rx: tokio::sync::mpsc::UnboundedReceiver<Value>) {
    use std::io::Write;
    let mut rx = rx;
    while let Some(msg) = rx.blocking_recv() {
        let bytes = lsp_proto::encode(&msg);
        if stdin.write_all(&bytes).is_err() || stdin.flush().is_err() {
            return;
        }
    }
}

fn reader_loop(
    stdout: std::process::ChildStdout,
    core: Arc<Core>,
    language_id: String,
    bg: crate::kernel::runtime::BackgroundHandle,
) {
    let mut read = stdout;
    loop {
        match lsp_proto::read_message(&mut read) {
            Err(lsp_proto::FrameError::Eof) => break,
            Err(lsp_proto::FrameError::Malformed(e)) => {
                // A malformed message must never kill the IDE: skip it.
                debug!("LSP {language_id}: skipping malformed message: {e}");
                continue;
            }
            Ok(msg) => handle_message(&core, &language_id, &bg, msg),
        }
    }

    core.set_state(ServerState::Exited);
    // Every pending request dies with the server; say so instead of leaving
    // callers waiting forever.
    let pending: Vec<Pending> = core.pending
        .lock().unwrap_or_else(|e| e.into_inner())
        .drain()
        .map(|(_, p)| p)
        .collect();
    for p in pending {
        bg.send_event(BackgroundEvent::LspResponse {
            path: language_id.clone(),
            method: p.method,
            result: "language server exited".to_string(),
            request_id: 0,
            buf_key: p.buf_key,
            revision: p.revision,
        });
    }
    bg.send_event(BackgroundEvent::LspServerExited { language: language_id });
}

fn handle_message(core: &Arc<Core>, language_id: &str, bg: &crate::kernel::runtime::BackgroundHandle, msg: Value) {
    let inbound = if let Some(id) = msg.get("id").and_then(Value::as_u64) {
        if let Some(method) = msg.get("method").and_then(Value::as_str) {
            Inbound::Request { id, method: method.into(), params: msg.get("params").cloned().unwrap_or(Value::Null) }
        } else {
            let error = msg.get("error").map(|e| e.to_string());
            Inbound::Response { id, result: msg.get("result").cloned(), error }
        }
    } else if let Some(method) = msg.get("method").and_then(Value::as_str) {
        Inbound::Notification { method: method.into(), params: msg.get("params").cloned().unwrap_or(Value::Null) }
    } else {
        return;
    };

    match inbound {
        Inbound::Notification { method, params } => match method.as_str() {
            "textDocument/publishDiagnostics" => {
                let uri = params.pointer("/uri").and_then(Value::as_str).unwrap_or_default().to_string();
                let version = params.get("version").and_then(Value::as_u64);
                let diagnostics = parse_diagnostics(&params);
                bg.send_event(BackgroundEvent::LspDiagnostics { path: uri, version, diagnostics });
            }
            "$/progress" => {
                let token = params.pointer("/value/token").and_then(Value::as_str).unwrap_or_default().to_string();
                let message = params.pointer("/value/message").and_then(Value::as_str).unwrap_or_default().to_string();
                let percentage = params.pointer("/value/percentage")
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                bg.send_event(BackgroundEvent::LspProgress { token, message, percentage });
            }
            _ => {}
        },
        Inbound::Request { id, method: _method, params: _ } => {
            // Server→client requests (workspace/configuration, …) get an
            // empty response so the server does not stall waiting.
            let _ = core.outbound.send(json!({
                "jsonrpc": "2.0", "id": id, "result": Value::Null,
            }));
        }
        Inbound::Response { id, result, error } => {
            if let Some(err) = error {
                // initialize errors complete the handshake with failure.
                if let Ok(tx) = core.init_signal.lock() {
                    let _ = tx.send(Err(err.clone()));
                }
                if let Some(p) = core.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
                    bg.send_event(lsp_response_event(language_id, "error", &err, p));
                }
                return;
            }
            if id == 1 {
                // The initialize response completes the awaited handshake.
                if let Ok(tx) = core.init_signal.lock() {
                    let _ = tx.send(Ok(()));
                }
                core.set_state(ServerState::Ready);
                return;
            }
            let pending = core.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            if let Some(p) = pending {
                let method = p.method.clone();
                let event = match method.as_str() {
                    "textDocument/hover" => BackgroundEvent::LspHover {
                        path: language_id.to_string(),
                        contents: extract_hover_text(result.as_ref()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/definition" => {
                        let mut loc = result.as_ref()
                            .and_then(|r| first_location(r));
                        if loc.is_none() {
                            // Also consume the empty result as "nowhere".
                            loc = None;
                        }
                        match loc {
                            Some((uri, sl, sc, el, ec)) => BackgroundEvent::LspDefinition {
                                path: language_id.to_string(),
                                uri, start_line: sl, start_col: sc, end_line: el, end_col: ec,
                                buf_key: p.buf_key,
                                revision: p.revision,
                            },
                            None => lsp_response_event(language_id, "textDocument/definition", "null", p),
                        }
                    }
                    "textDocument/codeAction" => BackgroundEvent::LspCodeActions {
                        path: language_id.to_string(),
                        actions: result.map(|r| r.to_string()).unwrap_or_else(|| "[]".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/completion" => BackgroundEvent::LspCompletionItems {
                        path: language_id.to_string(),
                        items: result.map(|r| r.to_string()).unwrap_or_else(|| "[]".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/rename" => BackgroundEvent::LspRenameResult {
                        path: language_id.to_string(),
                        edit: result.map(|r| r.to_string()).unwrap_or_else(|| "null".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    other => {
                        let method = p.method.clone();
                        let event = lsp_response_event(language_id, &method, &result.map(|r| r.to_string()).unwrap_or_default(), p);
                        let _ = other;
                        event
                    }
                };
                bg.send_event(event);
            }
        }
    }
}

fn lsp_response_event(language_id: &str, method: &str, result: &str, p: Pending) -> BackgroundEvent {
    BackgroundEvent::LspResponse {
        path: language_id.to_string(),
        method: method.to_string(),
        result: result.to_string(),
        request_id: 0,
        buf_key: p.buf_key,
        revision: p.revision,
    }
}

/// Flatten a hover response to display text.
pub(crate) fn extract_hover_text(result: Option<&Value>) -> String {
    let contents = result.and_then(|r| r.get("contents"));
    match contents {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o.get("value")
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_default(),
        Some(Value::Array(items)) => items.iter()
            .filter_map(|i| i.get("value").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// First `Location`/`LocationLink` from a definition response.
pub(crate) fn first_location(result: &Value) -> Option<(String, usize, usize, usize, usize)> {
    let arr = match result {
        Value::Array(a) => a.first().cloned(),
        Value::Object(_) => Some(result.clone()),
        _ => None,
    }?;
    let target = arr.get("targetSelectionRange").or_else(|| arr.get("targetUri").map(|_| &arr))
        .unwrap_or(&arr);
    let uri = target.get("uri").or_else(|| arr.get("uri"))?.as_str()?.to_string();
    let start = target.get("range").or_else(|| arr.get("range"))?.get("start")?;
    let end = target.get("range").or_else(|| arr.get("range"))?.get("end")?;
    Some((
        uri,
        start.get("line").and_then(Value::as_u64)? as usize,
        start.get("character").and_then(Value::as_u64)? as usize,
        end.get("line").and_then(Value::as_u64)? as usize,
        end.get("character").and_then(Value::as_u64)? as usize,
    ))
}

/// Typed diagnostics straight off `publishDiagnostics` params.
pub(crate) fn parse_diagnostics(params: &Value) -> Vec<crate::kernel::semantic::provider::Diagnostic> {
    let Some(items) = params.get("diagnostics").and_then(Value::as_array) else {
        return Vec::new();
    };
    items.iter().filter_map(|d| {
        let range = d.get("range")?;
        let start = range.get("start")?;
        let end = range.get("end")?;
        let severity = match d.get("severity").and_then(Value::as_u64) {
            Some(1) => crate::kernel::semantic::provider::DiagnosticSeverity::Error,
            Some(2) => crate::kernel::semantic::provider::DiagnosticSeverity::Warning,
            Some(4) => crate::kernel::semantic::provider::DiagnosticSeverity::Hint,
            _ => crate::kernel::semantic::provider::DiagnosticSeverity::Info,
        };
        Some(crate::kernel::semantic::provider::Diagnostic {
            message: d.get("message").and_then(Value::as_str).unwrap_or_default().to_string(),
            severity,
            line: start.get("line").and_then(Value::as_u64)? as usize,
            column: start.get("character").and_then(Value::as_u64)? as usize,
            end_line: end.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
            end_column: end.get("character").and_then(Value::as_u64).unwrap_or(0) as usize,
        })
    }).collect()
}

/// Registry of running servers, owned by the foreground `Editor`.
#[derive(Default)]
pub struct LspRegistry {
    servers: HashMap<String, Arc<LspHandle>>,
    /// Restart attempts per language — bounded so a crash-looping server
    /// cannot spin forever.
    restarts: HashMap<String, u32>,
    /// Buffers with an attached server (buffer slab key → language id).
    /// Recorded at didOpen; drives Rust-side completion/hover requests.
    buffer_languages: HashMap<usize, String>,
}

impl LspRegistry {
    pub fn get(&self, language_id: &str) -> Option<&Arc<LspHandle>> {
        self.servers.get(language_id)
    }

    pub fn note_buffer_language(&mut self, buf_key: usize, language_id: &str) {
        self.buffer_languages.insert(buf_key, language_id.to_string());
    }

    pub fn language_for_buffer(&self, buf_key: usize) -> Option<String> {
        self.buffer_languages.get(&buf_key).cloned()
    }

    pub fn insert(&mut self, language_id: &str, handle: Arc<LspHandle>) {
        self.servers.insert(language_id.to_string(), handle);
    }

    /// Start (or return the running) server for a language.
    pub fn ensure_started(
        &mut self,
        command: &str,
        args: &[String],
        language_id: &str,
        root: Option<String>,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> Result<Arc<LspHandle>, String> {
        if let Some(existing) = self.servers.get(language_id)
            && existing.state() == ServerState::Ready {
                return Ok(existing.clone());
            }
        let restarts = self.restarts.entry(language_id.to_string()).or_insert(0);
        if *restarts >= 3 {
            return Err(format!("LSP server for {language_id} exceeded restart attempts"));
        }
        *restarts += 1;
        let handle = LspHandle::start(command, args, language_id, root, bg.clone())?;
        let handle = Arc::new(handle);
        self.servers.insert(language_id.to_string(), handle.clone());
        Ok(handle)
    }

    /// Reset the restart budget when a server has been stable.
    pub fn note_ready(&mut self, language_id: &str) {
        self.restarts.remove(language_id);
    }

    pub fn exited(&mut self, language_id: &str) {
        self.servers.remove(language_id);
    }

    pub fn running(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    pub fn shutdown_all(&mut self) {
        for handle in self.servers.values() {
            handle.shutdown();
        }
        self.servers.clear();
    }
}
