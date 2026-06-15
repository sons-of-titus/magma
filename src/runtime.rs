//! Background task system — bridges the tokio async runtime with the
//! synchronous editor loop via a typed `BackgroundEvent` channel.

use std::collections::HashMap;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::state::Editor;

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
fn apply_terminal_output(buf: &mut crate::buffer::Buffer, data: &str) {
    // Fast path: plain text with no control markers.
    if !data.contains('\r') && !data.contains('\x08') {
        let pos = buf.len();
        buf.insert(pos, data);
        return;
    }

    // Helper: byte index of the start of the last line in the buffer.
    let line_start = |buf: &crate::buffer::Buffer| -> usize {
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
    Custom(String, String),
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
                    let mut data = HashMap::new();
                    data.insert("path".into(), path);
                    data.insert("content".into(), text);
                    ed.events.emit("file-loaded", data);
                }
                Err(e) => {
                    let mut data = HashMap::new();
                    data.insert("path".into(), path);
                    data.insert("error".into(), e);
                    ed.events.emit("file-error", data);
                }
            }
        }
        BackgroundEvent::FileWriteResult { path, result, request_id: _ } => {
            match result {
                Ok(()) => {
                    let mut data = HashMap::new();
                    data.insert("path".into(), path);
                    ed.events.emit("file-saved", data);
                }
                Err(e) => {
                    let mut data = HashMap::new();
                    data.insert("path".into(), path);
                    data.insert("error".into(), e);
                    ed.events.emit("file-error", data);
                }
            }
        }
        BackgroundEvent::LspDiagnostics { path, diagnostics } => {
            let count = diagnostics.len();
            // Store on the buffer if we can find it
            for (_, buf) in ed.buffers.iter_mut() {
                if buf.path.as_deref() == Some(&path) {
                    buf.set_diagnostics(diagnostics.clone());
                    break;
                }
            }
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("count".into(), count.to_string());
            data.insert("items".into(), diagnostics.join("\n"));
            ed.events.emit("lsp-diagnostics", data);
        }
        BackgroundEvent::LspCompletion { path, items, request_id: _ } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("items".into(), items.join("\n"));
            ed.events.emit("lsp-completion", data);
        }
        BackgroundEvent::TerminalOutput { buf_id, data } => {
            if let Some(buf) = ed.buffers.get_mut(buf_id) {
                apply_terminal_output(buf, &data);
            }
        }
        BackgroundEvent::TerminalExited { buf_id, exit_code: _ } => {
            ed.io.terminals.remove(&buf_id);

            // Switch the focused window away from this buffer before removing it.
            let fallback = ed.buffers.iter()
                .map(|(k, _)| k)
                .find(|&k| k != buf_id);
            if let Some(win) = ed.windows.focused_window_mut()
                && win.buffer_id == Some(buf_id) {
                    win.buffer_id = fallback;
                }

            // Kill the buffer so the terminal content doesn't linger.
            ed.buffers.remove(buf_id);

            // Return to normal mode (we were in Terminal mode).
            if ed.io.terminal_mode_buf == Some(buf_id) {
                ed.io.terminal_mode_buf = None;
                ed.editor_mode = crate::state::mode::EditorMode::new("normal", false);
                ed.keymaps.push_layer("vim");
            }
        }
        BackgroundEvent::FileChanged { path, kind } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("kind".into(), kind);
            ed.events.emit("file-changed", data);
        }
        BackgroundEvent::ProcessOutput { id, line, stream } => {
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            data.insert("line".into(), line);
            data.insert("stream".into(), stream);
            ed.events.emit("process-output", data);
        }
        BackgroundEvent::ProcessExited { id, exit_code, cmd } => {
            if let Some(state) = ed.io.processes.get_mut(&id) {
                state.running = false;
            }
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            data.insert("exit-code".into(), exit_code.to_string());
            data.insert("cmd".into(), cmd);
            ed.events.emit("process-exit", data);
        }
        BackgroundEvent::TaskReady { id } => {
            #[cfg(feature = "janet")]
            crate::janet_bridge::execute_stored_task(ed, id);
            #[cfg(not(feature = "janet"))]
            let _ = id;
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
            }
            let mut data = HashMap::new();
            data.insert("project-name".into(), project_name);
            data.insert("count".into(), files.len().to_string());
            ed.events.emit("file-indexed", data);
        }
        // ── Sprint 13 Network API ─────────────────────────────────────────
        BackgroundEvent::HttpResponse { id, status, body } => {
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            data.insert("status".into(), status.to_string());
            data.insert("body".into(), body);
            ed.events.emit("http-response", data);
        }
        BackgroundEvent::HttpError { id, error } => {
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            data.insert("error".into(), error);
            ed.events.emit("http-error", data);
        }
        BackgroundEvent::TcpConnected { id } => {
            if let Some(conn) = ed.io.net_connections.get_mut(&id) {
                conn.connected = true;
            }
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            ed.events.emit("tcp-connected", data);
        }
        BackgroundEvent::TcpData { id, data: line } => {
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            data.insert("data".into(), line);
            ed.events.emit("tcp-data", data);
        }
        BackgroundEvent::TcpClosed { id } => {
            ed.io.net_connections.remove(&id);
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            ed.events.emit("tcp-closed", data);
        }
        BackgroundEvent::TcpError { id, error } => {
            ed.io.net_connections.remove(&id);
            let mut data = HashMap::new();
            data.insert("id".into(), id.to_string());
            data.insert("error".into(), error);
            ed.events.emit("tcp-error", data);
        }
        BackgroundEvent::TcpClientConnected { server_id, client_id, write_tx } => {
            if let Some(srv) = ed.io.net_servers.get_mut(&server_id) {
                srv.clients.insert(client_id, write_tx);
            }
            let mut data = HashMap::new();
            data.insert("server-id".into(), server_id.to_string());
            data.insert("client-id".into(), client_id.to_string());
            ed.events.emit("tcp-client-connected", data);
        }
        BackgroundEvent::TcpClientData { server_id, client_id, data: line } => {
            let mut data = HashMap::new();
            data.insert("server-id".into(), server_id.to_string());
            data.insert("client-id".into(), client_id.to_string());
            data.insert("data".into(), line);
            ed.events.emit("tcp-client-data", data);
        }
        BackgroundEvent::TcpClientDisconnected { server_id, client_id } => {
            if let Some(srv) = ed.io.net_servers.get_mut(&server_id) {
                srv.clients.remove(&client_id);
            }
            let mut data = HashMap::new();
            data.insert("server-id".into(), server_id.to_string());
            data.insert("client-id".into(), client_id.to_string());
            ed.events.emit("tcp-client-disconnected", data);
        }
        // ─────────────────────────────────────────────────────────────────
        // ── Sprint 5 LSP Power Features ──────────────────────────────────
        BackgroundEvent::LspResponse { path, method, result, request_id: _ } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("method".into(), method);
            data.insert("result".into(), result);
            ed.events.emit("lsp-response", data);
        }
        BackgroundEvent::LspHover { path, contents } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("contents".into(), contents);
            ed.events.emit("lsp-hover", data);
        }
        BackgroundEvent::LspDefinition { path, uri, start_line, start_col, end_line, end_col } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("uri".into(), uri);
            data.insert("start-line".into(), start_line.to_string());
            data.insert("start-col".into(), start_col.to_string());
            data.insert("end-line".into(), end_line.to_string());
            data.insert("end-col".into(), end_col.to_string());
            ed.events.emit("lsp-definition", data);
        }
        BackgroundEvent::LspCodeActions { path, actions } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("actions".into(), actions);
            ed.events.emit("lsp-code-actions", data);
        }
        BackgroundEvent::LspCompletionItems { path, items } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("items".into(), items);
            ed.events.emit("lsp-completion-items", data);
        }
        BackgroundEvent::LspRenameResult { path, edit } => {
            let mut data = HashMap::new();
            data.insert("path".into(), path);
            data.insert("edit".into(), edit);
            ed.events.emit("lsp-rename-result", data);
        }
        BackgroundEvent::LspProgress { token, message, percentage } => {
            let mut data = HashMap::new();
            data.insert("token".into(), token);
            data.insert("message".into(), message);
            data.insert("percentage".into(), percentage);
            ed.events.emit("lsp-progress", data);
        }
        // ──────────────────────────────────────────────────────────────────
        BackgroundEvent::Custom(name, payload) => {
            let mut data = HashMap::new();
            data.insert("payload".into(), payload);
            ed.events.emit(&name, data);
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
