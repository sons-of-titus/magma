/// Test pressing ':' through full Janet + dispatch_key pipeline
#[cfg(feature = "janet")]
mod tests {
    use crate::state::Editor;
    use crate::state::id::BufferId;
    use crate::buffer::Buffer;
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::input::{dispatch_key, focused_buffer_id};
    use crate::janet_bridge;

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
    fn check_command_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello");
        // Check that vim-enter-command does NOT exist yet
        assert!(!ed.commands.exists("vim-enter-command"));
        janet_bridge::init(&mut ed);
        // Check that vim-enter-command EXISTS after init
        assert!(ed.commands.exists("vim-enter-command"),
            "vim-enter-command should be registered after janet_bridge::init");
    }

    #[test]
    fn init_then_resolve_colon() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello");
        janet_bridge::init(&mut ed);
        let buf_id = focused_buffer_id(&ed);
        let cmd = ed.keymaps.resolve_for_buffer(":", Some(buf_id as u64));
        eprintln!("Resolved ':' -> {:?}", cmd);
        assert_eq!(cmd, Some("vim-enter-command".to_string()));
    }

    #[test]
    fn init_then_press_colon() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor("hello");
        janet_bridge::init(&mut ed);

        // Check command exists
        assert!(ed.commands.exists("vim-enter-command"));

        dispatch_key(&mut ed, ":");
        eprintln!("Mode after ':' press: {:?}", ed.editor_mode.name);
        assert_eq!(ed.editor_mode.name, "command",
            "mode should be 'command' after pressing ':', got {:?}", ed.editor_mode.name);
    }
}
