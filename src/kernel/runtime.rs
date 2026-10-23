//! Background task system — bridges the tokio async runtime with the
//! synchronous editor loop via a typed `BackgroundEvent` channel.
//!
//! Documented over the size gate (AGENTS.md gate 3): this file is the
//! single dispatch match every async subsystem lands in.  Subsystem
//! handlers that grow domain logic split out at their abstraction
//! boundaries — DAP results in `runtime_dap.rs`, index reconciliation in
//! `kernel/index`, LSP validation in `semantic/lsp` — leaving the arms
//! as thin calls.

use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::kernel::state::Editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

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
}

/// Handle for spawning background tasks and sending events to the main thread.
/// Cloneable — all clones share the same runtime and event channel.
#[derive(Clone)]
pub struct BackgroundHandle {
    runtime: Arc<Runtime>,
    /// Channel to send events to the main thread event loop.
    pub sender: mpsc::UnboundedSender<BackgroundEvent>,
    /// Priority lanes for background work (rework.md §3).  P0 interactive
    /// work never enters this struct — it runs inline on the foreground.
    lanes: Arc<Lanes>,
}

/// Bounded concurrency per priority class, plus coalescing: a queued job
/// whose coalesce key was superseded skips itself instead of doing work
/// whose result is already stale.
struct Lanes {
    foreground: Arc<tokio::sync::Semaphore>,
    indexing: Arc<tokio::sync::Semaphore>,
    maintenance: Arc<tokio::sync::Semaphore>,
    superseded: Mutex<std::collections::HashMap<(Lane, String), u64>>,
    tickets: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    /// P1: visible-buffer work (parses, completion scans). 2 slots.
    Foreground,
    /// P2: project indexing, symbol extraction. cpu-1 slots.
    Indexing,
    /// P3: persistence flushes, refreshes. 1 slot.
    Maintenance,
}

impl Lane {
    fn semaphore(self, lanes: &Lanes) -> Arc<tokio::sync::Semaphore> {
        match self {
            Lane::Foreground => lanes.foreground.clone(),
            Lane::Indexing => lanes.indexing.clone(),
            Lane::Maintenance => lanes.maintenance.clone(),
        }
    }

    fn slots(&self) -> usize {
        match self {
            Lane::Foreground => 2,
            Lane::Indexing => (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) - 1).max(1),
            Lane::Maintenance => 1,
        }
    }
}

/// True when the buffer the request was made against is still at the
/// revision the response was computed for — the stale-result gate
/// (rework.md §2).  Requests without document context pass.
fn revision_current(ed: &Editor, buf_key: Option<usize>, revision: u64) -> bool {
    match buf_key {
        None => true,
        Some(key) => ed.buffers.get(key)
            .map(|a| a.lock().unwrap().revision() == revision)
            .unwrap_or(false),
    }
}

/// Match a publishDiagnostics URI against a buffer path (LSP sends file URIs).
fn lsp_path_matches(buf_path: Option<&str>, diag_path: &str) -> bool {
    let diag = diag_path.strip_prefix("file://").unwrap_or(diag_path);
    buf_path == Some(diag) || buf_path == Some(diag_path)
}

