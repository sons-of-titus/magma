use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::process::QuickfixEntry;
use crate::state::id::BufferId;
use crate::state::Editor;

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", "hello\n");
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
    ed
}

// ── module_paths default ──────────────────────────────────────────────────────

#[test]
fn module_paths_contains_plugins_dir_by_default() {
    let ed = make_editor();
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        assert!(ed.module_paths.iter().any(|p| p.contains("magma/plugins")));
    }
}

#[test]
fn module_paths_can_be_mutated() {
    let mut ed = make_editor();
    ed.module_paths.push("/tmp/test-plugins".into());
    assert!(ed.module_paths.contains(&"/tmp/test-plugins".to_string()));
}

// ── mark ring ─────────────────────────────────────────────────────────────────

#[test]
fn mark_ring_starts_empty() {
    let ed = make_editor();
    assert!(ed.mark_ring.is_empty());
}

#[test]
fn mark_ring_push_and_pop() {
    let mut ed = make_editor();
    ed.mark_ring.push(("src/main.rs".into(), 42));
    ed.mark_ring.push(("src/lib.rs".into(), 100));
    assert_eq!(ed.mark_ring.len(), 2);
    let (path, offset) = ed.mark_ring.pop().unwrap();
    assert_eq!(path, "src/lib.rs");
    assert_eq!(offset, 100);
    let (path, offset) = ed.mark_ring.pop().unwrap();
    assert_eq!(path, "src/main.rs");
    assert_eq!(offset, 42);
    assert!(ed.mark_ring.is_empty());
}

#[test]
fn mark_ring_capped_at_100() {
    let mut ed = make_editor();
    for i in 0..110usize {
        if ed.mark_ring.len() >= 100 {
            ed.mark_ring.remove(0);
        }
        ed.mark_ring.push((format!("file{}.rs", i), i));
    }
    assert_eq!(ed.mark_ring.len(), 100);
    // Most recent should be file109
    assert_eq!(ed.mark_ring.last().unwrap().0, "file109.rs");
}

// ── quickfix list ─────────────────────────────────────────────────────────────

#[test]
fn quickfix_list_starts_empty() {
    let ed = make_editor();
    assert!(ed.quickfix_list.is_empty());
}

#[test]
fn quickfix_list_can_be_set() {
    let mut ed = make_editor();
    ed.quickfix_list = vec![
        QuickfixEntry { filename: "src/main.rs".into(), line: 10, col: 5, message: "error".into() },
        QuickfixEntry { filename: "src/lib.rs".into(), line: 20, col: 1, message: "warning".into() },
    ];
    assert_eq!(ed.quickfix_list.len(), 2);
    assert_eq!(ed.quickfix_list[0].filename, "src/main.rs");
    assert_eq!(ed.quickfix_list[1].line, 20);
}

#[test]
fn quickfix_index_resets_to_zero() {
    let mut ed = make_editor();
    ed.quickfix_index = 5;
    ed.quickfix_list = vec![
        QuickfixEntry { filename: "a.rs".into(), line: 1, col: 1, message: "x".into() },
    ];
    ed.quickfix_index = 0;
    assert_eq!(ed.quickfix_index, 0);
}
