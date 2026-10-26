//! The LSP subsystem's background-event vocabulary.
//!
//! The client's reader thread and the startup thread in
//! `scripting/lsp_api.rs` send these.  The central `runtime::BackgroundEvent`
//! wraps them as `BackgroundEvent::Lsp(_)`, so the foreground keeps one
//! ordered channel while the LSP subsystem owns its payload shapes.

use std::sync::Arc;

use super::lsp::LspHandle;
use super::provider::Diagnostic;

/// An event produced by a live language server or its startup thread.
#[derive(Debug, Clone)]
pub enum Event {
    /// `textDocument/publishDiagnostics`, with the server-side version when
    /// the server provides one.
    Diagnostics {
        path: String,
        version: Option<u64>,
        diagnostics: Vec<Diagnostic>,
    },
    /// Legacy completion response carried as plain strings.
    Completion { path: String, items: Vec<String>, request_id: u64 },
    /// Generic response for arbitrary methods, revision-tagged.
    Response {
        path: String,
        method: String,
        result: String,
        request_id: u64,
        buf_key: Option<usize>,
        revision: u64,
    },
    /// Parsed hover contents, tagged with the request's buffer and revision.
    Hover { path: String, contents: String, buf_key: Option<usize>, revision: u64 },
    /// Parsed go-to-definition result, revision-tagged.
    Definition {
        path: String,
        uri: String,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        buf_key: Option<usize>,
        revision: u64,
    },
    /// Parsed code actions (JSON array of `{"title","kind"}`).
    CodeActions { path: String, actions: String, buf_key: Option<usize>, revision: u64 },
    /// Parsed completion items (JSON array of `{"label","detail","insertText"}`).
    CompletionItems { path: String, items: String, buf_key: Option<usize>, revision: u64 },
    /// Parsed rename result (WorkspaceEdit JSON).
    RenameResult { path: String, edit: String, buf_key: Option<usize>, revision: u64 },
    /// Progress notification from `$/progress`.
    Progress { token: String, message: String, percentage: String },
    /// A language server process exited (crash or termination).
    ServerExited { language: String },
    /// The awaited initialize handshake finished off-thread.
    Started { language: String, result: Result<Arc<LspHandle>, String> },
    /// The server asked us to apply a workspace edit (applyEdit).
    ApplyEdit { language: String, request_id: u64, edit_json: String },
}
