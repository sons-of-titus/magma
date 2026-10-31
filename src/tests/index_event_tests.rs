//! Regression tests for the index subsystem's namespaced background events.
//!
//! `index::Event` owns project-index walk batches, per-file records, and
//! stamp scans; `BackgroundEvent::Index(_)` carries them on the one
//! foreground channel.  These tests pin the wrapper payloads, the
//! generation guard, and dispatch effects.

use std::sync::{Arc, Mutex};

use crate::kernel::index::Event as IndexEvent;
use crate::kernel::project::Project;
use crate::kernel::runtime::{BackgroundEvent, process_background_event};

/// Make `demo` the active project at `generation`, the way `open_root` does.
fn activate_demo(ed: &mut crate::kernel::state::Editor, generation: u64) {
    ed.project_manager.project = Project {
        name: Some("demo".into()),
        generation,
        ..Project::default()
    };
    ed.project_manager.current_project = Some("demo".into());
    ed.project_manager.projects.insert(
        "demo".to_string(),
        Project {
            name: Some("demo".into()),
            generation,
            ..Project::default()
        },
    );
}

#[test]
fn index_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Index(IndexEvent::ProjectIndexed {
        project_name: "demo".into(),
        generation: 1,
        files: vec!["src/main.rs".into(), "src/lib.rs".into()],
        ignored_files: vec![],
        done: false,
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/a".into(),
        content: Ok("x".into()),
        request_id: 3,
    });
    bg.send_event(BackgroundEvent::Index(IndexEvent::RecordArrived {
        generation: 1,
        record: crate::kernel::index::record_from_text("src/main.rs", 1, "fn main() {}"),
    }));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 4, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Index(IndexEvent::ProjectIndexed { project_name, generation, files, done, .. })
            if project_name.as_str() == "demo" && *generation == 1 && files.len() == 2 && !done
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 3, content: Ok(body), .. }
            if body.as_str() == "x"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Index(IndexEvent::RecordArrived { generation, record })
            if *generation == 1 && record.path.as_str() == "src/main.rs" && !record.symbols.is_empty()
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
    activate_demo(&mut ed, 1);
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    ed.events.on(
        crate::kernel::event::keys::events::FILE_INDEXED,
        move |data| {
            *cap.lock().unwrap_or_else(|e| e.into_inner()) = data.get("count").cloned();
            None
        },
    );

    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::ProjectIndexed {
            project_name: "demo".into(),
            generation: 1,
            files: vec!["src/main.rs".into()],
            ignored_files: vec![],
            done: true,
        }),
    );
    ed.events.drain_and_dispatch();

    let proj = ed
        .project_manager
        .projects
        .get("demo")
        .expect("project stays");
    assert_eq!(proj.files, vec!["src/main.rs".to_string()]);
    assert!(!proj.file_index_dirty);
    assert!(!proj.indexing, "the final batch clears the loading state");
    assert_eq!(
        captured
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref(),
        Some("1")
    );
}

#[test]
fn walk_batches_apply_incrementally_and_stale_generations_are_dropped() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    activate_demo(&mut ed, 2);

    // A batch from the previous open (generation 1) must not touch state.
    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::ProjectIndexed {
            project_name: "demo".into(),
            generation: 1,
            files: vec!["ghost.rs".into()],
            ignored_files: vec![],
            done: true,
        }),
    );
    assert!(
        ed.project_manager.project.files.is_empty(),
        "a superseded walk never overwrites the active project"
    );
    assert_eq!(ed.project_manager.project.files_epoch, 0);

    // Current-generation batches replace (they carry the accumulated list)
    // and the final batch clears the loading flag.
    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::ProjectIndexed {
            project_name: "demo".into(),
            generation: 2,
            files: vec!["a.rs".into(), "b.rs".into()],
            ignored_files: vec![],
            done: false,
        }),
    );
    assert_eq!(
        ed.project_manager.project.files,
        vec!["a.rs".to_string(), "b.rs".to_string()]
    );
    assert!(ed.project_manager.project.indexing);
    assert_eq!(ed.project_manager.project.files_epoch, 1);

    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::ProjectIndexed {
            project_name: "demo".into(),
            generation: 2,
            files: vec!["a.rs".into(), "b.rs".into(), "c.rs".into()],
            ignored_files: vec!["vendored".into()],
            done: true,
        }),
    );
    assert_eq!(ed.project_manager.project.files.len(), 3);
    assert!(!ed.project_manager.project.indexing);
    assert_eq!(ed.project_manager.project.files_epoch, 2);
}

#[test]
fn index_record_dispatch_merges_into_the_index_store() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    activate_demo(&mut ed, 0);
    let before = ed.index.stats().files;

    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::RecordArrived {
            generation: 0,
            record: crate::kernel::index::record_from_text(
                "src/dispatch.rs",
                1,
                "fn dispatched() {}",
            ),
        }),
    );

    let stats = ed.index.stats();
    assert_eq!(stats.files, before + 1);
    assert_eq!(stats.symbols, 1);
}

#[test]
fn records_from_a_superseded_generation_never_reach_the_store() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    activate_demo(&mut ed, 3);

    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::RecordArrived {
            generation: 2,
            record: crate::kernel::index::record_from_text("stale.rs", 1, "fn stale() {}"),
        }),
    );

    assert_eq!(
        ed.index.stats().files,
        0,
        "an in-flight extraction from an older project must not pollute the new store"
    );
}

/// Wait for the save-path extraction job to publish its record.
fn recv_until_index_event(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
) -> IndexEvent {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Ok(BackgroundEvent::Index(event)) = rx.try_recv() {
            return event;
        }
        if std::time::Instant::now() > deadline {
            panic!("save-path extraction never published an event");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn saving_a_file_reindexes_only_that_file_with_no_walk() {
    let root = std::env::temp_dir().join("magma_save_reindex");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let path = root.join("src/lib.rs");
    std::fs::write(&path, "fn fresh_after_save() {}\n").unwrap();

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    ed.project_manager.project = Project {
        name: Some("demo".into()),
        root: Some(root.clone()),
        generation: 1,
        ..Project::default()
    };
    ed.project_manager.current_project = Some("demo".into());

    crate::kernel::index::reindex_saved_file(&mut ed, &path.to_string_lossy());

    match recv_until_index_event(&mut rx) {
        IndexEvent::RecordArrived { generation, record } => {
            assert_eq!(generation, 1);
            assert_eq!(record.path, "src/lib.rs");
            assert!(
                record.symbols.iter().any(|s| s.name == "fresh_after_save"),
                "the saved file's new symbols are extracted: {:?}",
                record.symbols
            );
        }
        other => panic!("expected RecordArrived from the save path, got {other:?}"),
    }

    // Files outside the project never enqueue anything.
    let outside = std::env::temp_dir().join("magma_outside.rs");
    std::fs::write(&outside, "fn outside() {}\n").unwrap();
    crate::kernel::index::reindex_saved_file(&mut ed, &outside.to_string_lossy());
    let drained = {
        let mut found = false;
        while let Ok(BackgroundEvent::Index(IndexEvent::RecordArrived { .. })) = rx.try_recv() {
            found = true;
        }
        found
    };
    assert!(!drained, "an out-of-project save must not reach the index");

    let _ = std::fs::remove_dir_all(&root);
    std::fs::remove_file(&outside).ok();
}
