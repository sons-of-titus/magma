//! Diagnostic decoration commands.
//!
//! Selects the highest-severity diagnostic per line from the typed
//! diagnostics the kernel already stores on the buffer — display strings
//! are never re-parsed.

use std::collections::HashMap;

use crate::command::args::{ArgSpec, ArgType};
use crate::event::keys;
use crate::event::payload::DecorationChangedPayload;
use crate::state::Editor;
use crate::text_engine::diagnostic::DiagnosticSeverity;

const LAYER: &str = "lsp-diag-eol";

fn severity_rank(sev: DiagnosticSeverity) -> u8 {
    match sev {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        _ => 2,
    }
}

fn severity_face(sev: DiagnosticSeverity) -> &'static str {
    match sev {
        DiagnosticSeverity::Error => "error-face",
        DiagnosticSeverity::Warning => "warning-face",
        _ => "comment-face",
    }
}

pub(super) fn register(editor: &mut Editor) {
    editor.commands.register_fn(
        "diagnostics-apply-eol",
        "Set end-of-line decorations showing the highest-severity diagnostic per line",
        vec![ArgSpec::new("buffer", ArgType::Integer)],
        |editor, args| {
            let buf_id =
                args.get("buffer")
                    .and_then(|a| a.as_integer())
                    .ok_or_else(|| "a buffer id is required".to_string())? as usize;
            let best: Vec<(usize, DiagnosticSeverity, String)> = {
                let Some(arc) = editor.buffers.get(buf_id) else {
                    return Ok(());
                };
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let mut best: HashMap<usize, (u8, DiagnosticSeverity, String)> = HashMap::new();
                for d in &buf.lsp_diagnostics {
                    let rank = severity_rank(d.severity);
                    match best.get(&d.line) {
                        Some(&(prev, _, _)) if prev <= rank => {}
                        _ => {
                            best.insert(d.line, (rank, d.severity, d.message.clone()));
                        }
                    }
                }
                let mut out: Vec<_> = best
                    .into_iter()
                    .map(|(line, (_, sev, msg))| (line, sev, msg))
                    .collect();
                out.sort_by_key(|(line, _, _)| *line);
                out
            };
            editor.for_each_view_of_buffer_mut(buf_id, |view| {
                view.decor_clear_layer(LAYER);
                for (line, sev, msg) in &best {
                    view.decor_set_eol(
                        LAYER,
                        *line,
                        format!(" {msg}"),
                        severity_face(*sev).to_string(),
                    );
                }
            });
            for _ in &best {
                editor.events.emit_typed(
                    keys::events::DECORATION_CHANGED,
                    DecorationChangedPayload {
                        buffer: buf_id.to_string(),
                        layer: LAYER.to_string(),
                    },
                );
            }
            Ok(())
        },
    );
}
