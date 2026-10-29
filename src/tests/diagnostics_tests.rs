//! Diagnostics decoration command — highest severity per line, from the
//! typed diagnostics stored on the buffer.

use crate::kernel::command;
use crate::kernel::command::args::ArgValue;
use crate::kernel::text_engine::diagnostic::{Diagnostic, DiagnosticSeverity};
use crate::tests::helpers;

fn apply(ed: &mut crate::kernel::state::Editor) {
    let key = helpers::focused_key(ed);
    let mut args = std::collections::HashMap::new();
    args.insert("buffer".to_string(), ArgValue::Integer(key as i64));
    command::execute_command(ed, "diagnostics-apply-eol", &args).unwrap();
}

fn diag(sev: DiagnosticSeverity, line: usize, msg: &str) -> Diagnostic {
    Diagnostic {
        message: msg.to_string(),
        severity: sev,
        line,
        column: 0,
        end_line: line,
        end_column: 0,
    }
}

fn eol_entries(
    ed: &crate::kernel::state::Editor,
) -> Vec<(usize, String, String)> {
    let key = helpers::focused_key(ed);
    let view = ed.views.get(&key).unwrap();
    view.decoration_layers
        .get("lsp-diag-eol")
        .map(|v| {
            let mut out = Vec::new();
            for d in v {
                if let crate::kernel::text_engine::Decoration::EndOfLine {
                    line,
                    text,
                    face,
                } = d
                {
                    out.push((*line, text.clone(), face.clone()));
                }
            }
            out
        })
        .unwrap_or_default()
}

#[test]
fn apply_eol_keeps_highest_severity_per_line() {
    let mut ed = helpers::make_editor_with_buffer("aaa\nbbb\nccc");
    let key = helpers::focused_key(&ed);
    ed.buffers.get(key).unwrap().lock().unwrap().lsp_diagnostics = vec![
        diag(DiagnosticSeverity::Warning, 0, "warn msg"),
        diag(DiagnosticSeverity::Error, 0, "error msg"),
        diag(DiagnosticSeverity::Info, 2, "info msg"),
    ];
    apply(&mut ed);
    let entries = eol_entries(&ed);
    assert_eq!(entries.len(), 2, "two lines carry diagnostics");
    assert!(
        entries.contains(&(0, " error msg".to_string(), "error-face".to_string())),
        "error must win line 0, got {entries:?}"
    );
    assert!(
        entries.contains(&(2, " info msg".to_string(), "comment-face".to_string())),
        "info maps to comment-face, got {entries:?}"
    );
}

#[test]
fn apply_eol_clears_stale_decorations_when_diagnostics_empty() {
    let mut ed = helpers::make_editor_with_buffer("aaa\nbbb");
    let key = helpers::focused_key(&ed);
    ed.buffers.get(key).unwrap().lock().unwrap().lsp_diagnostics =
        vec![diag(DiagnosticSeverity::Error, 0, "gone")];
    apply(&mut ed);
    assert_eq!(eol_entries(&ed).len(), 1);
    // A publish with no diagnostics must clear the layer, not leave the
    // old annotations behind.
    ed.buffers.get(key).unwrap().lock().unwrap().lsp_diagnostics = vec![];
    apply(&mut ed);
    assert!(eol_entries(&ed).is_empty());
}
