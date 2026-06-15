use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::id::BufferId;
use crate::state::Editor;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
    ed
}

fn focused_key(ed: &Editor) -> usize {
    ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap_or(0)
}

#[test]
fn ts_trees_and_languages_stored_on_editor() {
    let mut ed = make_editor("fn main() {}");
    let key = focused_key(&ed);

    ed.ts_languages.insert(key, "rust".to_string());
    ed.ts_trees.insert(key, vec![1, 2, 3]);

    assert!(ed.ts_trees.contains_key(&key));
    assert_eq!(ed.ts_languages.get(&key).unwrap(), "rust");
}

#[test]
fn ts_has_tree_returns_false_for_missing() {
    let ed = make_editor("");
    assert!(!ed.ts_trees.contains_key(&999));
}

#[test]
fn ts_language_set_on_valid_buffer() {
    let mut ed = make_editor("hello");
    let key = focused_key(&ed);
    ed.ts_languages.insert(key, "janet".to_string());
    assert_eq!(ed.ts_languages.get(&key).unwrap(), "janet");
}
