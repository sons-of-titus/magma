//! Tests for project sessions: capture/restore of open files, cursors,
//! focus, breakpoints, and watches.

use std::path::PathBuf;

use crate::kernel::command::execute_command;
use crate::kernel::session::{self, SessionBreakpoint};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;

fn make_editor_with_builtins() -> Editor {
    crate::tests::helpers::make_editor()
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("session_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(root: &std::path::Path, name: &str, content: &str) -> String {
    let path = root.join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

fn open_via_command(ed: &mut Editor, path: &str) {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(path.to_string()));
    execute_command(ed, "open-file", &args).unwrap();
}

#[test]
fn capture_records_file_buffers_cursors_and_focus() {
    let root = temp_root("capture");
    let a = write_file(&root, "a.rs", "one\ntwo\nthree\n");
    let b = write_file(&root, "b.rs", "x\n");
    let mut ed = make_editor_with_builtins();
    open_via_command(&mut ed, &a);
    open_via_command(&mut ed, &b);

    let state = session::capture(&ed);
    let mut paths: Vec<&str> = state.buffers.iter().map(|b| b.path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, vec![a.as_str(), b.as_str()]);
    assert_eq!(state.focused.as_deref(), Some(b.as_str()),
        "the most recently opened file is focused");

    // Scratch buffers (no path) never enter the session.
    assert!(state.buffers.iter().all(|b| !b.path.is_empty()));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn save_restore_round_trips_cursors_and_focus() {
    let root = temp_root("roundtrip");
    let a = write_file(&root, "a.rs", "one\ntwo\nthree\n");
    let b = write_file(&root, "b.rs", "x\ny\n");
    let mut ed = make_editor_with_builtins();
    open_via_command(&mut ed, &a);
    let a_key = ed.buffers.iter().find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(a.as_str())).map(|(k, _)| k).unwrap();
    let line2_start = {
        let arc = ed.buffers.get_mut(a_key).unwrap();
        let buf = arc.lock().unwrap();
        buf.line_start_offset(1).unwrap()
    };
    if let Some(view) = ed.views.get_mut(&a_key) {
        view.set_cursor(line2_start + 2);
    }
    open_via_command(&mut ed, &b);

    session::save(&ed, &root).unwrap();

    let mut fresh = make_editor_with_builtins();
    session::restore(&mut fresh, &root);

    let paths: Vec<String> = fresh.buffers.iter()
        .filter_map(|(_, arc)| arc.lock().unwrap().path.clone())
        .collect();
    assert!(paths.contains(&a) && paths.contains(&b),
        "both files reopen: {paths:?}");

    let a_key2 = fresh.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(a.as_str()))
        .map(|(k, _)| k).unwrap();
    let cursor = fresh.views.get(&a_key2).map(|v| v.cursor_offset());
    assert_eq!(cursor, Some(line2_start + 2), "cursor restores to the saved offset");
    assert_eq!(crate::tests::helpers::focused_key(&fresh),
        fresh.buffers.iter()
            .find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(b.as_str()))
            .map(|(k, _)| k).unwrap(),
        "the saved focus restores");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn restore_recreates_breakpoints_and_watches() {
    let root = temp_root("bps");
    let a = write_file(&root, "a.rs", "l1\nl2\n");
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    ed.debug.add_breakpoint(a.clone(), 2);
    ed.debug.set_enabled(&a, 2, false);
    ed.debug.watches.push("len".to_string());
    session::save(&ed, &root).unwrap();

    let mut fresh = make_editor_with_builtins();
    session::restore(&mut fresh, &root);

    let bps = fresh.debug.breakpoints_for(&a);
    assert_eq!(bps.len(), 1);
    assert_eq!(bps[0].line, 2);
    assert!(!bps[0].enabled, "disabled state persists");
    assert_eq!(fresh.debug.watches, vec!["len".to_string()]);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn restore_skips_missing_files_and_bad_sessions() {
    let root = temp_root("missing");
    std::fs::create_dir_all(root.join(".magma")).unwrap();
    std::fs::write(root.join(".magma/session.json"), r#"{
        "buffers": [{"path": "/definitely/not/there.rs", "cursor": 0}]
    }"#).unwrap();

    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    session::restore(&mut ed, &root);
    assert!(ed.buffers.iter().all(|(_, arc)| arc.lock().unwrap().path.is_none()),
        "missing files are skipped without error");

    std::fs::write(root.join(".magma/session.json"), "not json at all").unwrap();
    session::restore(&mut ed, &root);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn session_breakpoint_serialization_defaults_enabled() {
    let bp: SessionBreakpoint = serde_json::from_str(r#"{"file":"/a.rs","line":1}"#).unwrap();
    assert!(bp.enabled);
}

fn open_two_tabs_with_project(tag: &str) -> (PathBuf, Editor, String, String, usize) {
    let root = temp_root(tag);
    let a = write_file(&root, "a.rs", "one\n");
    let b = write_file(&root, "b.rs", "two\n");
    let mut ed = make_editor_with_builtins();
    open_via_command(&mut ed, &a);
    open_via_command(&mut ed, &b);
    let b_key = ed
        .buffers
        .iter()
        .find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(b.as_str()))
        .map(|(k, _)| k)
        .unwrap();
    ed.project_manager.project = crate::kernel::project::Project {
        root: Some(root.clone()),
        name: Some("sessiontest".into()),
        ..crate::kernel::project::Project::default()
    };
    (root, ed, a, b, b_key)
}

fn session_json(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join(".magma/session.json"))
        .expect("closing a tab must flush the session file")
}

#[test]
fn closed_tab_never_resurrects_in_capture_or_session_file() {
    let (root, mut ed, a, b, b_key) = open_two_tabs_with_project("closed_tab");

    // The tab-strip close button: only the pane goes away.
    let b_pane = ed
        .view_tree
        .panes()
        .iter()
        .find(|p| p.buffer_id == Some(b_key))
        .map(|p| p.id)
        .expect("b has a pane");
    kernel::render::tabs::close_tab(&mut ed, b_pane);

    let state = session::capture(&ed);
    assert_eq!(
        state.buffers.iter().map(|b| b.path.as_str()).collect::<Vec<_>>(),
        vec![a.as_str()],
        "a buffer whose tab closed is not captured even though it lives on"
    );

    // close_tab flushes the session file immediately — a crash right after
    // must not resurrect the tab either.
    let raw = session_json(&root);
    assert!(!raw.contains(b.as_str()), "closed tab absent from disk: {raw}");
    assert!(raw.contains(a.as_str()));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn close_buffer_command_flushes_the_session_file() {
    let (root, mut ed, a, b, b_key) = open_two_tabs_with_project("closed_buffer");

    let mut args = std::collections::HashMap::new();
    args.insert(
        "buffer-id".to_string(),
        crate::kernel::command::args::ArgValue::Integer(b_key as i64),
    );
    execute_command(&mut ed, "close-buffer", &args).unwrap();

    let raw = session_json(&root);
    assert!(!raw.contains(b.as_str()), "closed buffer absent from disk: {raw}");
    assert!(raw.contains(a.as_str()));

    std::fs::remove_dir_all(&root).ok();
}
