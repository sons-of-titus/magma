//! LSP background-event handlers — diagnostics, completions, hover,
//! definition, code actions, rename, progress, and server lifecycle.

use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;
use crate::state::overlay;

use super::super::BackgroundEvent;

/// True when the buffer the request was made against is still at the
/// revision the response was computed for — the stale-result gate
/// (rework.md §2).  Requests without document context pass.
fn revision_current(ed: &Editor, buf_key: Option<usize>, revision: u64) -> bool {
    match buf_key {
        None => true,
        Some(key) => ed
            .buffers
            .get(key)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision() == revision)
            .unwrap_or(false),
    }
}

/// Match a publishDiagnostics URI against a buffer path (LSP sends file URIs).
fn lsp_path_matches(buf_path: Option<&str>, diag_path: &str) -> bool {
    match buf_path {
        Some(p) => crate::semantic::lsp_proto::uri_matches_path(diag_path, p),
        None => false,
    }
}

/// A scope paints only when presentation has claimed it — a Janet-registered
/// scope-face or a face of the same name.  Dropping unclaimed scopes keeps a
/// server's unmapped token types from repainting text in the generic
/// highlight colors.
fn scope_resolvable(ed: &Editor, scope: &str) -> bool {
    ed.scope_faces
        .iter()
        .any(|(prefix, _)| scope.starts_with(prefix.as_str()))
        || ed.faces.contains_key(scope)
}

/// Byte end of the span starting at `start` that covers `len_utf16` UTF-16
/// code units — astral chars count as two, matching how LSP measures.
fn utf16_span_end(buf: &crate::text_engine::Buffer, start: usize, len_utf16: usize) -> usize {
    let text = buf.slice(start, buf.len());
    let mut units = 0usize;
    for (i, c) in text.char_indices() {
        if units >= len_utf16 {
            return start + i;
        }
        units += c.len_utf16();
    }
    start + text.len()
}

/// Semantic tokens land in the "semantic" highlight layer (after syntax).
/// Each token contributes its type scope, then one range per modifier so a
/// modifier's styling paints over the type's.
fn apply_semantic_tokens(
    ed: &mut Editor,
    buf_key: usize,
    tokens: &[crate::semantic::lsp::SemanticToken],
) {
    let Some(arc) = ed.buffers.get(buf_key) else {
        return;
    };
    let mut ranges: Vec<(usize, usize, String)> = Vec::new();
    {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        for t in tokens {
            let start =
                crate::semantic::edit_apply::offset_of_lsp_position(&buf, t.line, t.col_utf16);
            let end = utf16_span_end(&buf, start, t.len_utf16);
            if end <= start {
                continue;
            }
            ranges.push((start, end, t.token_type.clone()));
            for m in &t.modifiers {
                ranges.push((start, end, m.clone()));
            }
        }
    }
    ranges.retain(|(_, _, scope)| scope_resolvable(ed, scope));
    ed.for_each_view_of_buffer_mut(buf_key, |view| {
        view.set_highlights_layer("semantic", ranges.clone());
    });
}

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::Lsp(event) => handle_lsp_event(ed, event),
        _ => {}
    }
}

