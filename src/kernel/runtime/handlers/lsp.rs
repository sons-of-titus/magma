//! LSP background-event handlers — diagnostics, completions, hover,
//! definition, code actions, rename, progress, and server lifecycle.

use crate::kernel::state::overlay;
use crate::kernel::state::Editor;
use crate::kernel::event::keys;
use crate::kernel::event::payload::*;

use super::super::BackgroundEvent;

/// True when the buffer the request was made against is still at the
/// revision the response was computed for — the stale-result gate
/// (rework.md §2).  Requests without document context pass.
fn revision_current(ed: &Editor, buf_key: Option<usize>, revision: u64) -> bool {
    match buf_key {
        None => true,
        Some(key) => ed.buffers.get(key)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision() == revision)
            .unwrap_or(false),
    }
}

/// Match a publishDiagnostics URI against a buffer path (LSP sends file URIs).
fn lsp_path_matches(buf_path: Option<&str>, diag_path: &str) -> bool {
    let diag = diag_path.strip_prefix("file://").unwrap_or(diag_path);
    buf_path == Some(diag) || buf_path == Some(diag_path)
}

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::LspDiagnostics { path, version, diagnostics } => {
            // A publish tagged with a version older than the buffer's current
            // revision describes text that no longer exists — drop it.  The
            // didChange that bumped the revision produces a fresh publish;
            // until then the previous diagnostics stay.
            let stale = version.is_some_and(|v| ed.buffers.iter().any(|(_, arc)| {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                lsp_path_matches(buf.path.as_deref(), &path) && buf.revision() > v
            }));
            if stale {
                return;
            }
            let count = diagnostics.len();
            // Typed diagnostics live in the semantic engine (canonical);
            // the buffer keeps display strings only.  URI vs path is
            // normalized so servers sending file:// URIs still land.
            let display: Vec<String> = diagnostics.iter().map(|d| d.display()).collect();
            let mut buffer_id: Option<usize> = None;
            for (key, arc) in ed.buffers.iter_mut() {
                let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                if lsp_path_matches(buf.path.as_deref(), &path) {
                    buf.set_lsp_diagnostics(diagnostics.clone());
                    buffer_id = Some(key);
                    break;
                }
            }
            ed.semantic.update_diagnostics(&path, diagnostics);
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
        BackgroundEvent::LspResponse { path, method, result, request_id: _, buf_key, revision }
            if revision_current(ed, buf_key, revision) => {
                ed.events.emit_typed(keys::events::LSP_RESPONSE, LspResponsePayload { path, method, result });
            }
        BackgroundEvent::LspServerExited { language } => {
            ed.lsp.exited(&language);
            ed.show_message(crate::kernel::state::MessageKind::Error,
                format!("language server for {language} exited"));
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
            if revision_current(ed, buf_key, revision) && !contents.trim().is_empty()
                && let Some(buf_key) = buf_key {
                    overlay::attach_lsp_hover(ed, buf_key, &contents);
                    ed.events.emit_typed(keys::events::LSP_HOVER, LspHoverPayload { path, contents });
            }
        }
        BackgroundEvent::LspDefinition { path, uri, start_line, start_col, end_line, end_col, buf_key, revision }
            if revision_current(ed, buf_key, revision) => {
                ed.events.emit_typed(keys::events::LSP_DEFINITION, LspDefinitionPayload {
                    path,
                    uri,
                    start_line: start_line.to_string(),
                    end_line: end_line.to_string(),
                    start_col: start_col.to_string(),
                    end_col: end_col.to_string(),
                });
            }
        BackgroundEvent::LspCodeActions { path, actions, buf_key, revision }
            if revision_current(ed, buf_key, revision) => {
                // The quick-fix picker is the primary consumer: open it on
                // the focused buffer when there is anything to choose.
                let parsed = overlay::parse_code_actions(&actions);
                let focused = ed.view_tree.focused_window()
                    .and_then(|wid| ed.view_tree.buffer(wid));
                if !parsed.is_empty() && focused == buf_key {
                    ed.quickfix_menu = Some(overlay::QuickfixMenu {
                        buf_key: buf_key.unwrap_or(0),
                        items: parsed,
                        idx: 0,
                    });
                } else if parsed.is_empty() {
                    ed.show_message(crate::kernel::state::MessageKind::Info, "no code actions here");
                }
                ed.events.emit_typed(keys::events::LSP_CODE_ACTIONS, LspCodeActionsPayload { path, actions });
            }
        BackgroundEvent::LspCompletionItems { path, items, buf_key, revision }
            if revision_current(ed, buf_key, revision) => {
                // The popup is the primary consumer: merge server items in
                // (they lead, duplicates drop).  The event stays available
                // to Janet observers.
                let parsed = crate::kernel::completion::parse_lsp_completion_items(&items);
                if !parsed.is_empty() {
                    crate::kernel::completion::merge_lsp_items(ed, parsed);
                }
                ed.events.emit_typed(keys::events::LSP_COMPLETION_ITEMS, LspCompletionItemsPayload { path, items });
            }
        BackgroundEvent::LspRenameResult { path, edit, buf_key, revision }
            if revision_current(ed, buf_key, revision) => {
                ed.events.emit_typed(keys::events::LSP_RENAME_RESULT, LspRenameResultPayload { path, edit });
            }
        BackgroundEvent::LspProgress { token, message, percentage } => {
            ed.events.emit_typed(keys::events::LSP_PROGRESS, LspProgressPayload { token, message, percentage });
        }
        BackgroundEvent::LspApplyEdit { language, request_id, edit_json } => {
            let applied = match serde_json::from_str::<serde_json::Value>(&edit_json) {
                Ok(v) => {
                    crate::kernel::semantic::edit_apply::apply_workspace_edit(ed, &v);
                    true
                }
                Err(_) => false,
            };
            if let Some(server) = ed.lsp.get(&language) {
                server.respond(request_id, serde_json::json!({ "applied": applied }));
            }
        }
        _ => {}
    }
}
