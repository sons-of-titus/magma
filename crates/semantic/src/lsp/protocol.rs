//! Inbound LSP message handling: notifications, server requests, and
//! response correlation into [`Event`]s.

use std::sync::Arc;

use logging::debug;
use serde_json::Value;

use super::semantic_tokens::{decode_semantic_tokens, semantic_token_legend};
use super::{Core, Event, EventSink, Pending, ServerState};
use crate::lsp_proto::Inbound;
use crate::provider::{Diagnostic, DiagnosticSeverity};

pub(super) fn dispatch<S: EventSink>(core: &Arc<Core>, language_id: &str, sink: &S, msg: Value) {
    let inbound = if let Some(id) = msg.get("id").and_then(Value::as_u64) {
        if let Some(method) = msg.get("method").and_then(Value::as_str) {
            Inbound::Request {
                id,
                method: method.into(),
                params: msg.get("params").cloned().unwrap_or(Value::Null),
            }
        } else {
            let error = msg.get("error").cloned();
            Inbound::Response {
                id,
                result: msg.get("result").cloned(),
                error,
            }
        }
    } else if let Some(method) = msg.get("method").and_then(Value::as_str) {
        Inbound::Notification {
            method: method.into(),
            params: msg.get("params").cloned().unwrap_or(Value::Null),
        }
    } else {
        return;
    };

    match inbound {
        Inbound::Notification { method, params } => match method.as_str() {
            "textDocument/publishDiagnostics" => {
                let uri = params
                    .pointer("/uri")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let version = params.get("version").and_then(Value::as_u64);
                let diagnostics = parse_diagnostics(&params);
                sink.send(Event::Diagnostics {
                    path: uri,
                    version,
                    diagnostics,
                });
            }
            "$/progress" => {
                let token = params
                    .pointer("/value/token")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let message = params
                    .pointer("/value/message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let percentage = params
                    .pointer("/value/percentage")
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                sink.send(Event::Progress {
                    token,
                    message,
                    percentage,
                });
            }
            _ => {}
        },
        Inbound::Request { id, method, params } => {
            if method == "workspace/applyEdit" {
                // The foreground applies the edit and answers with the
                // truth; the reader thread must not pre-empt it.
                let edit = params.get("edit").cloned().unwrap_or(Value::Null);
                sink.send(Event::ApplyEdit {
                    language: language_id.to_string(),
                    request_id: id,
                    edit_json: edit.to_string(),
                });
                return;
            }
            // Other server→client requests (workspace/configuration, …)
            // get an empty response so the server does not stall waiting.
            logging::LogErr::log_warn(core.outbound.send(serde_json::json!({
                "jsonrpc": "2.0", "id": id, "result": Value::Null,
            })));
        }
        Inbound::Response { id, result, error } => {
            if let Some(err) = error {
                // initialize errors complete the handshake with failure.
                if let Ok(tx) = core.init_signal.lock() {
                    logging::LogErr::log_warn(tx.send(Err(err.to_string())));
                }
                if let Some(p) = core
                    .pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id)
                {
                    // A cancellation means the server moved on (our document
                    // changed under it), not that anything is broken.  Logging
                    // it as an error spams the message log on every keystroke.
                    if is_cancellation(&err) {
                        debug!("LSP {language_id}: {} cancelled", p.method);
                    } else {
                        sink.send(lsp_response_event(
                            language_id,
                            "error",
                            &err.to_string(),
                            p,
                        ));
                    }
                }
                return;
            }
            if id == 1 {
                // The server's semantic-token legend lives only in the
                // initialize result; capture it before the handshake
                // response is consumed.
                if let Some(result) = result.as_ref() {
                    *core.legend.lock().unwrap_or_else(|e| e.into_inner()) =
                        semantic_token_legend(result);
                    *core.sync_kind.lock().unwrap_or_else(|e| e.into_inner()) =
                        text_sync_kind(result);
                }
                // The initialize response completes the awaited handshake.
                if let Ok(tx) = core.init_signal.lock() {
                    logging::LogErr::log_warn(tx.send(Ok(())));
                }
                core.set_state(ServerState::Ready);
                return;
            }
            let pending = core
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
            if let Some(p) = pending {
                let method = p.method.clone();
                let event = match method.as_str() {
                    "textDocument/hover" => Event::Hover {
                        path: language_id.to_string(),
                        contents: extract_hover_text(result.as_ref()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/definition" => {
                        let loc = result.as_ref().and_then(first_location);
                        match loc {
                            Some((uri, sl, sc, el, ec)) => Event::Definition {
                                path: language_id.to_string(),
                                uri,
                                start_line: sl,
                                start_col: sc,
                                end_line: el,
                                end_col: ec,
                                buf_key: p.buf_key,
                                revision: p.revision,
                            },
                            None => lsp_response_event(
                                language_id,
                                "textDocument/definition",
                                "null",
                                p,
                            ),
                        }
                    }
                    "textDocument/codeAction" => Event::CodeActions {
                        path: language_id.to_string(),
                        actions: result.map(|r| r.to_string()).unwrap_or_else(|| "[]".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/completion" => Event::CompletionItems {
                        path: language_id.to_string(),
                        items: result.map(|r| r.to_string()).unwrap_or_else(|| "[]".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/signatureHelp" => Event::SignatureHelp {
                        path: language_id.to_string(),
                        json: result
                            .map(|r| r.to_string())
                            .unwrap_or_else(|| "null".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/rename" => Event::RenameResult {
                        path: language_id.to_string(),
                        edit: result
                            .map(|r| r.to_string())
                            .unwrap_or_else(|| "null".into()),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    "textDocument/semanticTokens/full" => Event::SemanticTokens {
                        path: language_id.to_string(),
                        tokens: result
                            .as_ref()
                            .map(|r| {
                                decode_semantic_tokens(
                                    r,
                                    &core.legend.lock().unwrap_or_else(|e| e.into_inner()),
                                )
                            })
                            .unwrap_or_default(),
                        buf_key: p.buf_key,
                        revision: p.revision,
                    },
                    other => {
                        let event = lsp_response_event(
                            language_id,
                            other,
                            &result.map(|r| r.to_string()).unwrap_or_default(),
                            p,
                        );
                        event
                    }
                };
                sink.send(event);
            }
        }
    }
}

/// `textDocumentSync` arrives as a bare kind number or an object with a
/// `change` field; both are legal, and both must reach the didChange path.
fn text_sync_kind(result: &Value) -> Option<u8> {
    let sync = result.get("textDocumentSync")?;
    let raw = match sync {
        Value::Number(n) => n.as_f64(),
        Value::Object(o) => o.get("change").and_then(|c| c.as_f64()),
        _ => None,
    }?;
    if (0.0..=2.0).contains(&raw) {
        Some(raw as u8)
    } else {
        None
    }
}

fn lsp_response_event(language_id: &str, method: &str, result: &str, p: Pending) -> Event {
    Event::Response {
        path: language_id.to_string(),
        method: method.to_string(),
        result: result.to_string(),
        request_id: 0,
        buf_key: p.buf_key,
        revision: p.revision,
    }
}

/// Is this JSON-RPC error a cancellation rather than a failure?
///
/// `-32800` RequestCancelled, `-32801` ContentModified, `-32802`
/// ServerCancelled.  Servers emit these whenever an in-flight request is
/// superseded by an edit; surfacing them as errors turns ordinary typing into
/// a wall of `[lsp/error]` lines.
pub fn is_cancellation(err: &Value) -> bool {
    matches!(
        err.get("code").and_then(Value::as_i64),
        Some(-32802..=-32800)
    )
}

/// Flatten a hover response to display text.
pub fn extract_hover_text(result: Option<&Value>) -> String {
    let contents = result.and_then(|r| r.get("contents"));
    match contents {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o
            .get("value")
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_default(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.get("value").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// First `Location`/`LocationLink` from a definition response.
pub fn first_location(result: &Value) -> Option<(String, usize, usize, usize, usize)> {
    let arr = match result {
        Value::Array(a) => a.first().cloned(),
        Value::Object(_) => Some(result.clone()),
        _ => None,
    }?;
    let target = arr
        .get("targetSelectionRange")
        .or_else(|| arr.get("targetUri").map(|_| &arr))
        .unwrap_or(&arr);
    let uri = target
        .get("uri")
        .or_else(|| arr.get("uri"))?
        .as_str()?
        .to_string();
    let start = target
        .get("range")
        .or_else(|| arr.get("range"))?
        .get("start")?;
    let end = target
        .get("range")
        .or_else(|| arr.get("range"))?
        .get("end")?;
    Some((
        uri,
        start.get("line").and_then(Value::as_u64)? as usize,
        start.get("character").and_then(Value::as_u64)? as usize,
        end.get("line").and_then(Value::as_u64)? as usize,
        end.get("character").and_then(Value::as_u64)? as usize,
    ))
}

/// Typed diagnostics straight off `publishDiagnostics` params.
pub fn parse_diagnostics(params: &Value) -> Vec<Diagnostic> {
    let Some(items) = params.get("diagnostics").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|d| {
            let range = d.get("range")?;
            let start = range.get("start")?;
            let end = range.get("end")?;
            let severity = match d.get("severity").and_then(Value::as_u64) {
                Some(1) => DiagnosticSeverity::Error,
                Some(2) => DiagnosticSeverity::Warning,
                Some(4) => DiagnosticSeverity::Hint,
                _ => DiagnosticSeverity::Info,
            };
            Some(Diagnostic {
                message: d
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                severity,
                line: start.get("line").and_then(Value::as_u64)? as usize,
                column: start.get("character").and_then(Value::as_u64)? as usize,
                end_line: end.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
                end_column: end.get("character").and_then(Value::as_u64).unwrap_or(0) as usize,
            })
        })
        .collect()
}