fn handle_lsp_event(ed: &mut Editor, event: crate::semantic::Event) {
    use crate::semantic::Event;
    match event {
        Event::Diagnostics {
            path,
            version,
            diagnostics,
        } => {
            // A publish tagged with a version older than the buffer's current
            // revision describes text that no longer exists — drop it.  The
            // didChange that bumped the revision produces a fresh publish;
            // until then the previous diagnostics stay.
            let stale = version.is_some_and(|v| {
                ed.buffers.iter().any(|(_, arc)| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    lsp_path_matches(buf.path.as_deref(), &path) && buf.revision() > v
                })
            });
            if stale {
                return;
            }
            let count = diagnostics.len();
            // Typed diagnostics live in the semantic engine (canonical);
            // the buffer keeps display strings only.  The engine keys by
            // raw buffer path — decode the URI before storing so readers
            // querying by `buf.path` match.
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
            ed.semantic
                .update_diagnostics(&crate::semantic::lsp_proto::uri_to_path(&path), diagnostics);
            if let Some(buf) = buffer_id {
                crate::inlay_hints::request(ed, buf);
            }
            ed.events.emit_typed(
                keys::events::LSP_DIAGNOSTICS,
                LspDiagnosticsPayload {
                    buffer_id: buffer_id.map(|k| k.to_string()).unwrap_or_default(),
                    path,
                    count: count.to_string(),
                    items: display.join("\n"),
                },
            );
        }
        Event::Completion {
            path,
            items,
            request_id: _,
        } => {
            ed.events.emit_typed(
                keys::events::LSP_COMPLETION,
                LspCompletionPayload {
                    path,
                    items: items.join("\n"),
                },
            );
        }
        Event::Response {
            method,
            result,
            buf_key,
            revision,
            ..
        } if method == "textDocument/inlayHint" && revision_current(ed, buf_key, revision) => {
            if let Some(buf) = buf_key {
                crate::inlay_hints::apply(ed, buf, revision, &result);
            }
        }
        Event::Response {
            path,
            method,
            result,
            request_id: _,
            buf_key,
            revision,
        } if revision_current(ed, buf_key, revision) => {
            ed.events.emit_typed(
                keys::events::LSP_RESPONSE,
                LspResponsePayload {
                    path,
                    method,
                    result,
                },
            );
        }
        Event::RequestTimedOut {
            method,
            buf_key: _,
            revision: _,
        } => {
            ed.show_message(
                crate::state::MessageKind::Info,
                format!("language server: {method} timed out"),
            );
        }
        Event::ServerExited { language } => {
            ed.lsp.exited(&language);
            ed.show_message(
                crate::state::MessageKind::Error,
                format!("language server for {language} exited"),
            );
            // Janet's attach table still believes the server is live; the
            // reset event is what lets it re-attach (and respawn) the
            // language.  Bounded by the restart budget so a crash-looping
            // server cannot respawn forever.
            if ed.lsp.allow_restart(&language) {
                let mut data = std::collections::HashMap::new();
                data.insert("language".to_string(), language);
                ed.events.emit("lsp-server-exited", data);
            }
        }
        Event::Started { language, result } => match result {
            Ok(handle) => {
                ed.lsp.insert(&language, handle);
                ed.lsp.note_ready(&language);
                let mut data = std::collections::HashMap::new();
                data.insert("language".to_string(), language);
                ed.events.emit("lsp-initialized", data);
            }
            Err(e) => {
                ed.show_message(crate::state::MessageKind::Error, e);
                // A failed handshake never produces a handle; clear the
                // spawn slot and let the attach layer retry within budget.
                ed.lsp.exited(&language);
                if ed.lsp.allow_restart(&language) {
                    let mut data = std::collections::HashMap::new();
                    data.insert("language".to_string(), language);
                    ed.events.emit("lsp-server-exited", data);
                }
            }
        },
        Event::Hover {
            path,
            contents,
            buf_key,
            revision,
        } => {
            if revision_current(ed, buf_key, revision)
                && !contents.trim().is_empty()
                && let Some(buf_key) = buf_key
            {
                overlay::attach_lsp_hover(ed, buf_key, &contents);
                ed.events
                    .emit_typed(keys::events::LSP_HOVER, LspHoverPayload { path, contents });
            }
        }
        Event::Definition {
            path,
            uri,
            start_line,
            start_col,
            end_line,
            end_col,
            buf_key,
            revision,
        } if revision_current(ed, buf_key, revision) => {
            ed.events.emit_typed(
                keys::events::LSP_DEFINITION,
                LspDefinitionPayload {
                    path,
                    uri,
                    start_line: start_line.to_string(),
                    end_line: end_line.to_string(),
                    start_col: start_col.to_string(),
                    end_col: end_col.to_string(),
                },
            );
        }
        Event::CodeActions {
            path,
            actions,
            buf_key,
            revision,
        } if revision_current(ed, buf_key, revision) => {
            // The quick-fix picker is the primary consumer: open it on
            // the focused buffer when there is anything to choose.
            let parsed = overlay::parse_code_actions(&actions);
            let focused = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.buffer(wid));
            if !parsed.is_empty() && focused == buf_key {
                ed.quickfix_menu = Some(overlay::QuickfixMenu {
                    buf_key: buf_key.unwrap_or(0),
                    items: parsed,
                    idx: 0,
                });
            } else if parsed.is_empty() {
                ed.show_message(crate::state::MessageKind::Info, "no code actions here");
            }
            ed.events.emit_typed(
                keys::events::LSP_CODE_ACTIONS,
                LspCodeActionsPayload { path, actions },
            );
        }
        Event::CompletionItems {
            path,
            items,
            buf_key,
            revision,
        } if revision_current(ed, buf_key, revision) => {
            // The popup is the primary consumer: merge server items in
            // (they lead, duplicates drop).  The event stays available
            // to Janet observers.
            let parsed = crate::completion_lsp::parse_lsp_completion_items(&items);
            if !parsed.is_empty() {
                crate::completion_lsp::merge_lsp_items(ed, parsed);
            }
            ed.events.emit_typed(
                keys::events::LSP_COMPLETION_ITEMS,
                LspCompletionItemsPayload { path, items },
            );
        }
        Event::SignatureHelp {
            path,
            json,
            buf_key,
            revision,
        } if revision_current(ed, buf_key, revision) => {
            // The signature popup is the primary consumer on the focused
            // buffer; a null/empty response hides it (nothing to show here).
            let focused = ed
                .view_tree
                .focused_window()
                .and_then(|wid| ed.view_tree.buffer(wid));
            if focused == buf_key {
                ed.signature = crate::signature::parse_signature_help(&json).unwrap_or_default();
            }
            ed.events.emit_typed(
                keys::events::LSP_SIGNATURE_HELP,
                LspSignatureHelpPayload {
                    path,
                    signatures: json,
                },
            );
        }
        Event::RenameResult {
            path,
            edit,
            buf_key,
            revision,
        } if revision_current(ed, buf_key, revision) => {
            ed.events.emit_typed(
                keys::events::LSP_RENAME_RESULT,
                LspRenameResultPayload { path, edit },
            );
        }
        Event::SemanticTokens {
            tokens,
            buf_key,
            revision,
            ..
        } if revision_current(ed, buf_key, revision) => {
            if let Some(buf_key) = buf_key {
                apply_semantic_tokens(ed, buf_key, &tokens);
            }
        }
        Event::Progress {
            token,
            message,
            percentage,
        } => {
            ed.events.emit_typed(
                keys::events::LSP_PROGRESS,
                LspProgressPayload {
                    token,
                    message,
                    percentage,
                },
            );
        }
        Event::ApplyEdit {
            language,
            request_id,
            edit_json,
        } => {
            let applied = match serde_json::from_str::<serde_json::Value>(&edit_json) {
                Ok(v) => {
                    crate::semantic::edit_apply::apply_workspace_edit(ed, &v);
                    true
                }
                Err(e) => { logging::error!("lsp.rs:386: {e}"); false }
            };
            if let Some(server) = ed.lsp.get(&language) {
                server.respond(request_id, serde_json::json!({ "applied": applied }));
            }
        }
        // Revision-gated arms fall through here when the buffer moved on.
        _ => {}
    }
}
