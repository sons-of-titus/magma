//! Background event system — the typed `BackgroundEvent` channel that
//! bridges the tokio async runtime with the synchronous editor loop.
//!
//! Split per R-22 at the subsystem boundary:
//!   pool.rs          — `BackgroundHandle`, priority lanes, submission
//!   handlers/        — per-subsystem event handlers (one module each)
//!   mod.rs           — the event vocabulary and the dispatch entry
//!
//! `process_background_event` is the single foreground entry point: it
//! marks the frame dirty and routes through `handlers::dispatch`.

pub mod handlers;
mod pool;

pub use pool::{BackgroundHandle, Lane};

use tokio::sync::mpsc;
use tokio::runtime::Runtime;
use std::sync::Arc;

use crate::kernel::state::Editor;

/// Tracks a single managed subprocess.
#[derive(Debug)]
pub struct ProcessState {
    pub cmd: String,
    pub running: bool,
    pub pid: u32,
    pub stdin: Option<std::process::ChildStdin>,
}

/// Tracks a background task that will execute a Janet function.
#[derive(Debug)]
pub struct TaskState {
    pub running: bool,
}

/// Events sent from background tasks to the main thread event loop.
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    FileReadResult { path: String, content: Result<String, String>, request_id: u64 },
    FileWriteResult { path: String, result: Result<(), String>, request_id: u64 },
    /// LSP diagnostic publish notification — typed, with the server-side
    /// document version when the server provides one.
    LspDiagnostics { path: String, version: Option<u64>, diagnostics: Vec<crate::kernel::semantic::provider::Diagnostic> },
    /// LSP completion response (Sprint 4).
    LspCompletion { path: String, items: Vec<String>, request_id: u64 },
    /// Terminal output chunk from a background process.
    TerminalOutput { terminal_id: usize, data: String },
    /// A terminal process has exited.
    TerminalExited { terminal_id: usize, exit_code: i32 },
    /// A watched file changed on disk.
    FileChanged { path: String, kind: String },
    /// A line of output from a managed subprocess (Sprint 3).
    ProcessOutput { id: u64, line: String, stream: String },
    /// A managed subprocess has exited (Sprint 3).
    ProcessExited { id: u64, exit_code: i32, cmd: String },
    /// A background task is ready for its Janet function to be called on the main thread (Sprint 3).
    TaskReady { id: u64 },
    /// Project file index completed (Sprint 4).
    FileIndexed { project_name: String, files: Vec<String> },
    /// A streamed batch of project-search results; appended to the
    /// quickfix list when the generation is still current.
    GrepResultsBatch { generation: u64, entries: Vec<crate::kernel::task::QuickfixEntry> },
    /// A project-wide search finished; the quickfix index resets.
    GrepDone { generation: u64 },
    // ── Sprint 13 Network API ─────────────────────────────────────────────
    /// An HTTP request completed successfully.
    HttpResponse { id: u64, status: u16, body: String },
    /// An HTTP request failed.
    HttpError { id: u64, error: String },
    /// An outbound TCP connection was established.
    TcpConnected { id: u64 },
    /// A line of data arrived from an outbound TCP connection.
    TcpData { id: u64, data: String },
    /// An outbound TCP connection was closed by the remote end.
    TcpClosed { id: u64 },
    /// An outbound TCP connection failed.
    TcpError { id: u64, error: String },
    /// A new client connected to a `net/tcp-listen` server.
    /// `write_tx` is the channel to push lines to that specific client.
    TcpClientConnected { server_id: u64, client_id: u64, write_tx: tokio::sync::mpsc::UnboundedSender<String> },
    /// A line of data arrived from a client connected to a listening server.
    TcpClientData { server_id: u64, client_id: u64, data: String },
    /// A client disconnected from a listening server.
    TcpClientDisconnected { server_id: u64, client_id: u64 },
    // ─────────────────────────────────────────────────────────────────────
    // ── Sprint 5 LSP Power Features ──────────────────────────────────────
    /// Generic LSP response for arbitrary methods, revision-tagged.
    LspResponse { path: String, method: String, result: String, request_id: u64, buf_key: Option<usize>, revision: u64 },
    /// Parsed hover response contents, tagged with the request's buffer and
    /// revision so stale responses can be rejected on arrival.
    LspHover { path: String, contents: String, buf_key: Option<usize>, revision: u64 },
    /// Parsed go-to-definition result, revision-tagged.
    LspDefinition { path: String, uri: String, start_line: usize, start_col: usize, end_line: usize, end_col: usize, buf_key: Option<usize>, revision: u64 },
    /// Parsed code actions (JSON array of `{"title","kind"}`), revision-tagged.
    LspCodeActions { path: String, actions: String, buf_key: Option<usize>, revision: u64 },
    /// Parsed completion items (JSON array of `{"label","detail","insertText"}`),
    /// revision-tagged.
    LspCompletionItems { path: String, items: String, buf_key: Option<usize>, revision: u64 },
    /// Parsed rename result (WorkspaceEdit JSON), revision-tagged.
    LspRenameResult { path: String, edit: String, buf_key: Option<usize>, revision: u64 },
    /// Progress notification from `$/progress`.
    LspProgress { token: String, message: String, percentage: String },
    /// A language server process exited (crash or termination).
    LspServerExited { language: String },
    /// The awaited DAP initialize handshake finished off-thread.
    DapStarted {
        session_id: u64,
        result: Result<std::sync::Arc<crate::kernel::debug::dap_client::DapHandle>, String>,
    },
    /// The awaited initialize handshake finished off-thread.
    LspStarted {
        language: String,
        result: Result<std::sync::Arc<crate::kernel::semantic::lsp::LspHandle>, String>,
    },
    // ─────────────────────────────────────────────────────────────────────
    /// A task defined in the TaskScheduler has completed (Phase 4).
    TaskRunCompleted { id: u64, exit_code: i32, stdout: String, stderr: String, duration_ms: u64 },
    /// DAP: the debuggee stopped (breakpoint hit, step complete, etc.) — Phase 6.
    DapStopped { session_id: u64, reason: String, thread_id: u64 },
    /// DAP: a line of console/stdout output from the debuggee — Phase 6.
    DapOutput { session_id: u64, category: String, output: String },
    /// DAP: the debug adapter process terminated — Phase 6.
    DapTerminated { session_id: u64 },
    /// DAP: a generic adapter response (evaluate result, unknown event) — Phase 6.
    DapResponse { session_id: u64, event_type: String, data: String },
    /// A DAP command response (stackTrace/scopes/variables/threads) — feeds
    /// the session model the debug pane renders.
    DapCommandResult { session_id: u64, command: String, success: bool, body: String },
    Custom(String, String),
    // ── Phase 11 AI/Agent Architecture ───────────────────────────────────────
    /// Agent emitted a reasoning step.
    AgentThought { session_id: u64, thought: String },
    /// Agent decided to execute a command; args are JSON-encoded key=value pairs.
    AgentAction { session_id: u64, command: String, description: String },
    /// A foreground-executed agent step reported its outcome; the output
    /// feeds the next LLM turn.
    AgentStepResult { session_id: u64, output: String },
    /// A model reply landed: assistant text + tool calls (or the error).
    /// Drives the agent turn loop on the foreground.
    AgentLlmReply {
        session_id: u64,
        content: String,
        tool_calls_json: String,
        error: Option<String>,
    },
    /// Agent completed a session successfully.
    AgentResult { session_id: u64, result: String },
    /// Agent session failed.
    AgentError { session_id: u64, error: String },
    // ── Phase 9 Concurrency Model ─────────────────────────────────────────────
    /// A directly-submitted work item reported progress.
    WorkProgress { id: u64, done: u64, total: u64 },
    /// A directly-submitted work item completed successfully.
    WorkCompleted { id: u64 },
    /// A directly-submitted work item failed.
    WorkFailed { id: u64, error: String },
    /// Auto-save heartbeat: the foreground loop snapshots dirty buffers and
    /// hands the writes to the background pool.
    AutoSaveTick,
    /// An auto-save write landed on disk for the given buffer revision.
    /// Only that exact revision may be marked clean (docs/core-redesign.md §5.1).
    AutoSaveWritten { buf_key: usize, saved_rev: u64 },
    /// A background word scan finished for completion.  Applied only when
    /// the buffer is still at the scanned revision; otherwise discarded.
    BufferWords { buf_key: usize, revision: u64, words: Vec<String> },
    /// One file's symbol extraction finished for the project index.
    IndexRecordArrived(crate::kernel::index::IndexRecord),
    /// One streamed output line from a running task.
    TaskOutputLine { id: u64, stream: String, line: String },
    /// A VCS status fetch finished off the foreground thread.
    VcStatusFetched {
        backend_idx: usize,
        root: std::path::PathBuf,
        result: Result<crate::kernel::vc::VcStatus, String>,
    },
    /// A scheduled tree-sitter parse finished on the lane; pick the tree
    /// up from the bounded handoff store if the revision still matches.
    TsTreeReady { buf_key: usize, revision: u64 },
    /// A VCS repository operation (stage/diff/log/push/blame/commit)
    /// finished off the foreground thread.
    VcOpDone {
        scratch: Option<String>,
        then_refresh: bool,
        result: Result<String, String>,
    },
    /// An external-change reload read finished off the foreground thread.
    BufferReloaded { buf_key: usize, content: Result<String, String> },
    /// An explicit save's disk write finished; only this exact revision
    /// may be marked clean.
    SaveWritten { buf_key: usize, path: String, saved_rev: u64 },
    /// An explicit save's disk write failed.
    SaveFailed { buf_key: usize, error: String },
    /// A find-files walk finished off the foreground thread.
    FileSearchListed { pattern: String, files: Vec<String> },
    /// The server asked us to apply a workspace edit (applyEdit); the
    /// foreground applies it and answers the request.
    LspApplyEdit { language: String, request_id: u64, edit_json: String },
}

/// Process a background event on the main thread, dispatching it to the
/// event bus via the per-subsystem handlers.
pub fn process_background_event(ed: &mut Editor, event: BackgroundEvent) {
    ed.frame_dirty = true;
    handlers::dispatch(ed, event);
}

/// Build a `BackgroundHandle` over a freshly-created tokio runtime and a
/// new event channel.  Returns `(handle, receiver)` — the receiver is the
/// foreground half the GUI loop drains each frame.
pub fn spawn_background() -> (BackgroundHandle, mpsc::UnboundedReceiver<BackgroundEvent>) {
    let runtime = Arc::new(Runtime::new().expect("tokio runtime"));
    let (tx, rx) = mpsc::unbounded_channel();
    (BackgroundHandle::new(runtime, tx), rx)
}
