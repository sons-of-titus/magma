//! Project-opening lifecycle — the shell-first contract.
//!
//! `open-project` establishes the project context synchronously and pushes
//! everything expensive (file walk, session restore, task detection, index
//! stamp scan) onto the background pool.  These tests pin that split: the
//! context is usable with the pool's results never drained, streamed
//! results apply in order, and a superseded project's results are dropped.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::kernel::command::{self, args::ArgValue};
use crate::kernel::index::Event as IndexEvent;
use crate::kernel::project::Project;
use crate::kernel::run_config::{RunConfig, RunConfigManager};
use crate::kernel::runtime::{BackgroundEvent, process_background_event};

fn open_project(ed: &mut crate::kernel::state::Editor, path: &str) {
    let mut args = HashMap::new();
    args.insert("path".to_string(), ArgValue::Path(path.to_string()));
    command::execute_command(ed, "open-project", &args).unwrap();
}

fn temp_project(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for (rel, content) in files {
        let path = root.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
    root
}

fn buffer_paths(ed: &crate::kernel::state::Editor) -> Vec<String> {
    ed.buffers
        .iter()
        .filter_map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
        .collect()
}

/// Drain the background channel until `ready` holds or the deadline passes.
fn pump_until(
    ed: &mut crate::kernel::state::Editor,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<BackgroundEvent>,
    ready: impl Fn(&crate::kernel::state::Editor) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready(ed) {
        match rx.try_recv() {
            Ok(ev) => process_background_event(ed, ev),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => panic!("pool gone"),
            Err(_) if Instant::now() >= deadline => {
                panic!("background work never became ready");
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

// ── Shell-first ──────────────────────────────────────────────────────────────

#[test]
fn open_project_establishes_context_while_background_work_is_blocked() {
    let root = temp_project(
        "project_open_shell_first",
        &[("src/a.rs", "fn a() {}\n"), ("src/b.rs", "fn b() {}\n")],
    );
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    let before = ed.buffers.len();

    open_project(&mut ed, root.to_string_lossy().as_ref());

    // The context is fully established even though the pool's receiver is
    // never drained — the walk, session reads, and detection could be stuck
    // forever and the shell would still be usable.
    assert_eq!(
        ed.project_manager.project.root.as_deref(),
        Some(root.as_path())
    );
    assert_eq!(ed.project_manager.project.generation, 1);
    assert!(ed.project_manager.project.indexing, "a walk is in flight");
    assert_eq!(
        ed.buffers.len(),
        before,
        "no session file may open synchronously"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn repeated_open_bumps_the_generation() {
    let root = temp_project("project_open_repeated", &[("a.rs", "x")]);
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();

    open_project(&mut ed, root.to_string_lossy().as_ref());
    assert_eq!(ed.project_manager.project.generation, 1);
    open_project(&mut ed, root.to_string_lossy().as_ref());
    assert_eq!(ed.project_manager.project.generation, 2);

    let _ = std::fs::remove_dir_all(&root);
}

// ── Session restore streams in ───────────────────────────────────────────────

#[test]
fn session_buffers_stream_in_and_the_recorded_focus_wins() {
    let root = temp_project(
        "project_open_session",
        &[
            ("a.rs", "alpha\n"),
            ("b.rs", "bravo\n"),
            ("focus.rs", "focused\n"),
        ],
    );
    let session = format!(
        r#"{{"buffers":[{{"path":"{a}","cursor":3}},{{"path":"{b}","cursor":0}}],"focused":"{f}"}}"#,
        a = root.join("a.rs").to_string_lossy(),
        b = root.join("b.rs").to_string_lossy(),
        f = root.join("focus.rs").to_string_lossy(),
    );
    std::fs::create_dir_all(root.join(".magma")).unwrap();
    std::fs::write(root.join(".magma/session.json"), session).unwrap();

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    // Done means the recorded focus is back on screen: pump until then.
    let focused_is_recorded = |ed: &crate::kernel::state::Editor| {
        ed.view_tree
            .focused_window()
            .and_then(|wid| ed.view_tree.buffer(wid))
            .and_then(|key| ed.buffers.get(key))
            .and_then(|arc| {
                arc.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .path
                    .as_deref()
                    .is_some_and(|p| p.ends_with("focus.rs"))
                    .then_some(())
            })
            .is_some()
    };
    pump_until(&mut ed, &mut rx, |ed| {
        buffer_paths(ed).len() == 3 && focused_is_recorded(ed)
    });

    // The recorded cursor landed with the buffer, and the recorded focus is
    // the buffer on screen at the end.
    let a_key = ed
        .buffers
        .iter()
        .find(|(_, arc)| {
            arc.lock()
                .unwrap_or_else(|e| e.into_inner())
                .path
                .as_deref()
                .is_some_and(|p| p.ends_with("a.rs"))
        })
        .map(|(k, _)| k)
        .unwrap();
    assert_eq!(ed.views.get(&a_key).unwrap().cursor_offset(), 3);
    let focused_path = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .and_then(|key| ed.buffers.get(key))
        .map(|arc| arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
        .flatten();
    assert_eq!(
        focused_path.map(|p| p.ends_with("focus.rs")),
        Some(true),
        "the recorded focus wins after the restore"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_session_from_a_superseded_project_never_opens() {
    let p1 = temp_project("project_open_stale_session", &[("old.rs", "old\n")]);
    let p2 = temp_project("project_open_fresh_session", &[("new.rs", "new\n")]);
    std::fs::create_dir_all(p1.join(".magma")).unwrap();
    std::fs::write(
        p1.join(".magma/session.json"),
        format!(
            r#"{{"buffers":[{{"path":"{}","cursor":0}}]}}"#,
            p1.join("old.rs").to_string_lossy()
        ),
    )
    .unwrap();

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, p1.to_string_lossy().as_ref());
    // Switch before p1's reads land: its events are now stale.
    open_project(&mut ed, p2.to_string_lossy().as_ref());
    assert_eq!(ed.project_manager.project.generation, 2);

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(ev) => process_background_event(&mut ed, ev),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                if !buffer_paths(&ed).iter().any(|p| p.ends_with("old.rs")) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => panic!("pool gone"),
        }
    }
    assert!(
        !buffer_paths(&ed).iter().any(|p| p.ends_with("old.rs")),
        "project A's session must never open into project B"
    );

    let _ = std::fs::remove_dir_all(&p1);
    let _ = std::fs::remove_dir_all(&p2);
}

// ── Walk, stamp scan, extraction ─────────────────────────────────────────────

#[test]
fn the_full_index_pipeline_streams_through_the_pool() {
    let root = temp_project(
        "project_open_pipeline",
        &[("src/lib.rs", "fn alpha() {}\nfn beta() {}\n")],
    );
    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    // Walk batches land → done triggers the stamp scan → stale extraction
    // runs → the record merges, all through the one channel.
    pump_until(&mut ed, &mut rx, |ed| {
        !ed.project_manager.project.files.is_empty()
            && !ed.project_manager.project.indexing
            && ed.index.stats().files == 1
    });
    assert!(
        ed.project_manager
            .project
            .files
            .contains(&"src/lib.rs".to_string())
    );
    assert!(ed.index.stats().symbols >= 2);
    assert!(
        ed.project_manager.project.files_epoch >= 1,
        "the tree model was told about the new list"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_stamp_scan_from_a_superseded_project_is_dropped() {
    let root = temp_project("project_open_stale_scan", &[("lib.rs", "fn a() {}\n")]);
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    // A scan produced under generation 0 (before this open) arriving now.
    process_background_event(
        &mut ed,
        BackgroundEvent::Index(IndexEvent::StampScan {
            generation: 0,
            stamps: vec![("lib.rs".into(), 0, 0)],
        }),
    );
    assert_eq!(
        ed.index.stats().pending,
        0,
        "a stale scan must not enqueue extraction against the new project"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn extraction_drains_for_a_project_wider_than_one_chunk() {
    // Three chunks of 64: when extraction jobs shared a coalesce key, every
    // chunk but the newest skipped itself and its queue slots never drained.
    let pairs: Vec<(String, String)> = (0..150)
        .map(|i| (format!("src/f{i}.rs"), format!("fn f{i}() {{}}\n")))
        .collect();
    let files: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(name, body)| (name.as_str(), body.as_str()))
        .collect();
    let root = temp_project("project_open_many_chunks", &files);
    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    pump_until(&mut ed, &mut rx, |ed| {
        ed.index.stats().pending == 0 && ed.index.stats().files == 150
    });
    assert_eq!(ed.index.stats().symbols, 150, "every file was extracted");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unreadable_files_still_drain_the_extraction_queue() {
    let root = temp_project("project_open_binary", &[("src/lib.rs", "fn a() {}\n")]);
    // The walk lists every file; binaries fail the UTF-8 read and yield no
    // record — but they were counted, so their slots must report back.
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join("assets/logo.png"), [0xFFu8, 0xFE, 0x00, 0x01]).unwrap();
    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    // `files == 1` implies the scan already counted both slots, so a zero
    // pending now can only mean the skip reported back too.
    pump_until(&mut ed, &mut rx, |ed| {
        !ed.project_manager.project.indexing
            && ed.index.stats().files == 1
            && ed.index.stats().pending == 0
    });
    assert_eq!(
        ed.index.stats().files,
        1,
        "the binary stays out of the source index"
    );
    assert!(
        ed.project_manager
            .project
            .files
            .iter()
            .any(|f| f.ends_with("logo.png")),
        "the walk still lists it"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reopening_a_project_clears_stranded_extraction_slots() {
    let root = temp_project("project_open_slot_reset", &[("lib.rs", "fn a() {}\n")]);
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    // Outstanding slots counted under the outgoing generation; their
    // arrivals would be dropped as stale after the reopen.
    ed.index.note_pending(3);
    open_project(&mut ed, root.to_string_lossy().as_ref());
    assert_eq!(
        ed.index.stats().pending,
        0,
        "the generation bump invalidates the outgoing slots"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ── Task detection ───────────────────────────────────────────────────────────

#[test]
fn tasks_detected_applies_language_and_configs() {
    let root = temp_project("project_open_tasks", &[("Cargo.toml", "[package]\n")]);
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    let mut configs = RunConfigManager::default();
    configs.configs.push(RunConfig {
        name: "build".into(),
        command: "cargo build".into(),
        args: vec![],
        cwd: None,
        env: Default::default(),
        build_before_run: None,
        debuggable: false,
    });
    let generation = ed.project_manager.project.generation;
    process_background_event(
        &mut ed,
        BackgroundEvent::TasksDetected {
            generation,
            language: Some("rust".into()),
            configs,
        },
    );
    assert_eq!(ed.project_manager.project.language, "rust");
    assert!(ed.project_manager.run_configs.named("build").is_some());
    assert!(
        ed.project_manager
            .projects
            .get(root.file_name().unwrap().to_str().unwrap())
            .is_some_and(|p| p.language == "rust"),
        "the registry snapshot tracks the enriched active project"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tasks_detected_from_a_superseded_project_is_dropped() {
    let root = temp_project("project_open_stale_tasks", &[]);
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    let stale = ed.project_manager.project.generation - 1;
    process_background_event(
        &mut ed,
        BackgroundEvent::TasksDetected {
            generation: stale,
            language: Some("go".into()),
            configs: RunConfigManager::default(),
        },
    );
    assert_eq!(
        ed.project_manager.project.language, "",
        "a stale detection never describes the active project"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ── Progress signals ─────────────────────────────────────────────────────────

#[test]
fn the_session_restore_counter_drains_as_buffers_land() {
    let root = temp_project(
        "project_open_progress_session",
        &[("a.rs", "alpha\n"), ("b.rs", "bravo\n")],
    );
    let session = format!(
        r#"{{"buffers":[{{"path":"{a}","cursor":0}},{{"path":"{b}","cursor":0}}]}}"#,
        a = root.join("a.rs").to_string_lossy(),
        b = root.join("b.rs").to_string_lossy(),
    );
    std::fs::create_dir_all(root.join(".magma")).unwrap();
    std::fs::write(root.join(".magma/session.json"), session).unwrap();

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());
    assert_eq!(
        ed.session_restore_pending, 2,
        "the strip sees the restore queue depth immediately"
    );

    pump_until(&mut ed, &mut rx, |ed| ed.session_restore_pending == 0);
    assert!(
        !buffer_paths(&ed).is_empty(),
        "the drained queue actually opened the buffers"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_restore_from_a_superseded_project_never_leaks_into_the_counter() {
    let p1 = temp_project("project_open_progress_stale", &[("old.rs", "old\n")]);
    let p2 = temp_project("project_open_progress_fresh", &[("new.rs", "new\n")]);
    std::fs::create_dir_all(p1.join(".magma")).unwrap();
    std::fs::write(
        p1.join(".magma/session.json"),
        format!(
            r#"{{"buffers":[{{"path":"{}","cursor":0}}]}}"#,
            p1.join("old.rs").to_string_lossy()
        ),
    )
    .unwrap();

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, p1.to_string_lossy().as_ref());
    assert_eq!(ed.session_restore_pending, 1);
    // Switching projects resets the surface to the new project's truth.
    open_project(&mut ed, p2.to_string_lossy().as_ref());
    assert_eq!(ed.session_restore_pending, 0);

    // p1's queued read lands late: stale, so the counter stays put.
    while let Ok(ev) = rx.try_recv() {
        process_background_event(&mut ed, ev);
    }
    assert_eq!(
        ed.session_restore_pending, 0,
        "a stale restore must not raise the active project's progress"
    );
    assert!(
        !buffer_paths(&ed).iter().any(|p| p.ends_with("old.rs")),
        "and its buffers still never open"
    );

    let _ = std::fs::remove_dir_all(&p1);
    let _ = std::fs::remove_dir_all(&p2);
}

#[test]
fn the_detection_flag_tracks_in_flight_run_config_detection() {
    let root = temp_project(
        "project_open_progress_detect",
        &[("Cargo.toml", "[package]\n")],
    );
    let (mut ed, _bg, _rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    // What task/detect-project does while the pool job is queued.
    ed.project_manager.project.detecting_tasks = true;
    let stale = ed.project_manager.project.generation - 1;
    process_background_event(
        &mut ed,
        BackgroundEvent::TasksDetected {
            generation: stale,
            language: Some("go".into()),
            configs: RunConfigManager::default(),
        },
    );
    assert!(
        ed.project_manager.project.detecting_tasks,
        "a stale detection result must not settle the current chip"
    );

    let generation = ed.project_manager.project.generation;
    process_background_event(
        &mut ed,
        BackgroundEvent::TasksDetected {
            generation,
            language: Some("rust".into()),
            configs: RunConfigManager::default(),
        },
    );
    assert!(
        !ed.project_manager.project.detecting_tasks,
        "the project's own detection result clears the chip"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_settled_project_reports_no_progress() {
    let root = temp_project(
        "project_open_progress_settled",
        &[("src/lib.rs", "fn a() {}\n")],
    );
    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());

    pump_until(&mut ed, &mut rx, |ed| {
        !ed.project_manager.project.files.is_empty()
            && !ed.project_manager.project.indexing
            && ed.index.stats().pending == 0
            && ed.session_restore_pending == 0
            && !ed.project_manager.project.detecting_tasks
    });

    let _ = std::fs::remove_dir_all(&root);
}

// ── Registry consistency ─────────────────────────────────────────────────────

#[test]
fn open_root_resets_per_project_state_and_the_registry_entry_tracks_it() {
    let root = temp_project("project_open_registry", &[("a.rs", "x\n")]);
    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());
    pump_until(&mut ed, &mut rx, |ed| {
        !ed.project_manager.project.files.is_empty()
    });

    let name = ed
        .project_manager
        .current_project
        .clone()
        .expect("active project registered");
    let registry_files = ed
        .project_manager
        .projects
        .get(&name)
        .map(|p| p.files.len())
        .unwrap_or(usize::MAX);
    assert_eq!(
        registry_files,
        ed.project_manager.project.files.len(),
        "registry mirrors the active project after indexing"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_defaults_keep_generation_zero_for_embedded_hosts() {
    let project = Project::default();
    assert_eq!(project.generation, 0);
    assert!(!project.indexing);
    assert!(!project.detecting_tasks);
}

#[test]
fn switching_projects_shuts_down_lsp_terminals_and_debug() {
    // PM-02/PM-03/PM-06: the outgoing project's language servers (rooted
    // in it), terminals, and debug sessions must follow it out — a
    // survivor would answer the new project with the old workspace — and
    // the new project's index must serve immediately, not the old one.
    use kernel::semantic::lsp::{EventSink, LspHandle};
    use std::sync::Arc;

    struct NoopSink;
    impl EventSink for NoopSink {
        fn send(&self, _event: kernel::semantic::lsp::Event) {}
    }

    fn temp(root_name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(root_name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
        dir
    }
    let a = temp("magma_switch_a");
    let b = temp("magma_switch_b");

    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, a.to_string_lossy().as_ref());
    pump_until(&mut ed, &mut rx, |ed| {
        !ed.project_manager.project.files.is_empty()
    });

    // A live fake language server rooted in project A (`cat` stays alive
    // reading stdin; shutdown() closes it in order).
    let handle = LspHandle::start(
        "cat",
        &[],
        "rust",
        Some(a.to_string_lossy().into_owned()),
        NoopSink,
    )
    .expect("fake server starts");
    ed.lsp.insert("rust", Arc::new(handle));

    open_project(&mut ed, b.to_string_lossy().as_ref());
    assert_eq!(
        ed.project_manager.current_project.as_deref(),
        Some("magma_switch_b"),
        "the switch itself completes"
    );

    assert!(
        ed.lsp.get("rust").is_none(),
        "the outgoing project's language server must not survive the switch"
    );
    assert!(
        ed.io.terminals.sessions.is_empty(),
        "terminals follow the project out"
    );
    assert!(
        ed.debug.active_session().is_none(),
        "debug sessions follow the project out"
    );
    let a_file = a.join("a.rs").to_string_lossy().into_owned();
    assert!(
        ed.index.symbols_in_file(&a_file).is_empty(),
        "project A's index records must not leak into project B queries"
    );

    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn session_persists_and_restores_split_panes() {
    // PM-08: a split arrangement must survive close/reopen — one split
    // per recorded fork, showing its buffer, focus untouched.
    use kernel::state::id::WindowId;
    let root = temp_project(
        "project_open_splits",
        &[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")],
    );
    let (mut ed, _bg, mut rx) = crate::tests::helpers::editor_with_background();
    open_project(&mut ed, root.to_string_lossy().as_ref());
    pump_until(&mut ed, &mut rx, |ed| {
        !ed.project_manager.project.files.is_empty()
    });

    let a_path = root.join("a.rs").to_string_lossy().into_owned();
    let b_path = root.join("b.rs").to_string_lossy().into_owned();
    kernel::session::open_path(&mut ed, &a_path);
    let a_key = ed.focused_buffer_key();
    kernel::session::open_path(&mut ed, &b_path);
    let b_key = ed.focused_buffer_key();

    // Fork b into a second window (what a split creates).
    ed.fork_view_for_window(WindowId(9), b_key);

    let state = kernel::session::capture(&ed);
    assert_eq!(
        state.split_buffers,
        vec![b_path.clone()],
        "the forked buffer is recorded as a split"
    );

    // Simulate reopen: a fresh editor without a background pool restores
    // the session synchronously inside open-project — no second call.
    kernel::session::save(&ed, &root).expect("session write");
    let mut ed2 = crate::tests::helpers::make_editor();
    open_project(&mut ed2, root.to_string_lossy().as_ref());
    assert_eq!(
        ed2.window_views.len(),
        1,
        "the split fork exists again after restore"
    );
    let (_, view) = ed2.window_views.iter().next().unwrap();
    assert_eq!(view.buffer.lock().unwrap().path.as_deref(), Some(b_path.as_str()));

    let _ = std::fs::remove_dir_all(&root);
    let _ = a_key;
}
