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

#[test]
fn default_cursor_shape_is_block() {
    let ed = make_editor("");
    assert_eq!(ed.cursor_shape, "block");
}

#[test]
fn cursor_shape_can_be_set_to_beam() {
    let mut ed = make_editor("");
    ed.cursor_shape = "beam".to_string();
    assert_eq!(ed.cursor_shape, "beam");
}

#[test]
fn cursor_shape_can_be_set_to_underline() {
    let mut ed = make_editor("");
    ed.cursor_shape = "underline".to_string();
    assert_eq!(ed.cursor_shape, "underline");
}

#[test]
fn integer_option_returns_default_when_missing() {
    let ed = make_editor("");
    let val = crate::render::frame::int_option(&ed, "colorcolumn", 0);
    assert_eq!(val, 0);
}

#[test]
fn integer_option_parses_value() {
    let mut ed = make_editor("");
    ed.options.insert("colorcolumn".to_string(), "80".to_string());
    let val = crate::render::frame::int_option(&ed, "colorcolumn", 0);
    assert_eq!(val, 80);
}

#[test]
fn bool_option_returns_false_when_missing() {
    let ed = make_editor("");
    let val = crate::render::frame::bool_option(&ed, "number");
    assert!(!val);
}

#[test]
fn bool_option_returns_true_when_set() {
    let mut ed = make_editor("");
    ed.options.insert("number".to_string(), "true".to_string());
    let val = crate::render::frame::bool_option(&ed, "number");
    assert!(val);
}
