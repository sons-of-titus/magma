//! Background task system — bridges the tokio async runtime with the
//! synchronous editor loop via a typed `BackgroundEvent` channel.

use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use std::sync::Arc;

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

/// Insert terminal PTY output into a buffer, respecting cursor-control semantics.
///
/// Characters produced by `strip_ansi` can contain:
///   `\r`   — standalone CR: overwrite from the start of the current line.
///   `\x08` — backspace marker: remove the last character on the current line.
///             `strip_ansi` emits these for `\x08` and `\x1b[nD` sequences so
///             that ghost-suggestion text can be cancelled even when it arrived
///             in a prior `read()` chunk.
fn apply_terminal_output(buf: &mut crate::kernel::text_engine::Buffer, data: &str) {
    // Fast path: plain text with no control markers.
    if !data.contains('\r') && !data.contains('\x08') {
        let pos = buf.len();
        buf.insert(pos, data);
        return;
    }

    // Helper: byte index of the start of the last line in the buffer.
    let line_start = |buf: &crate::kernel::text_engine::Buffer| -> usize {
        let len = buf.len();
        if len == 0 { return 0; }
        let text = buf.slice(0, len);
        text.rfind('\n').map(|i| i + 1).unwrap_or(0)
    };

    let mut remaining = data;
    while !remaining.is_empty() {
        // Find the next control character (\r or \x08).
        let ctrl = remaining
            .char_indices()
            .find(|(_, c)| *c == '\r' || *c == '\x08');

        match ctrl {
            None => {
                let pos = buf.len();
                buf.insert(pos, remaining);
                break;
            }
            Some((idx, ch)) => {
                // Append everything before the control char.
                if idx > 0 {
                    let pos = buf.len();
                    buf.insert(pos, &remaining[..idx]);
                }
                remaining = &remaining[idx + ch.len_utf8()..];

                match ch {
                    '\r' => {
                        // Overwrite from the start of the current line.
                        let cur_len = buf.len();
                        let ls = line_start(buf);
                        if ls < cur_len {
                            buf.delete(ls, cur_len);
                        }
                    }
                    '\x08' => {
                        // Remove the last character on the current line.
                        let cur_len = buf.len();
                        let ls = line_start(buf);
                        if ls < cur_len {
                            let text = buf.slice(0, cur_len);
                            let last_char_offset = text[ls..]
                                .char_indices()
                                .next_back()
                                .map(|(i, _)| ls + i);
                            if let Some(start) = last_char_offset {
                                buf.delete(start, cur_len);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Events sent from background tasks to the main thread event loop.
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    FileReadResult { path: String, content: Result<String, String>, request_id: u64 },
    FileWriteResult { path: String, result: Result<(), String>, request_id: u64 },
    /// LSP diagnostic publish notification.
    LspDiagnostics { path: String, diagnostics: Vec<String> },
    /// LSP completion response (Sprint 4).
    LspCompletion { path: String, items: Vec<String>, request_id: u64 },
    /// Terminal output chunk from a background process.
    TerminalOutput { buf_id: usize, data: String },
    /// A terminal process has exited.
    TerminalExited { buf_id: usize, exit_code: i32 },
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
    /// Generic LSP response for arbitrary methods.
    LspResponse { path: String, method: String, result: String, request_id: u64 },
    /// Parsed hover response contents.
    LspHover { path: String, contents: String },
    /// Parsed go-to-definition result.
    LspDefinition { path: String, uri: String, start_line: usize, start_col: usize, end_line: usize, end_col: usize },
    /// Parsed code actions (JSON array of `{"title","kind"}`).
    LspCodeActions { path: String, actions: String },
    /// Parsed completion items (JSON array of `{"label","detail","insertText"}`).
    LspCompletionItems { path: String, items: String },
    /// Parsed rename result (WorkspaceEdit JSON).
    LspRenameResult { path: String, edit: String },
    /// Progress notification from `$/progress`.
    LspProgress { token: String, message: String, percentage: String },
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
    Custom(String, String),
    // ── Phase 9 Concurrency Model ─────────────────────────────────────────────
    /// A directly-submitted work item reported progress.
    WorkProgress { id: u64, done: u64, total: u64 },
    /// A directly-submitted work item completed successfully.
    WorkCompleted { id: u64 },
    /// A directly-submitted work item failed.
    WorkFailed { id: u64, error: String },
}

/// Handle for spawning background tasks and sending events to the main thread.
/// Cloneable — all clones share the same runtime and event channel.
#[derive(Clone)]
pub struct BackgroundHandle {
    runtime: Arc<Runtime>,
    /// Channel to send events to the main thread event loop.
    pub sender: mpsc::UnboundedSender<BackgroundEvent>,
}

/// Process a background event on the main thread, dispatching it to the event bus.
pub fn process_background_event(ed: &mut Editor, event: BackgroundEvent) {
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
        BackgroundEvent::LspDiagnostics { path, diagnostics } => {
            let count = diagnostics.len();
            // Store on the buffer if we can find it.
            for (_, arc) in ed.buffers.iter_mut() {
                let mut buf = arc.lock().unwrap();
                if buf.path.as_deref() == Some(&path) {
                    buf.set_diagnostics(diagnostics.clone());
                    break;
                }
            }
            // Also store typed diagnostics in the semantic engine.
            ed.semantic.update_diagnostics_from_strings(&path, &diagnostics);
            ed.events.emit_typed(keys::events::LSP_DIAGNOSTICS, LspDiagnosticsPayload {
                path,
                count: count.to_string(),
                items: diagnostics.join("\n"),
            });
        }
        BackgroundEvent::LspCompletion { path, items, request_id: _ } => {
            ed.events.emit_typed(keys::events::LSP_COMPLETION, LspCompletionPayload {
                path,
                items: items.join("\n"),
            });
        }
        BackgroundEvent::TerminalOutput { buf_id, data } => {
            if let Some(arc) = ed.buffers.get_mut(buf_id) {
                let mut buf = arc.lock().unwrap();
                apply_terminal_output(&mut buf, &data);
            }
        }
        BackgroundEvent::TerminalExited { buf_id, exit_code: _ } => {
            ed.io.terminals.remove(&buf_id);

            // Switch the focused window away from this buffer before removing it.
            let fallback = ed.buffers.iter()
                .map(|(k, _)| k)
                .find(|&k| k != buf_id);
            if let Some(win) = ed.view_tree.focused_window_mut()
                && win.buffer_id == Some(buf_id) {
                    win.buffer_id = fallback;
                }

            // Kill the buffer so the terminal content doesn't linger.
            ed.buffers.remove(buf_id);

            // Return to normal mode (we were in Terminal mode).
            if ed.io.terminal_mode_buf == Some(buf_id) {
                ed.io.terminal_mode_buf = None;
                ed.editor_mode = crate::kernel::state::mode::EditorMode::new("normal", false);
                ed.keymaps.push_layer("vim");
            }
        }
        BackgroundEvent::FileChanged { path, kind } => {
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
            if let Some(state) = ed.io.processes.get_mut(&id) {
                state.running = false;
            }
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
            if let Some(ref mut rt) = ed.runtime {
                rt.execute_stored_task(id);
            }
        }
        BackgroundEvent::FileIndexed { project_name, files } => {
            // Update the project state in the registry with the file list
            if let Some(proj) = ed.project_manager.projects.get_mut(&project_name) {
                proj.files = files.clone();
                proj.file_index_dirty = false;
            }
            if ed.project_manager.current_project.as_deref() == Some(&project_name) {
                ed.project_manager.project.files = files.clone();
                ed.project_manager.project.file_index_dirty = false;
                // Phase 5: wire indexed files into the Semantic Engine's ProjectGraph
                ed.project_manager.sync_to_project_graph(&mut ed.semantic.project_graph);
            }
            ed.events.emit_typed(keys::events::FILE_INDEXED, FileIndexedPayload {
                project_name,
                count: files.len().to_string(),
            });
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
        BackgroundEvent::LspResponse { path, method, result, request_id: _ } => {
            ed.events.emit_typed(keys::events::LSP_RESPONSE, LspResponsePayload { path, method, result });
        }
        BackgroundEvent::LspHover { path, contents } => {
            ed.events.emit_typed(keys::events::LSP_HOVER, LspHoverPayload { path, contents });
        }
        BackgroundEvent::LspDefinition { path, uri, start_line, start_col, end_line, end_col } => {
            ed.events.emit_typed(keys::events::LSP_DEFINITION, LspDefinitionPayload {
                path,
                uri,
                start_line: start_line.to_string(),
                end_line: end_line.to_string(),
                start_col: start_col.to_string(),
                end_col: end_col.to_string(),
            });
        }
        BackgroundEvent::LspCodeActions { path, actions } => {
            ed.events.emit_typed(keys::events::LSP_CODE_ACTIONS, LspCodeActionsPayload { path, actions });
        }
        BackgroundEvent::LspCompletionItems { path, items } => {
            ed.events.emit_typed(keys::events::LSP_COMPLETION_ITEMS, LspCompletionItemsPayload { path, items });
        }
        BackgroundEvent::LspRenameResult { path, edit } => {
            ed.events.emit_typed(keys::events::LSP_RENAME_RESULT, LspRenameResultPayload { path, edit });
        }
        BackgroundEvent::LspProgress { token, message, percentage } => {
            ed.events.emit_typed(keys::events::LSP_PROGRESS, LspProgressPayload { token, message, percentage });
        }
        // ──────────────────────────────────────────────────────────────────
        // ── Phase 6 Debug System ─────────────────────────────────────────────
        BackgroundEvent::DapStopped { session_id, reason, thread_id } => {
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.stopped = true;
                session.stopped_thread = Some(thread_id);
            }
            ed.events.emit_typed(keys::events::DEBUG_STOPPED, crate::kernel::event::payload::DebugStoppedPayload {
                session_id: session_id.to_string(),
                reason,
                thread_id: thread_id.to_string(),
            });
        }
        BackgroundEvent::DapOutput { session_id, category, output } => {
            ed.events.emit_typed(keys::events::DEBUG_OUTPUT, crate::kernel::event::payload::DebugOutputPayload {
                session_id: session_id.to_string(),
                category,
                output,
            });
        }
        BackgroundEvent::DapTerminated { session_id } => {
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
        BackgroundEvent::TaskRunCompleted { id, exit_code, stdout, stderr, duration_ms } => {
            use crate::kernel::task::TaskOutput;
            let output = TaskOutput { stdout, stderr, exit_code, duration_ms };
            let task_name = ed.task_scheduler.tasks.get(&id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
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
            if let Some(ref mut rt) = ed.runtime {
                rt.execute_task_complete_callback(id);
            }
            if let Some(bg) = ed.background.clone() {
                let ready: Vec<u64> = ed.task_scheduler.ready_to_run();
                for rid in ready {
                    let _ = ed.task_scheduler.launch(rid, &bg);
                }
            }
        }
    }
}

impl BackgroundHandle {
    pub fn new(runtime: Arc<Runtime>, sender: mpsc::UnboundedSender<BackgroundEvent>) -> Self {
        Self { runtime, sender }
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
