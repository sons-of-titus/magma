//! Regression tests: F-18 (publishDiagnostics version gate) and F-13's
//! keyword-context quickfix surfacing.

use std::collections::HashMap;

use crate::kernel::runtime::{self, BackgroundEvent};
use crate::kernel::semantic::provider::{Diagnostic, DiagnosticSeverity};
use crate::tests::helpers;

fn diag(msg: &str, line: usize) -> Diagnostic {
    Diagnostic {
        message: msg.to_string(),
        severity: DiagnosticSeverity::Error,
        line,
        column: 0,
        end_line: line,
        end_column: 5,
    }
}

fn buffer_at_path(ed: &mut crate::kernel::state::Editor, path: &str) -> usize {
    let key = ed.create_buffer("diag-target");
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap_or_else(|e| e.into_inner()).path = Some(path.to_string());
    }
    key
}

#[test]
fn diagnostics_with_stale_version_are_dropped() {
    let mut ed = helpers::make_editor();
    let key = buffer_at_path(&mut ed, "/tmp/stale.rs");
    let rev_at_request = ed
        .buffers
        .get(key)
        .unwrap()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .revision();
    // The buffer moves on after the request was sent — the publish that
    // finally arrives describes the old text.
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(0, "more text ");
    }

    runtime::process_background_event(
        &mut ed,
        BackgroundEvent::Lsp(crate::kernel::semantic::Event::Diagnostics {
            path: "/tmp/stale.rs".to_string(),
            version: Some(rev_at_request),
            diagnostics: vec![diag("stale", 0)],
        }),
    );
    assert!(
        ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_diagnostics()
            .is_empty(),
        "a publish older than the buffer revision must not land"
    );
    assert!(
        ed.semantic.diagnostics_for_path("/tmp/stale.rs").is_empty(),
        "the semantic store must not receive stale diagnostics either"
    );
}

#[test]
fn diagnostics_with_current_version_apply() {
    let mut ed = helpers::make_editor();
    let key = buffer_at_path(&mut ed, "/tmp/current.rs");
    let rev = ed
        .buffers
        .get(key)
        .unwrap()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .revision();

    runtime::process_background_event(
        &mut ed,
        BackgroundEvent::Lsp(crate::kernel::semantic::Event::Diagnostics {
            path: "/tmp/current.rs".to_string(),
            version: Some(rev),
            diagnostics: vec![diag("fresh", 1)],
        }),
    );
    assert_eq!(
        ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_diagnostics()
            .len(),
        1
    );
    assert_eq!(ed.semantic.diagnostics_for_path("/tmp/current.rs").len(), 1);
}

#[test]
fn diagnostics_without_version_apply() {
    let mut ed = helpers::make_editor();
    let key = buffer_at_path(&mut ed, "/tmp/nover.rs");

    runtime::process_background_event(
        &mut ed,
        BackgroundEvent::Lsp(crate::kernel::semantic::Event::Diagnostics {
            path: "/tmp/nover.rs".to_string(),
            version: None,
            diagnostics: vec![diag("unversioned", 2)],
        }),
    );
    assert_eq!(
        ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_diagnostics()
            .len(),
        1
    );
}

#[test]
fn keyword_context_fills_the_search_list() {
    let mut ed = helpers::make_editor_with_buffer(
        "fn main() {}\nlet x = main;\nplain line\nuse main here\n",
    );
    // Cursor on the "main" word of line 1 (offset 3).
    if let Some(view) = ed.view_of_buffer_mut(helpers::focused_key(&ed)) {
        view.set_cursor(3);
    }
    crate::kernel::command::execute_command(&mut ed, "keyword-context", &HashMap::new()).unwrap();

    let lines: Vec<usize> = ed.search.results.iter().map(|e| e.line).collect();
    assert_eq!(
        lines,
        vec![1, 2, 4],
        "every line containing the word lands as a find entry"
    );
    assert_eq!(ed.search.index, 0);
    for entry in &ed.search.results {
        assert_eq!(
            entry.filename, "test",
            "entries carry the buffer name when no path"
        );
    }
}

#[test]
fn keyword_context_with_no_other_matches_lists_only_the_origin_line() {
    let mut ed = helpers::make_editor_with_buffer("unique words only here\n");
    if let Some(view) = ed.view_of_buffer_mut(helpers::focused_key(&ed)) {
        view.set_cursor(0); // "unique"
    }
    crate::kernel::command::execute_command(&mut ed, "keyword-context", &HashMap::new()).unwrap();
    assert_eq!(
        ed.search.results.len(),
        1,
        "the word's own line is the one match"
    );
    // The status message is the visible surface (nothing goes to stderr).
    let msg = ed
        .status_message
        .as_ref()
        .map(|m| m.text.clone())
        .unwrap_or_default();
    assert!(msg.contains("unique"), "the status message names the word");
}
