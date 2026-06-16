use std::collections::HashMap;
use crate::kernel::text_engine::Buffer;
use crate::kernel::command::{self, builtin};
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::state::id::BufferId;
use crate::kernel::state::Editor;

pub fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::new(BufferId(id), "test");
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

pub fn make_editor_with_buffer(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
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
    ed.buffers.get(key).unwrap().slice(0, ed.buffers.get(key).unwrap().len())
}

pub fn cursor(ed: &Editor) -> usize {
    let key = focused_key(ed);
    ed.buffers.get(key).unwrap().cursor()
}

#[cfg(feature = "janet")]
pub fn acquire_janet_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::kernel::scripting::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
