//! Regression tests for the index subsystem's namespaced background events.
//!
//! `index::Event` owns project-index completion and per-file records;
//! `BackgroundEvent::Index(_)` carries them on the one foreground channel.
//! These tests pin the wrapper payloads, cross-subsystem ordering, and
//! dispatch effects.

use std::sync::{Arc, Mutex};

use crate::kernel::index::Event as IndexEvent;
use crate::kernel::runtime::{BackgroundEvent, process_background_event};
use crate::kernel::project::Project;

#[test]
fn index_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Index(IndexEvent::ProjectIndexed {
        project_name: "demo".into(),
        files: vec!["src/main.rs".into(), "src/lib.rs".into()],
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/a".into(),
        content: Ok("x".into()),
        request_id: 3,
    });
    bg.send_event(BackgroundEvent::Index(IndexEvent::RecordArrived(
        crate::kernel::index::record_from_text("src/main.rs", 1, "fn main() {}"),
    )));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 4, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Index(IndexEvent::ProjectIndexed { project_name, files })
            if project_name.as_str() == "demo" && files.len() == 2
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 3, content: Ok(body), .. }
            if body.as_str() == "x"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Index(IndexEvent::RecordArrived(rec))
            if rec.path.as_str() == "src/main.rs" && !rec.symbols.is_empty()
    ));
    assert!(matches!(
        &seen[3],
        BackgroundEvent::Custom(name, payload)
            if name.as_str() == "ping" && payload.as_str() == "pong"
    ));
}

#[test]
fn project_indexed_dispatch_updates_the_registry_and_emits_the_bus_event() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    ed.project_manager.projects.insert("demo".to_string(), Project {
        name: Some("demo".to_string()),
        ..Project::default()
    });
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    ed.events.on(crate::kernel::event::keys::events::FILE_INDEXED, move |data| {
        *cap.lock().unwrap_or_else(|e| e.into_inner()) = data.get("count").cloned();
        None
    });

    process_background_event(&mut ed, BackgroundEvent::Index(IndexEvent::ProjectIndexed {
        project_name: "demo".into(),
        files: vec!["src/main.rs".into()],
    }));
    ed.events.drain_and_dispatch();

    let proj = ed.project_manager.projects.get("demo").expect("project stays");
    assert_eq!(proj.files, vec!["src/main.rs".to_string()]);
    assert!(!proj.file_index_dirty);
    assert_eq!(captured.lock().unwrap_or_else(|e| e.into_inner()).as_deref(), Some("1"));
}

#[test]
fn index_record_dispatch_merges_into_the_index_store() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let before = ed.index.stats().files;

    process_background_event(&mut ed, BackgroundEvent::Index(IndexEvent::RecordArrived(
        crate::kernel::index::record_from_text("src/dispatch.rs", 1, "fn dispatched() {}"),
    )));

    let stats = ed.index.stats();
    assert_eq!(stats.files, before + 1);
    assert_eq!(stats.symbols, 1);
}
