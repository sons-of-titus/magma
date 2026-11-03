//! LSP client — a real server lifecycle (rework.md §4).
//!
//! `spawn → initialize (awaited, timeout) → initialized → didOpen /
//! didChange (real document versions) → requests correlated per server →
//! cancellation → shutdown / crash detection`.
//!
//! Ownership: the registry lives with the embedding application.  The IO
//! threads share only an `Arc` core (outbound queue + pending map + state)
//! and report results through an [`EventSink`], which carries the buffer key
//! and document revision each request was made against.  Stale responses are
//! dropped by the application.

mod event;
mod protocol;
mod registry;
mod semantic_tokens;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use logging::debug;
use serde_json::{Value, json};

use crate::lsp_proto;

pub use event::{Event, EventSink};
pub use protocol::{extract_hover_text, first_location, is_cancellation, parse_diagnostics};
pub use registry::LspRegistry;
pub use semantic_tokens::{SemanticLegend, SemanticToken, decode_semantic_tokens};

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
    /// Shared with the reader thread so the timeout sweep (which runs on
    /// whichever thread sends requests) can still report.
    sink: Arc<dyn EventSink + Send + Sync>,
    state: Mutex<ServerState>,
    next_id: AtomicU64,
    /// The server's semantic-token legend, captured from the initialize
    /// result and applied to every later semanticTokens response.
    legend: Mutex<semantic_tokens::SemanticLegend>,
    /// The negotiated `textDocumentSync.change` kind (0 none, 1 full,
    /// 2 incremental), from the initialize result.  `None` until known.
    sync_kind: Mutex<Option<u8>>,
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
    ///
    /// `sink` receives every event the connection produces; the client has no
    /// knowledge of the application's event channel.
    pub fn start<S>(
        command: &str,
        args: &[String],
        language_id: &str,
        root: Option<String>,
        sink: S,
    ) -> Result<LspHandle, String>
    where
        S: EventSink + Send + Sync + 'static,
    {
        let sink = Arc::new(sink);
        let mut child = std::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn LSP server '{command}': {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "LSP server stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "LSP server stdout unavailable".to_string())?;

        let (outbound, outbound_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let (init_tx, init_rx) = std::sync::mpsc::channel();

        let core = Arc::new(Core {
            outbound,
            pending: Mutex::new(HashMap::new()),
            state: Mutex::new(ServerState::Initializing),
            next_id: AtomicU64::new(1),
            legend: Mutex::new(semantic_tokens::SemanticLegend::default()),
            sync_kind: Mutex::new(None),
            init_signal: Mutex::new(init_tx),
            sink: sink.clone(),
        });

        // Writer thread: one owned stdin, pulls prebuilt messages.
        std::thread::Builder::new()
            .name(format!("lsp-write-{language_id}"))
            .spawn(move || writer_loop(stdin, outbound_rx))
            .map_err(|e| format!("LSP writer thread: {e}"))?;

        // Reader thread: frames messages, correlates responses, detects exit.
        let reader_core = core.clone();
        let reader_lang = language_id.to_string();
        std::thread::Builder::new()
            .name(format!("lsp-read-{language_id}"))
            .spawn(move || reader_loop(stdout, reader_core, reader_lang, sink))
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

    /// The negotiated `textDocumentSync.change` kind: 1 full, 2
    /// incremental.  `None` before the initialize result names one.
    pub fn sync_kind(&self) -> Option<u8> {
        *self.core.sync_kind.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn request_internal(&self, method: &str, params: Value, ctx: Option<(usize, u64)>) -> u64 {
        let id = self.core.next_id.fetch_add(1, Ordering::Relaxed);
        let superseded = {
            let mut pending = self.core.pending.lock().unwrap_or_else(|e| e.into_inner());
            // A newer request for the same buffer+method supersedes the old
            // one; tell the server to stop computing it instead of letting
            // stale work queue up behind fresh keystrokes.
            let superseded: Vec<u64> = pending
                .iter()
                .filter(|(_, p)| p.method == method && p.buf_key == ctx.map(|(k, _)| k))
                .map(|(id, _)| *id)
                .collect();
            for old in &superseded {
                pending.remove(old);
            }
            pending.insert(
                id,
                Pending {
                    method: method.to_string(),
                    buf_key: ctx.map(|(k, _)| k),
                    revision: ctx.map(|(_, r)| r).unwrap_or(0),
                    sent_at: Instant::now(),
                },
            );
            superseded
        };
        for old in superseded {
            self.notify("$/cancelRequest", json!({ "id": old }));
        }
        let _ = self
            .core
            .outbound
            .send(lsp_proto::request(id, method, params));
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
        let _ = self
            .core
            .outbound
            .send(lsp_proto::notification(method, params));
    }

    /// Answer a server→client request after the foreground handled it
    /// (e.g. `workspace/applyEdit` answered once the edit is applied).
    pub fn respond(&self, id: u64, result: Value) {
        let _ = self.core.outbound.send(json!({
            "jsonrpc": "2.0", "id": id, "result": result,
        }));
    }

    /// `textDocument/didOpen` — version is the document revision.
    pub fn did_open(&self, uri: &str, text: &str, version: u64) {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": self.language_id,
                    "version": version,
                    "text": text,
                }
            }),
        );
    }

    /// `textDocument/didChange` — full-text sync, real version.
    pub fn did_change(&self, uri: &str, text: &str, version: u64) {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }],
            }),
        );
    }

    pub fn did_close(&self, uri: &str) {
        self.notify(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri } }),
        );
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
                    stale.push((*id, p.clone()));
                    false
                } else {
                    true
                }
            });
        }
        for (id, p) in stale {
            // The server should stop computing; a late answer is dropped
            // with the pending entry anyway.
            self.notify("$/cancelRequest", json!({ "id": id }));
            self.core.sink.send(Event::RequestTimedOut {
                method: p.method,
                buf_key: p.buf_key,
                revision: p.revision,
            });
        }
    }

    /// Ordered shutdown: `shutdown` request, `exit` notification, kill.
    pub fn shutdown(&self) {
        let _ = self.core.outbound.send(lsp_proto::request(
            self.core.next_id.fetch_add(1, Ordering::Relaxed),
            "shutdown",
            Value::Null,
        ));
        self.notify("exit", Value::Null);
        self.kill();
    }

    /// Kill the server process; the reader thread observes EOF and marks
    /// the state Exited.
    pub fn kill(&self) {
        if let Ok(mut guard) = self.child.lock()
            && let Some(mut child) = guard.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn writer_loop(
    mut stdin: std::process::ChildStdin,
    rx: tokio::sync::mpsc::UnboundedReceiver<Value>,
) {
    use std::io::Write;
    let mut rx = rx;
    while let Some(msg) = rx.blocking_recv() {
        let bytes = lsp_proto::encode(&msg);
        if stdin.write_all(&bytes).is_err() || stdin.flush().is_err() {
            return;
        }
    }
}

fn reader_loop<R, S>(mut read: R, core: Arc<Core>, language_id: String, sink: S)
where
    R: Read,
    S: EventSink,
{
    loop {
        match lsp_proto::read_message(&mut read) {
            Err(lsp_proto::FrameError::Eof) => break,
            Err(lsp_proto::FrameError::Malformed(e)) => {
                // A malformed message must never kill the IDE: skip it.
                debug!("LSP {language_id}: skipping malformed message: {e}");
                continue;
            }
            Ok(msg) => protocol::dispatch(&core, &language_id, &sink, msg),
        }
    }

    core.set_state(ServerState::Exited);
    // Every pending request dies with the server; say so instead of leaving
    // callers waiting forever.
    let pending: Vec<Pending> = core
        .pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain()
        .map(|(_, p)| p)
        .collect();
    for p in pending {
        sink.send(Event::Response {
            path: language_id.clone(),
            method: p.method,
            result: "language server exited".to_string(),
            request_id: 0,
            buf_key: p.buf_key,
            revision: p.revision,
        });
    }
    sink.send(Event::ServerExited {
        language: language_id,
    });
}
