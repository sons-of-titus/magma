//! Project-switch lifecycle: path anchoring at open-file, and the outgoing
//! project's buffers not leaking into the next project.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::kernel::command::args::ArgValue;
use crate::kernel::command::execute_command;
use crate::kernel::state::Editor;
use crate::kernel::workspace;
use crate::tests::helpers::make_editor;

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("project_switch_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("a/b/c")).unwrap();
    std::fs::write(dir.join("top.txt"), "top").unwrap();
    std::fs::write(dir.join("a/b/c/deep.txt"), "deep").unwrap();
    dir
}

fn run_open(ed: &mut Editor, path: &Path) -> Result<(), String> {
    let mut args = HashMap::new();
    args.insert(
        "path".to_string(),
        ArgValue::Path(path.to_string_lossy().into_owned()),
    );
    execute_command(ed, "open-project", &args)
}

fn run_open_file(ed: &mut Editor, path: &str) -> Result<(), String> {
    let mut args = HashMap::new();
    args.insert("path".to_string(), ArgValue::Path(path.to_string()));
    execute_command(ed, "open-file", &args)
}

fn buffer_paths(ed: &Editor) -> Vec<String> {
    ed.buffers
        .iter()
        .filter_map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
        .collect()
}

#[test]
fn open_file_anchors_project_relative_paths_to_the_root() {
    let root = temp_project("anchor");
    let mut ed = make_editor();
    run_open(&mut ed, &root).unwrap();

    // The test process runs from the repo, not the project root — the exact
    // condition under which the sidebar's relative open failed.
    run_open_file(&mut ed, "a/b/c/deep.txt").unwrap();
    let expected = root.join("a/b/c/deep.txt").to_string_lossy().into_owned();
    assert!(
        buffer_paths(&ed).contains(&expected),
        "buffer path must be the root-anchored file: {:?}",
        buffer_paths(&ed)
    );

    // The absolute form focuses the same buffer — never a duplicate.
    run_open_file(&mut ed, &expected).unwrap();
    assert_eq!(
        buffer_paths(&ed)
            .iter()
            .filter(|p| **p == expected)
            .count(),
        1
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn switching_projects_closes_outgoing_buffers() {
    let a = temp_project("switch_a");
    let b = temp_project("switch_b");
    let mut ed = make_editor();
    run_open(&mut ed, &a).unwrap();
    run_open_file(&mut ed, "a/b/c/deep.txt").unwrap();
    assert!(!buffer_paths(&ed).is_empty());

    run_open(&mut ed, &b).unwrap();
    let a_file = a.join("a/b/c/deep.txt").to_string_lossy().into_owned();
    assert!(
        !buffer_paths(&ed).contains(&a_file),
        "project A's file must not survive the switch: {:?}",
        buffer_paths(&ed)
    );
    assert!(
        ed.view_tree
            .panes()
            .iter()
            .all(|p| p.buffer_id.is_none_or(|k| !ed.buffers.contains(k))),
        "no pane may still show a project A buffer"
    );

    std::fs::remove_dir_all(&a).ok();
    std::fs::remove_dir_all(&b).ok();
}

#[test]
fn switching_with_unsaved_work_is_refused_and_changes_nothing() {
    let a = temp_project("dirty_a");
    let b = temp_project("dirty_b");
    let mut ed = make_editor();
    run_open(&mut ed, &a).unwrap();
    run_open_file(&mut ed, "top.txt").unwrap();
    let key = ed
        .buffers
        .iter()
        .find(|(_, arc)| arc.lock().unwrap().path.is_some())
        .map(|(k, _)| k)
        .unwrap();
    ed.views.get_mut(&key).unwrap().insert(0, "unsaved ");

    let err = run_open(&mut ed, &b).unwrap_err();
    assert!(err.contains("modified"), "{err}");
    assert_eq!(ed.project_manager.project.root.as_deref(), Some(a.as_path()));
    assert!(buffer_paths(&ed)
        .contains(&a.join("top.txt").to_string_lossy().into_owned()));

    std::fs::remove_dir_all(&a).ok();
    std::fs::remove_dir_all(&b).ok();
}

#[test]
fn closing_the_editor_session_drops_open_files() {
    let root = temp_project("close");
    let mut ed = make_editor();
    run_open(&mut ed, &root).unwrap();
    run_open_file(&mut ed, "top.txt").unwrap();

    workspace::close_editor_session(&mut ed);
    assert!(ed.project_manager.project.root.is_none());
    assert!(buffer_paths(&ed).is_empty(), "{:?}", buffer_paths(&ed));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn create_file_anchors_relative_paths_to_the_root() {
    let root = temp_project("create");
    let mut ed = make_editor();
    run_open(&mut ed, &root).unwrap();

    let mut args = HashMap::new();
    args.insert(
        "path".to_string(),
        ArgValue::Path("a/b/new.txt".to_string()),
    );
    execute_command(&mut ed, "create-file", &args).unwrap();
    let created = root.join("a/b/new.txt").to_string_lossy().into_owned();
    assert!(Path::new(&created).is_file(), "file must land inside the project");
    assert!(buffer_paths(&ed).contains(&created));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn session_restore_anchors_relative_paths_to_the_root() {
    let root = temp_project("session_rel");
    std::fs::create_dir_all(root.join(".magma")).unwrap();
    std::fs::write(
        root.join(".magma/session.json"),
        r#"{"buffers":[{"path":"a/b/c/deep.txt","cursor":0}],"focused":"a/b/c/deep.txt"}"#,
    )
    .unwrap();
    let mut ed = make_editor();
    run_open(&mut ed, &root).unwrap();

    let expected = root.join("a/b/c/deep.txt").to_string_lossy().into_owned();
    assert!(
        buffer_paths(&ed).contains(&expected),
        "relative session entries must restore against the project root: {:?}",
        buffer_paths(&ed)
    );

    std::fs::remove_dir_all(&root).ok();
}
