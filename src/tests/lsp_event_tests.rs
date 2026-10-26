//! Regression tests for the LSP subsystem's namespaced background events.
//!
//! `semantic::Event` owns the LSP vocabulary; `BackgroundEvent::Lsp(_)` carries
//! it on the one foreground channel.  These tests pin the wrapper payloads,
//! cross-subsystem ordering, and dispatch effects so the split cannot silently
//! drop, duplicate or reorder events.

use std::sync::{Arc, Mutex};

use crate::kernel::runtime::{BackgroundEvent, process_background_event};
use crate::kernel::semantic::Event as LspEvent;
use crate::kernel::semantic::provider::{Diagnostic, DiagnosticSeverity};

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
    let key = ed.create_buffer("lsp-event-target");
    if let Some(arc) = ed.buffers.get(key) {
        arc.lock().unwrap_or_else(|e| e.into_inner()).path = Some(path.to_string());
    }
    key
}

#[test]
fn lsp_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Lsp(LspEvent::Hover {
        path: "rust".into(),
        contents: "hover text".into(),
        buf_key: Some(2),
        revision: 11,
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/a.rs".into(),
        content: Ok("body".into()),
        request_id: 5,
    });
    bg.send_event(BackgroundEvent::Lsp(LspEvent::Progress {
        token: "tok".into(),
        message: "indexing".into(),
        percentage: "50".into(),
    }));
    bg.send_event(BackgroundEvent::Lsp(LspEvent::ServerExited {
        language: "rust".into(),
    }));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 5, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Lsp(LspEvent::Hover { path, contents, buf_key: Some(2), revision: 11 })
            if path.as_str() == "rust" && contents.as_str() == "hover text"
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 5, content: Ok(body), .. }
            if body.as_str() == "body"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Lsp(LspEvent::Progress { token, message, percentage })
            if token.as_str() == "tok" && message.as_str() == "indexing" && percentage.as_str() == "50"
    ));
    assert!(matches!(
        &seen[3],
        BackgroundEvent::Lsp(LspEvent::ServerExited { language })
            if language.as_str() == "rust"
    ));
    assert!(matches!(
        &seen[4],
        BackgroundEvent::Custom(name, payload)
            if name.as_str() == "ping" && payload.as_str() == "pong"
    ));
}

#[test]
fn lsp_diagnostics_dispatch_updates_the_buffer_and_semantic_store() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let key = buffer_at_path(&mut ed, "/tmp/lsp_event.rs");
    ed.frame_dirty = false;

    process_background_event(
        &mut ed,
        BackgroundEvent::Lsp(LspEvent::Diagnostics {
            path: "/tmp/lsp_event.rs".into(),
            version: None,
            diagnostics: vec![diag("bad thing", 3)],
        }),
    );

    assert!(ed.frame_dirty, "dispatch must invalidate the frame");
    let buf = ed.buffers.get(key).unwrap();
    let buf = buf.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.get_diagnostics().len(), 1);
    assert!(buf.get_diagnostics()[0].display().contains("bad thing"));
    assert_eq!(ed.semantic.diagnostics_for_path("/tmp/lsp_event.rs").len(), 1);
}

#[test]
fn lsp_completion_dispatch_reaches_the_event_bus() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    ed.events.on(crate::kernel::event::keys::events::LSP_COMPLETION, move |data| {
        *cap.lock().unwrap_or_else(|e| e.into_inner()) = data.get("items").cloned();
        None
    });

    process_background_event(
        &mut ed,
        BackgroundEvent::Lsp(LspEvent::Completion {
            path: "rust".into(),
            items: vec!["alpha".into(), "beta".into()],
            request_id: 1,
        }),
    );
    ed.events.drain_and_dispatch();

    assert_eq!(
        captured.lock().unwrap_or_else(|e| e.into_inner()).as_deref(),
        Some("alpha\nbeta"),
    );
}
