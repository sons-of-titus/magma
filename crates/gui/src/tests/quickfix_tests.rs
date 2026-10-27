//! Quickfix flow — grep results, jump-to-line, and open-file dedup.

use std::collections::HashMap;

use kernel::command::{self, builtin, args::ArgValue};
use kernel::runtime::{process_background_event, BackgroundEvent};
use crate::render::gui_dialog::DialogAction;
use kernel::state::Editor;
use kernel::storage::disk::DiskFileSystem;
use kernel::task::QuickfixEntry;

fn editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

#[test]
fn grep_results_land_in_the_search_list() {
    let mut ed = editor();
    ed.search.generation += 1;
    let generation = ed.search.generation;
    process_background_event(&mut ed, BackgroundEvent::Project(
        kernel::project::Event::GrepDone { generation }));
    process_background_event(&mut ed, BackgroundEvent::Project(
        kernel::project::Event::GrepResultsBatch { generation, entries: Vec::new() }));
    process_background_event(&mut ed, BackgroundEvent::Project(
        kernel::project::Event::GrepResultsBatch {
            generation,
            entries: vec![
                QuickfixEntry {
                    filename: "src/main.rs".into(),
                    line: 12,
                    col: 1,
                    message: "let x = todo!".into(),
                ..Default::default() },
            ],
        }));
    assert_eq!(ed.search.results.len(), 1);
    assert_eq!(ed.search.results[0].filename, "src/main.rs");
    assert_eq!(ed.search.index, 0);
}

#[test]
fn open_file_at_line_creates_and_jumps() {
    let dir = std::env::temp_dir().join(format!("magma-jump-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("j.txt");
    std::fs::write(&path, "one\ntwo\nthree\n").unwrap();

    let mut ed = editor();
    DialogAction::OpenFileAtLine {
        path: path.to_str().unwrap().to_string(),
        line: 3,
    }.apply(&mut ed);

    // The first slab key is 0, which focused_buffer_key() also uses as its
    // "none" sentinel — resolve through the focused pane directly.
    let key = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .expect("buffer must be open and focused");
    let view = ed.views.get(&key).unwrap();
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    let pos = buf.position_of(view.cursor_offset());
    assert_eq!(pos.line, 2, "0-based line 2 = file line 3");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_file_twice_focuses_instead_of_forking() {
    let dir = std::env::temp_dir().join(format!("magma-dedup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("d.txt");
    std::fs::write(&path, "content").unwrap();
    let path_str = path.to_str().unwrap().to_string();

    let mut ed = editor();
    for _ in 0..2 {
        let mut args = HashMap::new();
        args.insert("path".to_string(), ArgValue::Path(path_str.clone()));
        command::execute_command(&mut ed, "open-file", &args).unwrap();
    }
    let opened: Vec<_> = ed.buffers.iter()
        .filter(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(path_str.as_str()))
        .collect();
    assert_eq!(opened.len(), 1, "opening twice must not fork a duplicate");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stale_search_generations_are_dropped() {
    let mut ed = editor();
    ed.search.generation = 5;
    let entry = QuickfixEntry {
        filename: "old.rs".into(),
        line: 1,
        col: 1,
        message: "stale".into(),
    ..Default::default() };
    // A result from generation 3 lands after the search moved to 5: gone.
    process_background_event(&mut ed, BackgroundEvent::Project(
        kernel::project::Event::GrepResultsBatch {
            generation: 3,
            entries: vec![entry],
        }));
    assert!(ed.search.results.is_empty(), "stale search results must never land");

    let fresh = QuickfixEntry {
        filename: "new.rs".into(),
        line: 2,
        col: 1,
        message: "fresh".into(),
    ..Default::default() };
    process_background_event(&mut ed, BackgroundEvent::Project(
        kernel::project::Event::GrepResultsBatch {
            generation: 5,
            entries: vec![fresh],
        }));
    assert_eq!(ed.search.results.len(), 1);
    assert_eq!(ed.search.results[0].filename, "new.rs");
}

#[test]
fn build_problems_and_find_results_are_separate() {
    let mut ed = editor();
    kernel::task::apply_tool_diagnostics(&mut ed, "cargo build",
        "error[E0308]: mismatched types\n --> src/main.rs:1:1\n",
        std::path::Path::new("/repo"));
    assert_eq!(ed.quickfix_list.len(), 1);

    ed.search.generation += 1;
    let generation = ed.search.generation;
    process_background_event(&mut ed, BackgroundEvent::Project(
        kernel::project::Event::GrepResultsBatch {
            generation,
            entries: vec![QuickfixEntry {
                filename: "a.rs".into(), line: 3, col: 1, message: "hit".into(),
            ..Default::default() }],
        }));

    assert_eq!(ed.search.results.len(), 1);
    assert_eq!(ed.quickfix_list.len(), 1, "a grep must not clear build problems");
    assert_eq!(ed.quickfix_list[0].filename, "/repo/src/main.rs");
}

#[test]
fn quickfix_focus_selects_the_navigated_list() {
    let mut ed = editor();
    ed.quickfix_list = vec![QuickfixEntry {
        filename: "build.rs".into(), line: 1, col: 1, message: "e".into(),
    ..Default::default() }];
    ed.search.set_results(vec![
        QuickfixEntry { filename: "s1.rs".into(), line: 1, col: 1, message: "h".into(), ..Default::default() },
        QuickfixEntry { filename: "s2.rs".into(), line: 1, col: 1, message: "h".into(), ..Default::default() },
    ]);

    // No relevant panel focused → build problems (conventional quickfix).
    assert_eq!(ed.quickfix_focus, kernel::task::QuickfixList::Problems);
    assert!(kernel::task::quickfix_next(&mut ed).is_err(),
        "one problem, nothing to advance to");

    // Focused Find panel → search results; the problems cursor stays put.
    ed.quickfix_focus = kernel::task::QuickfixList::Search;
    kernel::task::quickfix_next(&mut ed).unwrap();
    assert_eq!(ed.search.index, 1);
    assert_eq!(ed.quickfix_index, 0, "the problems cursor must not move");
}

#[test]
fn find_result_jump_leaves_build_diagnostics_untouched() {
    let dir = std::env::temp_dir().join(format!("magma-qf-diag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hit.txt");
    std::fs::write(&path, "alpha\nbeta\n").unwrap();
    let path_str = path.to_string_lossy().into_owned();

    let mut ed = editor();
    ed.search.set_results(vec![
        QuickfixEntry { filename: path_str.clone(), line: 1, col: 1, message: "h1".into(), ..Default::default() },
        QuickfixEntry { filename: path_str.clone(), line: 2, col: 1, message: "h2".into(), ..Default::default() },
    ]);
    ed.quickfix_focus = kernel::task::QuickfixList::Search;
    kernel::task::quickfix_next(&mut ed).unwrap();

    let key = ed.buffers.iter()
        .find(|(_, arc)| arc.lock().unwrap().path.as_deref() == Some(path_str.as_str()))
        .map(|(k, _)| k)
        .expect("the hit file opens");
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert!(buf.tool_diagnostics.is_empty(),
        "a find result must not paint the file with build errors");
    std::fs::remove_dir_all(&dir).ok();
}