/// Process a background event on the main thread, dispatching it to the event bus.
pub fn process_background_event(ed: &mut Editor, event: BackgroundEvent) {
    ed.frame_dirty = true;
    match event {
        BackgroundEvent::FileReadResult { path, content, request_id: _ } => {
            match content {
                Ok(text) => {
                    ed.events.emit_typed(keys::events::FILE_LOADED, FileLoadedPayload { path, content: text });
                }
                Err(e) => {
                    ed.events.emit_typed(keys::events::FILE_ERROR, FileErrorPayload { path, error: e });
                }
            }
        }
        BackgroundEvent::FileWriteResult { path, result, request_id: _ } => {
            match result {
                Ok(()) => {
                    ed.events.emit_typed(keys::events::FILE_SAVED, FileSavedPayload { path });
                }
                Err(e) => {
                    ed.events.emit_typed(keys::events::FILE_ERROR, FileErrorPayload { path, error: e });
                }
            }
        }
        BackgroundEvent::LspDiagnostics { path, version: _, diagnostics } => {
            let count = diagnostics.len();
            // Typed diagnostics live in the semantic engine (canonical);
            // the buffer keeps display strings only.  URI vs path is
            // normalized so servers sending file:// URIs still land.
            let display: Vec<String> = diagnostics.iter()
                .map(|d| format!("[{}] line {}:{} {}",
                    d.severity.as_str()[..1].to_uppercase(), d.line + 1, d.column + 1, d.message))
                .collect();
            let typed = diagnostics.clone();
            let mut buffer_id: Option<usize> = None;
            for (key, arc) in ed.buffers.iter_mut() {
                let mut buf = arc.lock().unwrap();
                if lsp_path_matches(buf.path.as_deref(), &path) {
                    buf.set_diagnostics(display.clone());
                    buffer_id = Some(key);
                    break;
                }
            }
            ed.semantic.update_diagnostics(&path, typed);
            ed.events.emit_typed(keys::events::LSP_DIAGNOSTICS, LspDiagnosticsPayload {
                buffer_id: buffer_id.map(|k| k.to_string()).unwrap_or_default(),
                path,
                count: count.to_string(),
                items: display.join("\n"),
            });
        }
        BackgroundEvent::LspCompletion { path, items, request_id: _ } => {
            ed.events.emit_typed(keys::events::LSP_COMPLETION, LspCompletionPayload {
                path,
                items: items.join("\n"),
            });
        }
        BackgroundEvent::TerminalOutput { terminal_id, data } => {
            ed.io.terminals.apply_output(terminal_id, &data);
        }
        BackgroundEvent::TerminalExited { terminal_id, exit_code } => {
            if exit_code != 0 {
                ed.show_message(crate::kernel::state::MessageKind::Error,
                    format!("terminal exited with code {exit_code}"));
            }
            ed.io.terminals.close(terminal_id);
            if ed.io.terminals.focused.is_none() && ed.editor_mode.name == "terminal" {
                crate::kernel::terminal::set_focused(ed, None);
            }
        }
        BackgroundEvent::FileChanged { path, kind } => {
            // Only list-relevant changes mark the index stale for the GUI's
            // debounced rescan — churn in ignored dirs (build output, VCS,
            // tool state) and writes to already-indexed files don't change
            // the file list, and re-walking on each of them re-indexed the
            // project every couple of seconds during any build.
            if let Some(root) = ed.project_manager.project.root.clone()
                && let Some(canonical) = std::fs::canonicalize(&root).ok()
                && path.starts_with(canonical.to_string_lossy().as_ref())
                && crate::kernel::project::index_change_marks_dirty(
                    &canonical, &path, &kind, &ed.project_manager.project.files) {
                    ed.project_manager.project.file_index_dirty = true;
                }
            ed.events.emit_typed(keys::events::FILE_CHANGED, FileChangedPayload { path, kind });
        }
        BackgroundEvent::ProcessOutput { id, line, stream } => {
            ed.events.emit_typed(keys::events::PROCESS_OUTPUT, ProcessOutputPayload {
                id: id.to_string(),
                line,
                stream,
            });
        }
        BackgroundEvent::ProcessExited { id, exit_code, cmd } => {
            // Finished processes leave the list — it tracks live processes,
            // and a graveyard of exited entries taught nothing.
            ed.io.processes.remove(&id);
            if exit_code == 0 {
                ed.scheduler.complete_process(id);
            } else {
                ed.scheduler.fail_process(id, format!("exit code {exit_code}"));
            }
            ed.events.emit_typed(keys::events::PROCESS_EXIT, ProcessExitPayload {
                id: id.to_string(),
                exit_code: exit_code.to_string(),
                cmd,
            });
        }
        BackgroundEvent::TaskReady { id } => {
            let mut rt = ed.runtime.take();
            if let Some(rt) = &mut rt {
                rt.execute_stored_task(ed, id);
            }
            ed.runtime = rt;
        }
        BackgroundEvent::FileIndexed { project_name, files } => {
            // Update the project state in the registry with the file list
            if let Some(proj) = ed.project_manager.projects.get_mut(&project_name) {
                proj.files = files.clone();
                proj.files_epoch += 1;
                proj.file_index_dirty = false;
            }
            if ed.project_manager.current_project.as_deref() == Some(&project_name) {
                ed.project_manager.project.files = files.clone();
                ed.project_manager.project.files_epoch += 1;
                ed.project_manager.project.file_index_dirty = false;
                // Phase 5: wire indexed files into the Semantic Engine's ProjectGraph
                ed.project_manager.sync_to_project_graph(&mut ed.semantic.project_graph);
                crate::kernel::index::reconcile_project_index(ed, &files);
            }
            ed.events.emit_typed(keys::events::FILE_INDEXED, FileIndexedPayload {
                project_name,
                count: files.len().to_string(),
            });
        }
        BackgroundEvent::GrepResultsBatch { generation, entries } => {
            if generation == ed.search_generation {
                ed.quickfix_list.extend(entries);
                ed.quickfix_list.truncate(crate::kernel::render::gui_find_tool::MAX_RESULTS);
            }
        }
        BackgroundEvent::GrepDone { generation } => {
            if generation == ed.search_generation {
                ed.quickfix_index = 0;
            }
        }
        // ── Sprint 13 Network API ─────────────────────────────────────────
        BackgroundEvent::HttpResponse { id, status, body } => {
            ed.events.emit_typed(keys::events::HTTP_RESPONSE, HttpResponsePayload {
                id: id.to_string(),
                status: status.to_string(),
                body,
            });
        }
        BackgroundEvent::HttpError { id, error } => {
            ed.events.emit_typed(keys::events::HTTP_ERROR, HttpErrorPayload {
                id: id.to_string(),
                error,
            });
        }
        BackgroundEvent::TcpConnected { id } => {
            if let Some(conn) = ed.io.net_connections.get_mut(&id) {
                conn.connected = true;
            }
            ed.events.emit_typed(keys::events::TCP_CONNECTED, TcpConnectedPayload {
                id: id.to_string(),
            });
        }
        BackgroundEvent::TcpData { id, data: line } => {
            ed.events.emit_typed(keys::events::TCP_DATA, TcpDataPayload {
                id: id.to_string(),
                data: line,
            });
        }
        BackgroundEvent::TcpClosed { id } => {
            ed.io.net_connections.remove(&id);
            ed.events.emit_typed(keys::events::TCP_CLOSED, TcpClosedPayload {
                id: id.to_string(),
            });
        }
        BackgroundEvent::TcpError { id, error } => {
            ed.io.net_connections.remove(&id);
            ed.events.emit_typed(keys::events::TCP_ERROR, TcpErrorPayload {
                id: id.to_string(),
                error,
            });
        }
        BackgroundEvent::TcpClientConnected { server_id, client_id, write_tx } => {
            if let Some(srv) = ed.io.net_servers.get_mut(&server_id) {
                srv.clients.insert(client_id, write_tx);
            }
            ed.events.emit_typed(keys::events::TCP_CLIENT_CONNECTED, TcpClientConnectedPayload {
                server_id: server_id.to_string(),
                client_id: client_id.to_string(),
            });
        }
        BackgroundEvent::TcpClientData { server_id, client_id, data: line } => {
            ed.events.emit_typed(keys::events::TCP_CLIENT_DATA, TcpClientDataPayload {
                server_id: server_id.to_string(),
                client_id: client_id.to_string(),
                data: line,
            });
        }
        BackgroundEvent::TcpClientDisconnected { server_id, client_id } => {
            if let Some(srv) = ed.io.net_servers.get_mut(&server_id) {
                srv.clients.remove(&client_id);
            }
            ed.events.emit_typed(keys::events::TCP_CLIENT_DISCONNECTED, TcpClientDisconnectedPayload {
                server_id: server_id.to_string(),
                client_id: client_id.to_string(),
            });
        }
        // ─────────────────────────────────────────────────────────────────
        // ── Sprint 5 LSP Power Features ──────────────────────────────────
        BackgroundEvent::LspResponse { path, method, result, request_id: _, buf_key, revision } => {
            if revision_current(ed, buf_key, revision) {
                ed.events.emit_typed(keys::events::LSP_RESPONSE, LspResponsePayload { path, method, result });
            }
        }
        BackgroundEvent::LspServerExited { language } => {
            ed.lsp.exited(&language);
            ed.show_message(crate::kernel::state::MessageKind::Error,
                format!("language server for {language} exited"));
        }
        BackgroundEvent::DapStarted { session_id, result } => {
            match result {
                Ok(handle) => {
                    ed.debug.connections.insert(session_id, handle);
                    // Sync every project breakpoint to the fresh adapter,
                    // then release it with configurationDone — adapters that
                    // require the sequence (e.g. js-debug) hang without it.
                    crate::kernel::debug::sync_breakpoint_lines(ed);
                    let files: Vec<String> = ed.debug.breakpoints.keys().cloned().collect();
                    for file in files {
                        #[cfg(feature = "janet")]
                        crate::kernel::scripting::send_dap_command(
                            ed, session_id, "setBreakpoints",
                            &ed.debug.set_breakpoints_args(&file));
                    }
                    #[cfg(feature = "janet")]
                    crate::kernel::scripting::send_dap_command(
                        ed, session_id, "configurationDone", "{}");
                    ed.events.emit_typed(keys::events::DEBUG_SESSION_STARTED,
                        crate::kernel::event::payload::DebugSessionStartedPayload {
                            session_id: session_id.to_string(),
                            adapter: ed.debug.sessions.get(&session_id)
                                .map(|s| s.adapter.clone()).unwrap_or_default(),
                        });
                }
                Err(e) => {
                    ed.debug.sessions.remove(&session_id);
                    ed.show_message(crate::kernel::state::MessageKind::Error, e);
                }
            }
        }
        BackgroundEvent::LspStarted { language, result } => {
            match result {
                Ok(handle) => {
                    ed.lsp.insert(&language, handle);
                    ed.lsp.note_ready(&language);
                    let mut data = std::collections::HashMap::new();
                    data.insert("language".to_string(), language);
                    ed.events.emit("lsp-initialized", data);
                }
                Err(e) => {
                    ed.show_message(crate::kernel::state::MessageKind::Error, e);
                }
            }
        }
        BackgroundEvent::LspHover { path, contents, buf_key, revision } => {
            if revision_current(ed, buf_key, revision) {
                ed.events.emit_typed(keys::events::LSP_HOVER, LspHoverPayload { path, contents });
            }
        }
        BackgroundEvent::LspDefinition { path, uri, start_line, start_col, end_line, end_col, buf_key, revision } => {
            if revision_current(ed, buf_key, revision) {
                ed.events.emit_typed(keys::events::LSP_DEFINITION, LspDefinitionPayload {
                    path,
                    uri,
                    start_line: start_line.to_string(),
                    end_line: end_line.to_string(),
                    start_col: start_col.to_string(),
                    end_col: end_col.to_string(),
                });
            }
        }
        BackgroundEvent::LspCodeActions { path, actions, buf_key, revision } => {
            if revision_current(ed, buf_key, revision) {
                ed.events.emit_typed(keys::events::LSP_CODE_ACTIONS, LspCodeActionsPayload { path, actions });
            }
        }
        BackgroundEvent::LspCompletionItems { path, items, buf_key, revision } => {
            if revision_current(ed, buf_key, revision) {
                ed.events.emit_typed(keys::events::LSP_COMPLETION_ITEMS, LspCompletionItemsPayload { path, items });
            }
        }
        BackgroundEvent::LspRenameResult { path, edit, buf_key, revision } => {
            if revision_current(ed, buf_key, revision) {
                ed.events.emit_typed(keys::events::LSP_RENAME_RESULT, LspRenameResultPayload { path, edit });
            }
        }
        BackgroundEvent::LspProgress { token, message, percentage } => {
            ed.events.emit_typed(keys::events::LSP_PROGRESS, LspProgressPayload { token, message, percentage });
        }
        // ──────────────────────────────────────────────────────────────────
        // ── Phase 6 Debug System ─────────────────────────────────────────────
        BackgroundEvent::DapStopped { session_id, reason, thread_id } => {
            let watch_count = ed.debug.watches.len();
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.note_stopped(watch_count);
                session.stopped_thread = Some(thread_id);
            }
            // A stopped debuggee with no stack is a facade — fetch it.
            #[cfg(feature = "janet")]
            crate::kernel::scripting::send_dap_command(
                ed, session_id, "stackTrace", &format!(r#"{{"threadId":{thread_id},"startFrame":0,"levels":20}}"#),
            );
            // The thread list changes across stops; refresh it for the pane.
            #[cfg(feature = "janet")]
            crate::kernel::scripting::send_dap_command(ed, session_id, "threads", "{}");
            ed.events.emit_typed(keys::events::DEBUG_STOPPED, crate::kernel::event::payload::DebugStoppedPayload {
                session_id: session_id.to_string(),
                reason,
                thread_id: thread_id.to_string(),
            });
        }
        BackgroundEvent::DapOutput { session_id, category, output } => {
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.push_console(&output);
            }
            ed.events.emit_typed(keys::events::DEBUG_OUTPUT, crate::kernel::event::payload::DebugOutputPayload {
                session_id: session_id.to_string(),
                category,
                output,
            });
        }
        BackgroundEvent::DapTerminated { session_id } => {
            ed.debug.connections.remove(&session_id);
            ed.debug.sessions.remove(&session_id);
            if ed.debug.active_session == Some(session_id) {
                ed.debug.active_session = None;
            }
            ed.events.emit_typed(keys::events::DEBUG_SESSION_ENDED, crate::kernel::event::payload::DebugSessionEndedPayload {
                session_id: session_id.to_string(),
            });
        }
        BackgroundEvent::DapResponse { session_id, event_type, data } => {
            if event_type == "evaluate" {
                crate::kernel::runtime_dap::apply_dap_response(ed, session_id, &data);
                ed.events.emit_typed(keys::events::DEBUG_EVALUATE_RESULT, crate::kernel::event::payload::DebugEvaluateResultPayload {
                    session_id: session_id.to_string(),
                    result: data,
                });
            } else {
                let mut payload = std::collections::HashMap::new();
                payload.insert("session-id".into(), session_id.to_string());
                payload.insert("event-type".into(), event_type);
                payload.insert("data".into(), data);
                ed.events.emit("debug-dap-event", payload);
            }
        }
        BackgroundEvent::DapCommandResult { session_id, command, success, body } => {
            if !success {
                crate::kernel::runtime_dap::note_request_error(ed, session_id, &command, &body);
            }
            crate::kernel::runtime_dap::apply_dap_command_result(ed, session_id, &command, success, &body);
        }
        // ─────────────────────────────────────────────────────────────────────
        BackgroundEvent::Custom(name, payload) => {
            let mut data = std::collections::HashMap::new();
            data.insert("payload".into(), payload);
            ed.events.emit(&name, data);
        }
        // ── Phase 9 Concurrency Model ─────────────────────────────────────────
        BackgroundEvent::WorkProgress { id, done, total } => {
            ed.scheduler.update_progress(id, done, total);
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.events.emit_typed(keys::events::SCHEDULER_WORK_PROGRESS, crate::kernel::event::payload::SchedulerWorkProgressPayload {
                id: id.to_string(), name, done: done.to_string(), total: total.to_string(),
            });
        }
        BackgroundEvent::WorkCompleted { id } => {
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.scheduler.mark_completed(id);
            ed.events.emit_typed(keys::events::SCHEDULER_WORK_COMPLETED, crate::kernel::event::payload::SchedulerWorkCompletedPayload {
                id: id.to_string(), name,
            });
        }
        BackgroundEvent::WorkFailed { id, error } => {
            let name = ed.scheduler.name(id).unwrap_or("").to_string();
            ed.scheduler.mark_failed(id, error.clone());
            ed.events.emit_typed(keys::events::SCHEDULER_WORK_FAILED, crate::kernel::event::payload::SchedulerWorkFailedPayload {
                id: id.to_string(), name, error,
            });
        }
        BackgroundEvent::AutoSaveTick => {
            // The heartbeat doubles as the maintenance tick: persist the
            // project index when it has pending changes.
            if ed.index.is_dirty()
                && let Err(e) = ed.index.flush() {
                    debug!("index flush failed: {e}");
                }
            // Opt-in only: the editor must never rewrite user files uninvited.
            if ed.options.get("auto-save").map(|v| v.as_str()) != Some("true") {
                return;
            }
            let Some(bg) = ed.background.clone() else { return };
            let dirty: Vec<(usize, String, String, u64)> = ed.buffers.iter()
                .filter(|(_, arc)| arc.lock().unwrap().modified())
                .filter_map(|(key, arc)| {
                    let buf = arc.lock().unwrap();
                    buf.path.clone().map(|p| (key, p, buf.slice(0, buf.len()), buf.revision()))
                })
                .collect();
            for (key, path, content, saved_rev) in dirty {
                let sender = bg.sender.clone();
                bg.spawn_blocking(move || {
                    let result = crate::kernel::storage::disk::atomic_write(&path, &content);
                    if result.is_ok() {
                        let _ = sender.send(BackgroundEvent::AutoSaveWritten { buf_key: key, saved_rev });
                    }
                });
            }
        }
        BackgroundEvent::AutoSaveWritten { buf_key, saved_rev } => {
            if let Some(arc) = ed.buffers.get_mut(buf_key) {
                // Only the revision actually written becomes clean; edits
                // made after the snapshot keep the buffer modified
                // (docs/core-redesign.md §5.1).
                arc.lock().unwrap().mark_saved_at(saved_rev);
            }
        }
        BackgroundEvent::BufferWords { buf_key, revision, words } => {
            let current = ed.buffers.get(buf_key).map(|a| a.lock().unwrap().revision());
            if current == Some(revision) {
                ed.completion.word_cache.insert(buf_key, crate::kernel::state::completion::WordCache {
                    revision,
                    words: std::sync::Arc::new(words.into_iter().collect()),
                });
            }
            // Clear the in-flight marker only for the request that landed —
            // a newer revision's scan must keep its slot.
            if ed.completion.words_requested == Some((buf_key, revision)) {
                ed.completion.words_requested = None;
            }
        }
        BackgroundEvent::IndexRecordArrived(rec) => {
            ed.index.apply(rec);
        }
        BackgroundEvent::TsTreeReady { buf_key, revision } => {
            crate::kernel::semantic::ts_schedule::take_ready_tree(ed, buf_key, revision);
        }
        BackgroundEvent::VcOpDone { scratch, then_refresh, result } => {
            if let Err(e) = crate::kernel::command::builtin::vcs::apply_vc_op(
                ed, scratch.as_deref(), then_refresh, result,
            ) {
                ed.show_message(crate::kernel::state::MessageKind::Error, e);
            }
        }
        BackgroundEvent::VcStatusFetched { backend_idx, root, result } => {
            match result {
                Ok(status) => {
                    ed.vc.last_status = Some(status);
                    crate::kernel::command::builtin::vcs::redraw_vc_buffer(ed);
                }
                Err(e) => ed.show_message(crate::kernel::state::MessageKind::Error, e),
            }
            let _ = (backend_idx, root);
        }
        BackgroundEvent::TaskOutputLine { id, stream, line } => {
            // Output-buffer tasks stream straight into their buffer so long
            // builds show progress as they run.
            if let Some(sink) = ed.task_scheduler.output_sinks.get(&id).cloned() {
                crate::kernel::task::append_output_line(ed, &sink, &line);
            }
            let mut data = std::collections::HashMap::new();
            data.insert("task-id".to_string(), id.to_string());
            data.insert("stream".to_string(), stream);
            data.insert("line".to_string(), line);
            ed.events.emit("task-output-line", data);
        }
        BackgroundEvent::TaskRunCompleted { id, exit_code, stdout, stderr, duration_ms } => {
            use crate::kernel::task::TaskOutput;
            let output = TaskOutput { stdout, stderr, exit_code, duration_ms };
            crate::kernel::task::finish_output_task(ed, id, exit_code, &output.stdout, &output.stderr);
            let task_name = ed.task_scheduler.tasks.get(&id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            ed.task_scheduler.running.remove(&id);
            ed.last_task = Some((task_name.clone(), format!(
                "$ {}\n{}{}",
                ed.task_scheduler.tasks.get(&id).map(|t| t.command.clone()).unwrap_or_default(),
                output.stdout,
                output.stderr,
            )));
            if exit_code == 0 {
                ed.task_scheduler.mark_completed(id, output);
                ed.scheduler.complete_task(id);
                ed.events.emit_typed(keys::events::TASK_COMPLETED, TaskCompletedPayload {
                    id: id.to_string(),
                    name: task_name,
                    exit_code: exit_code.to_string(),
                    duration_ms: duration_ms.to_string(),
                });
            } else {
                let err = format!("exit code {exit_code}");
                ed.task_scheduler.mark_failed(id, err.clone(), output);
                ed.scheduler.fail_task(id, err.clone());
                ed.events.emit_typed(keys::events::TASK_FAILED, TaskFailedPayload {
                    id: id.to_string(),
                    name: task_name,
                    error: err,
                });
            }
            crate::kernel::run_config::continue_pending(ed, id, exit_code);
            let mut rt = ed.runtime.take();
            if let Some(rt) = &mut rt {
                rt.execute_task_complete_callback(ed, id);
            }
            ed.runtime = rt;
            if let Some(bg) = ed.background.clone() {
                let ready: Vec<u64> = ed.task_scheduler.ready_to_run();
                for rid in ready {
                    let _ = ed.task_scheduler.launch(rid, &bg);
                }
            }
        }
        // ── Phase 11 AI/Agent Architecture ───────────────────────────────────
        BackgroundEvent::AgentThought { session_id, thought } => {
            ed.events.emit_typed(keys::events::AGENT_THOUGHT, crate::kernel::event::payload::AgentThoughtPayload {
                session_id: session_id.to_string(),
                thought,
            });
        }
        BackgroundEvent::AgentAction { session_id, command, description } => {
            ed.events.emit_typed(keys::events::AGENT_ACTION, crate::kernel::event::payload::AgentActionPayload {
                session_id: session_id.to_string(),
                command,
                description,
            });
        }
        BackgroundEvent::AgentResult { session_id, result } => {
            ed.agent.complete_session(session_id, result.clone());
            ed.events.emit_typed(keys::events::AGENT_RESULT, crate::kernel::event::payload::AgentResultPayload {
                session_id: session_id.to_string(),
                result,
            });
        }
        BackgroundEvent::AgentError { session_id, error } => {
            ed.agent.fail_session(session_id, error.clone());
            ed.events.emit_typed(keys::events::AGENT_ERROR, crate::kernel::event::payload::AgentErrorPayload {
                session_id: session_id.to_string(),
                error,
            });
        }
    }
}

