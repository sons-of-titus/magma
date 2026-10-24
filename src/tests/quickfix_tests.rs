//! Quickfix flow — grep results, jump-to-line, and open-file dedup.

use std::collections::HashMap;

use crate::kernel::command::{self, builtin, args::ArgValue};
use crate::kernel::runtime::{process_background_event, BackgroundEvent};
use crate::kernel::render::gui_dialog::DialogAction;
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::task::QuickfixEntry;

fn editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

#[test]
fn grep_results_replace_the_quickfix_list() {
    let mut ed = editor();
    ed.search_generation += 1;
    let generation = ed.search_generation;
    process_background_event(&mut ed, BackgroundEvent::GrepDone { generation });
    process_background_event(&mut ed, BackgroundEvent::GrepResultsBatch { generation, entries: Vec::new() });
    process_background_event(&mut ed, BackgroundEvent::GrepResultsBatch {
        generation,
        entries: vec![
            QuickfixEntry {
                filename: "src/main.rs".into(),
                line: 12,
                col: 1,
                message: "let x = todo!".into(),
            ..Default::default() },
        ],
    });
    assert_eq!(ed.quickfix_list.len(), 1);
    assert_eq!(ed.quickfix_list[0].filename, "src/main.rs");
    assert_eq!(ed.quickfix_index, 0);
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
    ed.search_generation = 5;
    let entry = QuickfixEntry {
        filename: "old.rs".into(),
        line: 1,
        col: 1,
        message: "stale".into(),
    ..Default::default() };
    // A result from generation 3 lands after the search moved to 5: gone.
    process_background_event(&mut ed, BackgroundEvent::GrepResultsBatch {
        generation: 3,
        entries: vec![entry],
    });
    assert!(ed.quickfix_list.is_empty(), "stale search results must never land");

    let fresh = QuickfixEntry {
        filename: "new.rs".into(),
        line: 2,
        col: 1,
        message: "fresh".into(),
    ..Default::default() };
    process_background_event(&mut ed, BackgroundEvent::GrepResultsBatch {
        generation: 5,
        entries: vec![fresh],
    });
    assert_eq!(ed.quickfix_list.len(), 1);
    assert_eq!(ed.quickfix_list[0].filename, "new.rs");
}
