use std::collections::HashMap;
use crate::kernel::command::{self, builtin};
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::state::Editor;

pub fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer("test");
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

pub fn make_editor_with_buffer(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("test", content);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

pub fn run(ed: &mut Editor, cmd: &str) {
    command::execute_command(ed, cmd, &HashMap::new()).unwrap();
}

pub fn focused_key(ed: &Editor) -> usize {
    ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
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

#[cfg(feature = "janet")]
pub fn acquire_janet_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::kernel::scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