impl BackgroundHandle {
    pub fn new(runtime: Arc<Runtime>, sender: mpsc::UnboundedSender<BackgroundEvent>) -> Self {
        let lanes = Lanes {
            foreground: Arc::new(tokio::sync::Semaphore::new(Lane::Foreground.slots())),
            indexing: Arc::new(tokio::sync::Semaphore::new(Lane::Indexing.slots())),
            maintenance: Arc::new(tokio::sync::Semaphore::new(Lane::Maintenance.slots())),
            superseded: Mutex::new(std::collections::HashMap::new()),
            tickets: std::sync::atomic::AtomicU64::new(0),
        };
        Self { runtime, sender, lanes: Arc::new(lanes) }
    }

    /// Submit blocking work on a priority lane with an optional coalesce
    /// key.  A queued job whose key was superseded by a later submission
    /// exits when it finally gets a slot — newer work wins over stale.
    pub fn spawn_on<F>(&self, lane: Lane, coalesce: Option<&str>, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let ticket = self.lanes.tickets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let coalesce_key = coalesce.map(|k| (lane, k.to_string()));
        if let Some(key) = &coalesce_key {
            let mut guard = self.lanes.superseded.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(key.clone(), ticket);
        }
        let semaphore = lane.semaphore(&self.lanes);
        let lanes = self.lanes.clone();
        self.spawn_blocking(move || {
            // Coalescing: a newer ticket for this key means the input is
            // already outdated — skip without occupying a lane slot.
            if let Some(key) = &coalesce_key {
                let guard = lanes.superseded.lock().unwrap_or_else(|e| e.into_inner());
                if guard.get(key).is_some_and(|&current| current != ticket) {
                    return;
                }
            }
            // spawn_blocking threads carry a runtime Handle; block on the
            // lane's semaphore there.
            let Ok(permit) = tokio::runtime::Handle::current().block_on(semaphore.acquire()) else {
                return;
            };
            f();
            drop(permit);
        });
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(future);
    }

    pub fn spawn_blocking<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.runtime.spawn_blocking(f);
    }

    pub fn send_event(&self, event: BackgroundEvent) {
        let _ = self.sender.send(event);
    }

    /// Synchronously execute a blocking closure on the runtime's blocking thread
    /// pool, blocking the calling thread until complete.
    /// Used by the Janet bridge to offload potentially-blocking I/O.
    pub fn block_on<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.runtime.spawn_blocking(move || {
            let _ = tx.send(f());
        });
        rx.blocking_recv().expect("BackgroundHandle::block_on: task panicked")
    }

    pub fn read_file(&self, path: String, request_id: u64) {
        let sender = self.sender.clone();
        self.spawn_blocking(move || {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string());
            let _ = sender.send(BackgroundEvent::FileReadResult { path, content, request_id });
        });
    }

    pub fn write_file(&self, path: String, content: String, request_id: u64) {
        let sender = self.sender.clone();
        self.spawn_blocking(move || {
            let result = (|| -> Result<(), String> {
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(&path, &content).map_err(|e| e.to_string())?;
                Ok(())
            })();
            let _ = sender.send(BackgroundEvent::FileWriteResult { path, result, request_id });
        });
    }
}
