//! Regression tests for the project subsystem's namespaced background events.
//!
//! `project::Event` owns find-files and project-wide search results;
//! `BackgroundEvent::Project(_)` carries them on the one foreground channel.
//! These tests pin the wrapper payloads, cross-subsystem ordering, and
//! dispatch effects.

use crate::kernel::project::Event as ProjectEvent;
use crate::kernel::runtime::{BackgroundEvent, process_background_event};
use crate::kernel::task::QuickfixEntry;

fn entry(filename: &str, line: usize) -> QuickfixEntry {
    QuickfixEntry {
        filename: filename.into(),
        line,
        col: 1,
        message: "hit".into(),
        ..Default::default()
    }
}

#[test]
fn project_events_round_trip_through_the_shared_channel_in_send_order() {
    let (_ed, bg, mut rx) = crate::tests::helpers::editor_with_background();

    bg.send_event(BackgroundEvent::Project(ProjectEvent::FileSearchListed {
        pattern: "main".into(),
        files: vec!["src/main.rs".into()],
    }));
    bg.send_event(BackgroundEvent::FileReadResult {
        path: "/tmp/a".into(),
        content: Ok("x".into()),
        request_id: 4,
    });
    bg.send_event(BackgroundEvent::Project(ProjectEvent::GrepResultsBatch {
        generation: 2,
        entries: vec![entry("src/lib.rs", 7)],
    }));
    bg.send_event(BackgroundEvent::Project(ProjectEvent::GrepDone {
        generation: 2,
    }));
    bg.send_event(BackgroundEvent::Custom("ping".into(), "pong".into()));

    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }

    assert_eq!(seen.len(), 5, "no event may be dropped or duplicated");
    assert!(matches!(
        &seen[0],
        BackgroundEvent::Project(ProjectEvent::FileSearchListed { pattern, files })
            if pattern.as_str() == "main" && files == &vec!["src/main.rs".to_string()]
    ));
    assert!(matches!(
        &seen[1],
        BackgroundEvent::FileReadResult { request_id: 4, content: Ok(body), .. }
            if body.as_str() == "x"
    ));
    assert!(matches!(
        &seen[2],
        BackgroundEvent::Project(ProjectEvent::GrepResultsBatch { generation: 2, entries })
            if entries.len() == 1 && entries[0].line == 7
    ));
    assert!(matches!(
        &seen[3],
        BackgroundEvent::Project(ProjectEvent::GrepDone { generation: 2 })
    ));
    assert!(matches!(
        &seen[4],
        BackgroundEvent::Custom(name, payload)
            if name.as_str() == "ping" && payload.as_str() == "pong"
    ));
}

#[test]
fn file_search_dispatch_presents_the_finder_buffer() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let before = ed.buffers.len();

    process_background_event(
        &mut ed,
        BackgroundEvent::Project(ProjectEvent::FileSearchListed {
            pattern: "main".into(),
            files: vec!["src/main.rs".into(), "src/bin/main.rs".into()],
        }),
    );

    assert_eq!(ed.buffers.len(), before + 1, "the finder buffer is created");
    let key = ed
        .view_tree
        .focused_window_mut()
        .and_then(|w| w.buffer_id)
        .expect("focused window shows the finder");
    let buf = ed.buffers.get(key).unwrap();
    let buf = buf.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.slice(0, buf.len()), "src/main.rs\nsrc/bin/main.rs\n");
}

#[test]
fn grep_batches_append_in_generation_order_and_done_resets_the_cursor() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    ed.search.generation += 1;
    let generation = ed.search.generation;

    process_background_event(
        &mut ed,
        BackgroundEvent::Project(ProjectEvent::GrepResultsBatch {
            generation,
            entries: vec![entry("a.rs", 1)],
        }),
    );
    process_background_event(
        &mut ed,
        BackgroundEvent::Project(ProjectEvent::GrepResultsBatch {
            generation,
            entries: vec![entry("b.rs", 2)],
        }),
    );
    assert_eq!(ed.search.results.len(), 2);

    ed.search.index = 1;
    process_background_event(
        &mut ed,
        BackgroundEvent::Project(ProjectEvent::GrepDone { generation }),
    );
    assert_eq!(ed.search.index, 0);
}

#[test]
fn stale_grep_generations_are_dropped() {
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let stale = ed.search.generation;
    ed.search.generation += 1;

    process_background_event(
        &mut ed,
        BackgroundEvent::Project(ProjectEvent::GrepResultsBatch {
            generation: stale,
            entries: vec![entry("stale.rs", 1)],
        }),
    );

    assert!(ed.search.results.is_empty());
}
