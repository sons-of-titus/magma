use std::collections::HashMap;

use kernel::command::{self, builtin};
use kernel::state::Editor;
use kernel::storage::disk::DiskFileSystem;

pub fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

pub fn make_editor_with_buffer(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("test", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

#[allow(dead_code)]
pub fn run(ed: &mut Editor, cmd: &str) {
    command::execute_command(ed, cmd, &HashMap::new()).unwrap();
}

pub fn focused_key(ed: &Editor) -> usize {
    ed.view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .unwrap_or(0)
}

pub fn buf_text(ed: &Editor) -> String {
    let key = focused_key(ed);
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    buf.slice(0, buf.len())
}

pub fn cursor(ed: &Editor) -> usize {
    let key = focused_key(ed);
    ed.views.get(&key).unwrap().cursor_offset()
}

pub fn acquire_janet_lock() -> std::sync::MutexGuard<'static, ()> {
    scripting::JANET_VM_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn editor_with_background() -> (
    Editor,
    kernel::runtime::BackgroundHandle,
    tokio::sync::mpsc::UnboundedReceiver<kernel::runtime::BackgroundEvent>,
) {
    let mut ed = make_editor();
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = kernel::runtime::BackgroundHandle::new(runtime, tx);
    ed.background = Some(handle.clone());
    (ed, handle, rx)
}
